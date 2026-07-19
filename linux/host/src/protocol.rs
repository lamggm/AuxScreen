use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    ClientHello {
        protocol: u32,
        token: String,
        device: String,
        max_width: u32,
        max_height: u32,
        max_fps: u32,
    },
    Answer {
        sdp: String,
    },
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: u32,
    },
    Ping {
        nonce: u64,
    },
    Pong {
        nonce: u64,
    },
    ClientStats {
        rendered_fps: f64,
        #[serde(default)]
        frames_decoded: Option<u64>,
        frames_dropped: u64,
        #[serde(default)]
        bitrate_kbps: Option<f64>,
        #[serde(default)]
        jitter_ms: Option<f64>,
        #[serde(default)]
        rtt_ms: Option<f64>,
        #[serde(default)]
        packets_received: Option<u64>,
        #[serde(default)]
        packets_lost: Option<i64>,
        decoder: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    HelloAck {
        protocol: u32,
    },
    Offer {
        sdp: String,
    },
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: u32,
    },
    StreamConfig {
        source_width: u32,
        source_height: u32,
        encoded_width: u32,
        encoded_height: u32,
        fps: u32,
        bitrate_kbps: u32,
    },
    Ping {
        nonce: u64,
    },
    Pong {
        nonce: u64,
    },
    Error {
        code: String,
        message: String,
    },
}

impl ServerMessage {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_round_trip() {
        let message = ClientMessage::ClientHello {
            protocol: PROTOCOL_VERSION,
            token: "secret".into(),
            device: "SM-X400".into(),
            max_width: 2112,
            max_height: 1320,
            max_fps: 60,
        };
        let encoded = serde_json::to_string(&message).unwrap();
        assert_eq!(
            serde_json::from_str::<ClientMessage>(&encoded).unwrap(),
            message
        );
    }

    #[test]
    fn rejects_unknown_message() {
        assert!(serde_json::from_str::<ClientMessage>(r#"{"type":"telepathy"}"#).is_err());
    }

    #[test]
    fn accepts_legacy_client_stats() {
        let message =
            r#"{"type":"client_stats","rendered_fps":29.5,"frames_dropped":0,"decoder":null}"#;
        assert!(serde_json::from_str::<ClientMessage>(message).is_ok());
    }

    #[test]
    fn shared_fixtures_match_rust_messages() {
        let hello = include_str!("../../../protocol/fixtures/client_hello.json");
        let stats = include_str!("../../../protocol/fixtures/client_stats.json");
        let config = include_str!("../../../protocol/fixtures/stream_config.json");
        let error = include_str!("../../../protocol/fixtures/error.json");
        assert!(matches!(
            serde_json::from_str::<ClientMessage>(hello).unwrap(),
            ClientMessage::ClientHello { .. }
        ));
        assert!(matches!(
            serde_json::from_str::<ClientMessage>(stats).unwrap(),
            ClientMessage::ClientStats { .. }
        ));
        assert!(matches!(
            serde_json::from_str::<ServerMessage>(config).unwrap(),
            ServerMessage::StreamConfig { .. }
        ));
        assert!(matches!(
            serde_json::from_str::<ServerMessage>(error).unwrap(),
            ServerMessage::Error { .. }
        ));
    }
}
