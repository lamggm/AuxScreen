use std::{
    os::fd::AsRawFd,
    sync::{Arc, Mutex, Weak},
};

use anyhow::{Context, Result, anyhow, bail};
use gst::{glib, prelude::*};
use gst_webrtc::prelude::WebRTCICEExt;
use tokio::sync::mpsc;

use crate::{cli::ServeArgs, portal::CaptureInfo, protocol::ServerMessage};

#[derive(Debug, Clone)]
pub struct MediaSession(Arc<MediaInner>);

#[derive(Debug)]
struct MediaInner {
    pipeline: gst::Pipeline,
    webrtcbin: gst::Element,
    outbound: Mutex<mpsc::UnboundedSender<ServerMessage>>,
    _capture: CaptureInfo,
    uses_gl_fallback: bool,
}

#[derive(Debug, Clone)]
struct MediaWeak(Weak<MediaInner>);

impl MediaWeak {
    fn upgrade(&self) -> Option<MediaSession> {
        self.0.upgrade().map(MediaSession)
    }
}

impl MediaSession {
    pub fn new(
        config: &ServeArgs,
        capture: CaptureInfo,
        outbound: mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Self> {
        Self::new_with_gl(config, capture, outbound, config.use_gl_fallback)
    }

    pub fn new_with_gl(
        config: &ServeArgs,
        capture: CaptureInfo,
        outbound: mpsc::UnboundedSender<ServerMessage>,
        uses_gl_fallback: bool,
    ) -> Result<Self> {
        let encoded = fit_dimensions(capture.size, config.encode_max_size);
        let source = source_description(config, &capture)?;
        let bridge = if uses_gl_fallback {
            "glupload ! glcolorconvert ! gldownload !"
        } else {
            "videoconvert !"
        };
        let description = format!(
            "{source} ! queue max-size-buffers=2 leaky=downstream ! {bridge} videoscale ! \
             video/x-raw,format=I420,width={},height={},framerate={}/1 ! \
             x264enc tune=zerolatency speed-preset=ultrafast bitrate={} key-int-max={} bframes=0 byte-stream=false aud=true ! \
             video/x-h264,profile=constrained-baseline,stream-format=avc,alignment=au ! \
             h264parse config-interval=-1 ! rtph264pay name=vpay pt=96 mtu=1200 config-interval=-1 ! \
             application/x-rtp,media=video,encoding-name=H264,payload=96,clock-rate=90000 ! webrtc. \
             webrtcbin name=webrtc bundle-policy=max-bundle latency=50",
            encoded.0, encoded.1, config.fps, config.bitrate_kbps, config.fps
        );
        tracing::debug!(%description, "building media pipeline");
        let pipeline = gst::parse::launch(&description)?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow!("GStreamer description did not create a pipeline"))?;
        let webrtcbin = pipeline
            .by_name("webrtc")
            .ok_or_else(|| anyhow!("webrtcbin missing from pipeline"))?;

        let ice = webrtcbin.property::<gst_webrtc::WebRTCICE>("ice-agent");
        ice.set_property("min-rtp-port", config.ice_ports.min as u32);
        ice.set_property("max-rtp-port", config.ice_ports.max as u32);
        if !ice.emit_add_local_ip_address(&config.ice_ip) {
            bail!("GStreamer rejected ICE address {}", config.ice_ip);
        }

        let session = Self(Arc::new(MediaInner {
            pipeline,
            webrtcbin,
            outbound: Mutex::new(outbound),
            _capture: capture,
            uses_gl_fallback,
        }));
        session.connect_signals();
        Ok(session)
    }

    pub fn uses_gl_fallback(&self) -> bool {
        self.0.uses_gl_fallback
    }

    fn downgrade(&self) -> MediaWeak {
        MediaWeak(Arc::downgrade(&self.0))
    }

    fn connect_signals(&self) {
        let weak = self.downgrade();
        self.0.webrtcbin.connect_closure(
            "on-negotiation-needed",
            false,
            glib::closure!(move |_webrtcbin: &gst::Element| {
                if let Some(session) = weak.upgrade()
                    && let Err(error) = session.create_offer()
                {
                    session.send(ServerMessage::error("offer_failed", error.to_string()));
                }
            }),
        );

        let weak = self.downgrade();
        self.0.webrtcbin.connect_closure(
            "on-ice-candidate",
            false,
            glib::closure!(
                move |_webrtcbin: &gst::Element, index: u32, candidate: &str| {
                    if let Some(session) = weak.upgrade() {
                        session.send(ServerMessage::IceCandidate {
                            candidate: candidate.to_string(),
                            sdp_mid: Some("0".to_string()),
                            sdp_mline_index: index,
                        });
                    }
                }
            ),
        );
    }

    pub fn start(&self) -> Result<()> {
        self.0
            .pipeline
            .set_state(gst::State::Playing)
            .context("failed to start media pipeline")?;
        Ok(())
    }

    fn create_offer(&self) -> Result<()> {
        let weak = self.downgrade();
        let promise = gst::Promise::with_change_func(move |reply| {
            let Some(session) = weak.upgrade() else {
                return;
            };
            if let Err(error) = session.offer_created(reply) {
                session.send(ServerMessage::error("offer_failed", error.to_string()));
            }
        });
        self.0
            .webrtcbin
            .emit_by_name::<()>("create-offer", &[&None::<gst::Structure>, &promise]);
        Ok(())
    }

    fn offer_created(
        &self,
        reply: Result<Option<&gst::StructureRef>, gst::PromiseError>,
    ) -> Result<()> {
        let reply = reply
            .map_err(|error| anyhow!("offer promise failed: {error:?}"))?
            .ok_or_else(|| anyhow!("offer promise returned no structure"))?;
        let offer = reply
            .value("offer")?
            .get::<gst_webrtc::WebRTCSessionDescription>()
            .context("invalid offer returned by webrtcbin")?;
        self.0
            .webrtcbin
            .emit_by_name::<()>("set-local-description", &[&offer, &None::<gst::Promise>]);
        self.send(ServerMessage::Offer {
            sdp: offer.sdp().as_text()?,
        });
        Ok(())
    }

    pub fn set_answer(&self, sdp: &str) -> Result<()> {
        let parsed = gst_sdp::SDPMessage::parse_buffer(sdp.as_bytes())
            .map_err(|_| anyhow!("failed to parse SDP answer"))?;
        let answer =
            gst_webrtc::WebRTCSessionDescription::new(gst_webrtc::WebRTCSDPType::Answer, parsed);
        self.0
            .webrtcbin
            .emit_by_name::<()>("set-remote-description", &[&answer, &None::<gst::Promise>]);
        Ok(())
    }

    pub fn add_ice_candidate(&self, index: u32, candidate: &str) {
        self.0
            .webrtcbin
            .emit_by_name::<()>("add-ice-candidate", &[&index, &candidate]);
    }

    pub fn pop_bus_message(&self) -> Option<gst::Message> {
        self.0.pipeline.bus().and_then(|bus| bus.pop())
    }

    pub fn handle_bus_message(&self, message: &gst::Message) -> Result<()> {
        use gst::message::MessageView;
        match message.view() {
            MessageView::Error(error) => bail!(
                "GStreamer error from {}: {} ({})",
                error
                    .src()
                    .map(|src| src.path_string())
                    .unwrap_or_else(|| "unknown".into()),
                error.error(),
                error.debug().unwrap_or_else(|| "no details".into())
            ),
            MessageView::Warning(warning) => tracing::warn!(
                source = %warning.src().map(|src| src.path_string()).unwrap_or_else(|| "unknown".into()),
                details = %warning.debug().unwrap_or_else(|| "no details".into()),
                "GStreamer warning"
            ),
            MessageView::Latency(_) => {
                let _ = self.0.pipeline.recalculate_latency();
            }
            _ => {}
        }
        Ok(())
    }

    fn send(&self, message: ServerMessage) {
        let _ = self
            .0
            .outbound
            .lock()
            .expect("outbound channel poisoned")
            .send(message);
    }
}

impl Drop for MediaInner {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn source_description(config: &ServeArgs, capture: &CaptureInfo) -> Result<String> {
    if let (Some(fd), Some(node_id)) = (&capture.pipewire_fd, capture.node_id) {
        Ok(format!(
            "pipewiresrc fd={} path={} do-timestamp=true keepalive-time=16",
            fd.as_raw_fd(),
            node_id
        ))
    } else if config.source == crate::cli::SourceArg::Test {
        Ok("videotestsrc pattern=ball is-live=true do-timestamp=true".to_string())
    } else {
        bail!("virtual source is missing its PipeWire descriptor")
    }
}

pub fn fit_dimensions(source: (u32, u32), maximum: (u32, u32)) -> (u32, u32) {
    let (source_width, source_height) = source;
    let (max_width, max_height) = maximum;
    if source_width <= max_width && source_height <= max_height {
        return (source_width & !1, source_height & !1);
    }
    let scale =
        (max_width as f64 / source_width as f64).min(max_height as f64 / source_height as f64);
    let width = ((source_width as f64 * scale).floor() as u32).max(2) & !1;
    let height = ((source_height as f64 * scale).floor() as u32).max(2) & !1;
    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_sixteen_ten() {
        assert_eq!(fit_dimensions((2112, 1320), (1920, 1200)), (1920, 1200));
    }

    #[test]
    fn preserves_sixteen_nine() {
        assert_eq!(fit_dimensions((2560, 1440), (1920, 1200)), (1920, 1080));
    }

    #[test]
    fn keeps_small_even_source() {
        assert_eq!(fit_dimensions((1280, 800), (1920, 1200)), (1280, 800));
    }
}
