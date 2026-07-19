use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use anyhow::{Context, Result, bail};
use axum::{
    Router,
    extract::{
        ConnectInfo, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use futures_util::SinkExt;
use rand::{Rng, distr::Alphanumeric};
use tokio::{
    net::TcpListener,
    sync::mpsc,
    time::{Duration, MissedTickBehavior, interval, timeout},
};

use crate::{
    cli::ServeArgs,
    media::{MediaSession, fit_dimensions},
    portal::CaptureInfo,
    protocol::{ClientMessage, PROTOCOL_VERSION, ServerMessage},
    shutdown,
};

#[derive(Debug)]
struct ServerState {
    config: ServeArgs,
    capture: CaptureInfo,
    token: String,
    connected: AtomicBool,
    failed_auth: AuthLimiter,
}

#[derive(Debug, Default)]
struct AuthLimiter(Mutex<HashMap<IpAddr, VecDeque<Instant>>>);

#[derive(Debug, PartialEq, Eq)]
struct ClientCapabilities {
    device: String,
    max_width: u32,
    max_height: u32,
    max_fps: u32,
}

#[derive(Debug, Default)]
struct HeartbeatState {
    nonce: u64,
    missed: u8,
}

impl HeartbeatState {
    fn next_ping(&mut self) -> Option<u64> {
        if self.missed >= 3 {
            return None;
        }
        self.nonce = self.nonce.wrapping_add(1);
        self.missed += 1;
        Some(self.nonce)
    }

    fn acknowledge(&mut self, nonce: u64) {
        if nonce == self.nonce {
            self.missed = 0;
        }
    }
}

fn validate_client_hello(
    message: ClientMessage,
    expected_token: &str,
) -> std::result::Result<ClientCapabilities, ServerMessage> {
    let ClientMessage::ClientHello {
        protocol,
        token,
        device,
        max_width,
        max_height,
        max_fps,
    } = message
    else {
        return Err(ServerMessage::error(
            "hello_required",
            "first message must be client_hello",
        ));
    };
    if token != expected_token {
        return Err(ServerMessage::error(
            "unauthorized",
            "invalid session token",
        ));
    }
    if protocol != PROTOCOL_VERSION {
        return Err(ServerMessage::error(
            "protocol_mismatch",
            format!("host requires protocol {PROTOCOL_VERSION}"),
        ));
    }
    Ok(ClientCapabilities {
        device,
        max_width,
        max_height,
        max_fps,
    })
}

impl AuthLimiter {
    const WINDOW: Duration = Duration::from_secs(60);
    const MAX_FAILURES: usize = 5;

    fn is_limited(&self, ip: IpAddr) -> bool {
        let mut failures = self.0.lock().expect("auth limiter poisoned");
        let entries = failures.entry(ip).or_default();
        entries.retain(|at| at.elapsed() < Self::WINDOW);
        entries.len() >= Self::MAX_FAILURES
    }

    fn record_failure(&self, ip: IpAddr) {
        let mut failures = self.0.lock().expect("auth limiter poisoned");
        let entries = failures.entry(ip).or_default();
        entries.retain(|at| at.elapsed() < Self::WINDOW);
        entries.push_back(Instant::now());
    }
}

pub async fn run(config: ServeArgs, capture: CaptureInfo) -> Result<()> {
    config.validate()?;
    let token: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect();
    let state = Arc::new(ServerState {
        config: config.clone(),
        capture,
        token: token.clone(),
        connected: AtomicBool::new(false),
        failed_auth: AuthLimiter::default(),
    });
    let app = Router::new()
        .route("/v1/session", get(upgrade))
        .with_state(state);
    let listener = TcpListener::bind(config.listen)
        .await
        .with_context(|| format!("failed to listen on {}", config.listen))?;

    println!("AuxScreen ready");
    println!("  endpoint: ws://{}/v1/session", config.listen);
    println!("  token:    {token}");
    println!(
        "  ICE UDP:  {}-{}",
        config.ice_ports.min, config.ice_ports.max
    );
    println!("Press Ctrl+C or send SIGTERM to stop and remove the virtual monitor.");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        if let Err(error) = shutdown::wait().await {
            tracing::warn!(%error, "failed to install shutdown signal handler");
        }
    })
    .await?;
    Ok(())
}

async fn upgrade(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<Arc<ServerState>>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| async move {
        if let Err(error) = handle_socket(state, peer.ip(), socket).await {
            tracing::error!(error = %format!("{error:#}"), "client session failed");
        }
    })
}

async fn handle_socket(
    state: Arc<ServerState>,
    peer_ip: IpAddr,
    mut socket: WebSocket,
) -> Result<()> {
    if state.failed_auth.is_limited(peer_ip) {
        send_json(
            &mut socket,
            &ServerMessage::error(
                "rate_limited",
                "too many invalid tokens; retry in one minute",
            ),
        )
        .await?;
        socket.close().await?;
        return Ok(());
    }
    if state
        .connected
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        send_json(
            &mut socket,
            &ServerMessage::error("busy", "another tablet is already connected"),
        )
        .await?;
        socket.close().await?;
        return Ok(());
    }
    struct ConnectionGuard<'a>(&'a AtomicBool);
    impl Drop for ConnectionGuard<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }
    let _guard = ConnectionGuard(&state.connected);

    let first = timeout(Duration::from_secs(10), socket.recv())
        .await
        .context("client hello timeout")?
        .ok_or_else(|| anyhow::anyhow!("client disconnected before hello"))??;
    let capabilities = match validate_client_hello(parse_text(first)?, &state.token) {
        Ok(capabilities) => capabilities,
        Err(error) => {
            if matches!(&error, ServerMessage::Error { code, .. } if code == "unauthorized") {
                state.failed_auth.record_failure(peer_ip);
            }
            send_json(&mut socket, &error).await?;
            socket.close().await?;
            return Ok(());
        }
    };
    let ClientCapabilities {
        device,
        max_width,
        max_height,
        max_fps,
    } = capabilities;
    tracing::info!(%device, max_width, max_height, max_fps, "tablet authenticated");

    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel();
    let mut media = MediaSession::new(&state.config, state.capture.clone(), outbound_tx.clone())?;
    let encoded = fit_dimensions(state.capture.size, state.config.encode_max_size);
    send_json(
        &mut socket,
        &ServerMessage::HelloAck {
            protocol: PROTOCOL_VERSION,
        },
    )
    .await?;
    send_json(
        &mut socket,
        &ServerMessage::StreamConfig {
            source_width: state.capture.size.0,
            source_height: state.capture.size.1,
            encoded_width: encoded.0,
            encoded_height: encoded.1,
            fps: state.config.fps,
            bitrate_kbps: state.config.bitrate_kbps,
        },
    )
    .await?;
    media.start()?;

    let mut bus_tick = interval(Duration::from_millis(50));
    bus_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut heartbeat = interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut heartbeat_state = HeartbeatState::default();
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(incoming) = incoming else { break; };
                match incoming? {
                    Message::Text(text) => {
                        if let Some(nonce) = handle_client_message(&media, &mut socket, serde_json::from_str(&text)?).await?
                        {
                            heartbeat_state.acknowledge(nonce);
                        }
                    },
                    Message::Ping(data) => socket.send(Message::Pong(data)).await?,
                    Message::Pong(_) => {},
                    Message::Close(_) => break,
                    Message::Binary(_) => send_json(&mut socket, &ServerMessage::error("text_only", "binary signaling is not supported")).await?,
                }
            }
            Some(outgoing) = outbound_rx.recv() => send_json(&mut socket, &outgoing).await?,
            _ = bus_tick.tick() => {
                while let Some(message) = media.pop_bus_message() {
                    if let Err(error) = media.handle_bus_message(&message) {
                        let details = error.to_string();
                        let negotiation_failed = details.contains("not-negotiated") || details.contains("not negotiated");
                        if negotiation_failed && !media.uses_gl_fallback() {
                            tracing::warn!(%details, "DMA-BUF negotiation failed; rebuilding with OpenGL bridge");
                            let replacement = MediaSession::new_with_gl(
                                &state.config,
                                state.capture.clone(),
                                outbound_tx.clone(),
                                true,
                            )?;
                            replacement.start()?;
                            media = replacement;
                        } else {
                            send_json(&mut socket, &ServerMessage::error("media_pipeline", &details)).await?;
                            return Err(error);
                        }
                    }
                }
            }
            _ = heartbeat.tick() => {
                let Some(nonce) = heartbeat_state.next_ping() else {
                    send_json(&mut socket, &ServerMessage::error("heartbeat_timeout", "client stopped responding")).await?;
                    bail!("client heartbeat timed out");
                };
                send_json(&mut socket, &ServerMessage::Ping { nonce }).await?;
            }
        }
    }
    Ok(())
}

async fn handle_client_message(
    media: &MediaSession,
    socket: &mut WebSocket,
    message: ClientMessage,
) -> Result<Option<u64>> {
    match message {
        ClientMessage::Answer { sdp } => media.set_answer(&sdp)?,
        ClientMessage::IceCandidate {
            candidate,
            sdp_mline_index,
            ..
        } => media.add_ice_candidate(sdp_mline_index, &candidate),
        ClientMessage::Ping { nonce } => send_json(socket, &ServerMessage::Pong { nonce }).await?,
        ClientMessage::Pong { nonce } => return Ok(Some(nonce)),
        ClientMessage::ClientStats {
            rendered_fps,
            frames_decoded,
            frames_dropped,
            bitrate_kbps,
            jitter_ms,
            rtt_ms,
            packets_received,
            packets_lost,
            decoder,
        } => tracing::info!(
            rendered_fps,
            ?frames_decoded,
            frames_dropped,
            ?bitrate_kbps,
            ?jitter_ms,
            ?rtt_ms,
            ?packets_received,
            ?packets_lost,
            ?decoder,
            "Android client stats"
        ),
        ClientMessage::ClientHello { .. } => {
            send_json(
                socket,
                &ServerMessage::error("duplicate_hello", "client_hello is only valid once"),
            )
            .await?
        }
    }
    Ok(None)
}

fn parse_text(message: Message) -> Result<ClientMessage> {
    match message {
        Message::Text(text) => Ok(serde_json::from_str(&text)?),
        _ => bail!("expected a JSON text message"),
    }
}

async fn send_json(socket: &mut WebSocket, message: &ServerMessage) -> Result<()> {
    socket
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use tokio_tungstenite::{connect_async, tungstenite::Message as ClientWsMessage};

    async fn websocket_error(state: Arc<ServerState>, payload: serde_json::Value) -> ServerMessage {
        let app = Router::new()
            .route("/v1/session", get(upgrade))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        let (mut client, _) = connect_async(format!("ws://{address}/v1/session"))
            .await
            .unwrap();
        client
            .send(ClientWsMessage::Text(payload.to_string().into()))
            .await
            .unwrap();
        let response = client.next().await.unwrap().unwrap();
        server.abort();
        serde_json::from_str(response.to_text().unwrap()).unwrap()
    }

    fn test_state() -> Arc<ServerState> {
        Arc::new(ServerState {
            config: ServeArgs {
                source: crate::cli::SourceArg::Test,
                listen: "127.0.0.1:0".parse().unwrap(),
                ice_ip: "192.168.1.254".into(),
                ice_ports: crate::cli::PortRange {
                    min: 9900,
                    max: 9910,
                },
                encode_max_size: (1920, 1200),
                fps: 30,
                bitrate_kbps: 6000,
                use_gl_fallback: false,
            },
            capture: CaptureInfo::test_pattern((1920, 1200)),
            token: "secret".into(),
            connected: AtomicBool::new(false),
            failed_auth: AuthLimiter::default(),
        })
    }

    #[test]
    fn rate_limits_only_the_failing_peer() {
        let limiter = AuthLimiter::default();
        let noisy = IpAddr::from([192, 168, 1, 7]);
        let other = IpAddr::from([192, 168, 1, 8]);
        for _ in 0..AuthLimiter::MAX_FAILURES {
            limiter.record_failure(noisy);
        }
        assert!(limiter.is_limited(noisy));
        assert!(!limiter.is_limited(other));
    }

    #[test]
    fn validates_first_message_token_and_protocol() {
        let hello = ClientMessage::ClientHello {
            protocol: PROTOCOL_VERSION,
            token: "secret".into(),
            device: "SM-X400".into(),
            max_width: 2112,
            max_height: 1320,
            max_fps: 60,
        };
        assert!(validate_client_hello(hello.clone(), "secret").is_ok());
        assert!(matches!(
            validate_client_hello(hello.clone(), "wrong"),
            Err(ServerMessage::Error { code, .. }) if code == "unauthorized"
        ));
        assert!(matches!(
            validate_client_hello(ClientMessage::Ping { nonce: 1 }, "secret"),
            Err(ServerMessage::Error { code, .. }) if code == "hello_required"
        ));
        let incompatible = ClientMessage::ClientHello {
            protocol: 999,
            token: "secret".into(),
            device: "SM-X400".into(),
            max_width: 2112,
            max_height: 1320,
            max_fps: 60,
        };
        assert!(matches!(
            validate_client_hello(incompatible, "secret"),
            Err(ServerMessage::Error { code, .. }) if code == "protocol_mismatch"
        ));
    }

    #[test]
    fn heartbeat_requires_matching_pong_and_times_out_after_three_misses() {
        let mut heartbeat = HeartbeatState::default();
        let first = heartbeat.next_ping().unwrap();
        heartbeat.acknowledge(first.wrapping_add(1));
        assert!(heartbeat.next_ping().is_some());
        assert!(heartbeat.next_ping().is_some());
        assert!(heartbeat.next_ping().is_none());

        heartbeat.acknowledge(heartbeat.nonce);
        assert!(heartbeat.next_ping().is_some());
    }

    #[tokio::test]
    async fn websocket_rejects_invalid_handshakes_and_releases_slot() {
        let state = test_state();
        for (payload, expected_code) in [
            (
                serde_json::json!({"type":"ping","nonce":1}),
                "hello_required",
            ),
            (
                serde_json::json!({
                    "type":"client_hello","protocol":1,"token":"wrong",
                    "device":"test","max_width":1920,"max_height":1200,"max_fps":30
                }),
                "unauthorized",
            ),
            (
                serde_json::json!({
                    "type":"client_hello","protocol":99,"token":"secret",
                    "device":"test","max_width":1920,"max_height":1200,"max_fps":30
                }),
                "protocol_mismatch",
            ),
        ] {
            let response = websocket_error(state.clone(), payload).await;
            assert!(matches!(response, ServerMessage::Error { code, .. } if code == expected_code));
        }
    }

    #[tokio::test]
    async fn websocket_enforces_busy_and_rate_limit() {
        let busy = test_state();
        busy.connected.store(true, Ordering::Release);
        let response = websocket_error(busy, serde_json::json!({"type":"ping","nonce":1})).await;
        assert!(matches!(response, ServerMessage::Error { code, .. } if code == "busy"));

        let limited = test_state();
        for _ in 0..AuthLimiter::MAX_FAILURES {
            let response = websocket_error(
                limited.clone(),
                serde_json::json!({
                    "type":"client_hello","protocol":1,"token":"wrong",
                    "device":"test","max_width":1920,"max_height":1200,"max_fps":30
                }),
            )
            .await;
            assert!(
                matches!(response, ServerMessage::Error { code, .. } if code == "unauthorized")
            );
        }
        let response = websocket_error(limited, serde_json::json!({"type":"ping","nonce":1})).await;
        assert!(matches!(response, ServerMessage::Error { code, .. } if code == "rate_limited"));
    }
}
