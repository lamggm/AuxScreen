mod cli;
mod doctor;
mod media;
mod portal;
mod preview;
mod protocol;
mod server;
mod shutdown;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, SourceArg};
use portal::CaptureInfo;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("auxscreen_host=info")),
        )
        .with_target(false)
        .init();
    gst::init()?;

    match Cli::parse().command {
        Command::Doctor(args) => doctor::run(args).await,
        Command::Preview(args) => match args.source {
            SourceArg::Test => {
                preview::run(SourceArg::Test, CaptureInfo::test_pattern((1920, 1200))).await
            }
            SourceArg::Virtual => {
                portal::with_virtual_capture(|capture| preview::run(SourceArg::Virtual, capture))
                    .await
            }
            SourceArg::Monitor => {
                portal::with_monitor_capture(|capture| preview::run(SourceArg::Monitor, capture))
                    .await
            }
        },
        Command::Serve(args) => match args.source {
            SourceArg::Test => {
                let size = args.encode_max_size;
                server::run(args, CaptureInfo::test_pattern(size)).await
            }
            SourceArg::Virtual => {
                portal::with_virtual_capture(|capture| server::run(args, capture)).await
            }
            SourceArg::Monitor => {
                portal::with_monitor_capture(|capture| server::run(args, capture)).await
            }
        },
    }
}
