use std::{future::Future, os::fd::OwnedFd, sync::Arc};

use anyhow::{Context, Result, anyhow, bail};
use ashpd::desktop::{
    PersistMode,
    screencast::{
        CursorMode, OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType,
    },
};
use ashpd::enumflags2::BitFlags;
use tokio::time::{Duration, timeout};

const PORTAL_APPROVAL_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct CaptureInfo {
    pub pipewire_fd: Option<Arc<OwnedFd>>,
    pub node_id: Option<u32>,
    pub size: (u32, u32),
}

impl CaptureInfo {
    pub fn test_pattern(size: (u32, u32)) -> Self {
        Self {
            pipewire_fd: None,
            node_id: None,
            size,
        }
    }
}

pub async fn with_virtual_capture<T, F, Fut>(callback: F) -> Result<T>
where
    F: FnOnce(CaptureInfo) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let portal = Screencast::new()
        .await
        .context("failed to connect to ScreenCast portal")?;
    let available = portal
        .available_source_types()
        .await
        .context("failed to query portal source types")?;
    if !available.contains(SourceType::Virtual) {
        bail!("ScreenCast portal does not advertise VIRTUAL sources");
    }
    let session = portal
        .create_session(Default::default())
        .await
        .context("failed to create portal session")?;

    portal
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Hidden)
                .set_sources(BitFlags::from_flag(SourceType::Virtual))
                .set_multiple(false)
                .set_persist_mode(PersistMode::DoNot),
        )
        .await
        .context("failed to request a virtual source")?;

    println!("Waiting for KDE to approve the virtual monitor…");
    let response = timeout(PORTAL_APPROVAL_TIMEOUT, async {
        portal
            .start(&session, None, Default::default())
            .await
            .context("failed to start portal request")?
            .response()
            .context("the ScreenCast portal request was cancelled or rejected")
    })
    .await
    .context("portal approval timed out after 60 seconds")??;
    let stream = response
        .streams()
        .first()
        .ok_or_else(|| anyhow!("portal returned no PipeWire stream"))?;
    if stream.source_type() != Some(SourceType::Virtual) {
        bail!(
            "portal returned {:?} instead of a virtual monitor",
            stream.source_type()
        );
    }

    let size = stream
        .size()
        .map(|(width, height)| (width as u32, height as u32))
        .unwrap_or((1920, 1080));
    let remote_fd = portal
        .open_pipe_wire_remote(&session, OpenPipeWireRemoteOptions::default())
        .await
        .context("failed to open the restricted PipeWire remote")?;
    let info = CaptureInfo {
        pipewire_fd: Some(Arc::new(remote_fd)),
        node_id: Some(stream.pipe_wire_node_id()),
        size,
    };

    tracing::info!(
        node_id = stream.pipe_wire_node_id(),
        width = size.0,
        height = size.1,
        "virtual monitor ready"
    );
    let result = callback(info).await;
    if let Err(error) = session.close().await {
        tracing::warn!(%error, "failed to close portal session cleanly");
    }
    result
}
