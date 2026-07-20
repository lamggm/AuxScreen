use std::os::fd::AsRawFd;

use anyhow::{Result, anyhow, bail};
use gst::prelude::*;
use tokio::time::{Duration, interval};

use crate::{cli::SourceArg, portal::CaptureInfo, shutdown};

pub async fn run(source: SourceArg, capture: CaptureInfo) -> Result<()> {
    let source = match source {
        SourceArg::Test => "videotestsrc pattern=ball is-live=true do-timestamp=true".to_string(),
        SourceArg::Virtual | SourceArg::Monitor => {
            let fd = capture
                .pipewire_fd
                .as_ref()
                .ok_or_else(|| anyhow!("missing PipeWire fd"))?;
            let node = capture
                .node_id
                .ok_or_else(|| anyhow!("missing PipeWire node"))?;
            format!(
                "pipewiresrc fd={} path={} do-timestamp=true keepalive-time=16",
                fd.as_raw_fd(),
                node
            )
        }
    };
    let description = format!(
        "{source} ! queue max-size-buffers=2 leaky=downstream ! videoconvert ! glimagesink sync=false"
    );
    let pipeline = gst::parse::launch(&description)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow!("preview did not create a pipeline"))?;
    pipeline.set_state(gst::State::Playing)?;
    println!(
        "Preview running at {}x{}. Press Ctrl+C or send SIGTERM to stop.",
        capture.size.0, capture.size.1
    );

    let bus = pipeline.bus().expect("pipeline has a bus");
    let mut tick = interval(Duration::from_millis(50));
    loop {
        tokio::select! {
            signal = shutdown::wait() => {
                signal?;
                break;
            },
            _ = tick.tick() => {
                while let Some(message) = bus.pop() {
                    use gst::message::MessageView;
                    match message.view() {
                        MessageView::Error(error) => bail!("preview error: {} ({})", error.error(), error.debug().unwrap_or_else(|| "no details".into())),
                        MessageView::Eos(_) => break,
                        _ => {}
                    }
                }
            }
        }
    }
    let _ = pipeline.set_state(gst::State::Null);
    Ok(())
}
