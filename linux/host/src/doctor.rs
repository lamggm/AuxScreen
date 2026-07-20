use std::{net::TcpListener, process::Command};

use anyhow::{Context, Result, bail};
use zbus::{Connection, Proxy};

use crate::cli::DoctorArgs;

const PORTAL_DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";

pub async fn run(args: DoctorArgs) -> Result<()> {
    gst::init().context("failed to initialize GStreamer")?;

    let mut failures = Vec::new();
    println!("AuxScreen doctor v{}", env!("CARGO_PKG_VERSION"));

    match portal_capabilities().await {
        Ok((version, types)) => {
            let virtual_supported = types & 4 != 0;
            println!(
                "[ok] ScreenCast portal v{version}, source mask={types} (virtual={virtual_supported})"
            );
            if !virtual_supported {
                failures.push("portal does not advertise VIRTUAL source support".to_string());
            }
        }
        Err(error) => failures.push(format!("portal unavailable: {error:#}")),
    }

    for element in [
        "pipewiresrc",
        "webrtcbin",
        "x264enc",
        "h264parse",
        "rtph264pay",
        "videorate",
    ] {
        if gst::ElementFactory::find(element).is_some() {
            println!("[ok] GStreamer element {element}");
        } else {
            failures.push(format!("missing GStreamer element {element}"));
        }
    }

    match TcpListener::bind(args.listen) {
        Ok(listener) => {
            println!("[ok] signaling address {} is available", args.listen);
            drop(listener);
        }
        Err(error) => failures.push(format!("cannot bind {}: {error}", args.listen)),
    }

    match Command::new("nvidia-smi")
        .args(["--query-gpu=name,driver_version", "--format=csv,noheader"])
        .output()
    {
        Ok(output) if output.status.success() => {
            println!(
                "[ok] GPU {}",
                String::from_utf8_lossy(&output.stdout).trim()
            );
        }
        Ok(output) => println!(
            "[warn] nvidia-smi: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => println!("[warn] nvidia-smi unavailable: {error}"),
    }

    if failures.is_empty() {
        println!("doctor result: ready");
        Ok(())
    } else {
        for failure in &failures {
            eprintln!("[fail] {failure}");
        }
        bail!("doctor found {} blocking problem(s)", failures.len())
    }
}

async fn portal_capabilities() -> Result<(u32, u32)> {
    let connection = Connection::session().await?;
    let proxy = Proxy::new(
        &connection,
        PORTAL_DESTINATION,
        PORTAL_PATH,
        SCREENCAST_INTERFACE,
    )
    .await?;
    let version = proxy.get_property::<u32>("version").await?;
    let types = proxy.get_property::<u32>("AvailableSourceTypes").await?;
    Ok((version, types))
}
