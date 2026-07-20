use std::io;

use tokio::signal::{
    ctrl_c,
    unix::{SignalKind, signal},
};

pub async fn wait() -> io::Result<()> {
    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        result = ctrl_c() => result,
        _ = terminate.recv() => Ok(()),
    }
}
