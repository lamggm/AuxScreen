use std::{
    env,
    fs::{self, OpenOptions},
    future::Future,
    io::Write,
    os::{fd::OwnedFd, unix::fs::OpenOptionsExt},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context, Result, bail};
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
    with_capture(SourceType::Virtual, "virtual monitor", "virtual", callback).await
}

pub async fn with_monitor_capture<T, F, Fut>(callback: F) -> Result<T>
where
    F: FnOnce(CaptureInfo) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    with_capture(SourceType::Monitor, "physical monitor", "monitor", callback).await
}

async fn with_capture<T, F, Fut>(
    source_type: SourceType,
    source_label: &'static str,
    source_key: &'static str,
    callback: F,
) -> Result<T>
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
    if !available.contains(source_type) {
        bail!("ScreenCast portal does not advertise {source_label} sources");
    }
    let session = portal
        .create_session(Default::default())
        .await
        .context("failed to create portal session")?;
    let restore_token = match load_restore_token(source_key) {
        Ok(token) => token,
        Err(error) => {
            tracing::warn!(%error, "failed to read portal restore token");
            None
        }
    };

    portal
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Embedded)
                .set_sources(BitFlags::from_flag(source_type))
                // Plasma 6.7.3 with Qt 6.11 can abort when a single-select
                // delegate accepts the dialog synchronously from its click
                // handler. Multi-select uses the dialog's Share button and
                // avoids that upstream crash. We still enforce exactly one
                // stream below, so the host remains single-source.
                .set_multiple(true)
                .set_restore_token(restore_token.as_deref())
                .set_persist_mode(PersistMode::ExplicitlyRevoked),
        )
        .await
        .with_context(|| format!("failed to request a {source_label} source"))?;

    println!("Waiting for KDE to approve the {source_label}…");
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
    if let Some(token) = response.restore_token()
        && let Err(error) = save_restore_token(source_key, token)
    {
        tracing::warn!(%error, "failed to persist portal restore token");
    }
    validate_stream_count(response.streams().len())?;
    let stream = &response.streams()[0];
    if stream.source_type() != Some(source_type) {
        bail!(
            "portal returned {:?} instead of the requested {source_label}",
            stream.source_type(),
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
        source = source_label,
        "capture source ready"
    );
    let result = callback(info).await;
    if let Err(error) = session.close().await {
        tracing::warn!(%error, "failed to close portal session cleanly");
    }
    result
}

fn restore_token_path(source_key: &str) -> Option<PathBuf> {
    let state_root = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))?;
    Some(
        state_root
            .join("auxscreen")
            .join(format!("portal-{source_key}.token")),
    )
}

fn load_restore_token(source_key: &str) -> Result<Option<String>> {
    let Some(path) = restore_token_path(source_key) else {
        return Ok(None);
    };
    match fs::read_to_string(&path) {
        Ok(token) if !token.trim().is_empty() => Ok(Some(token.trim().to_owned())),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn save_restore_token(source_key: &str, token: &str) -> Result<()> {
    let path =
        restore_token_path(source_key).context("HOME and XDG_STATE_HOME are both unavailable")?;
    let parent = path.parent().context("restore token path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    file.write_all(token.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn validate_stream_count(count: usize) -> Result<()> {
    match count {
        1 => Ok(()),
        0 => bail!("portal returned no PipeWire stream"),
        count => bail!("portal returned {count} streams; AuxScreen requires exactly one"),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_stream_count;

    #[test]
    fn accepts_exactly_one_portal_stream() {
        assert!(validate_stream_count(1).is_ok());
    }

    #[test]
    fn rejects_zero_or_multiple_portal_streams() {
        assert_eq!(
            validate_stream_count(0).unwrap_err().to_string(),
            "portal returned no PipeWire stream"
        );
        assert_eq!(
            validate_stream_count(2).unwrap_err().to_string(),
            "portal returned 2 streams; AuxScreen requires exactly one"
        );
    }
}
