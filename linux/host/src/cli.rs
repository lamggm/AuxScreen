use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "auxscreen-host", version, about = "AuxScreen Linux host")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate the local KDE/Wayland, portal, GStreamer and network stack.
    Doctor(DoctorArgs),
    /// Open a local preview of a virtual monitor or test pattern.
    Preview(PreviewArgs),
    /// Stream a virtual monitor or test pattern to one Android client.
    Serve(ServeArgs),
}

#[derive(Debug, Clone, Args)]
pub struct DoctorArgs {
    #[arg(long, default_value = "192.168.1.254:9898")]
    pub listen: SocketAddr,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum SourceArg {
    Virtual,
    Monitor,
    Test,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum EncoderArg {
    Auto,
    Nvenc,
    X264,
}

#[derive(Debug, Clone, Args)]
pub struct PreviewArgs {
    #[arg(long, value_enum, default_value_t = SourceArg::Virtual)]
    pub source: SourceArg,
}

#[derive(Debug, Clone, Args)]
pub struct ServeArgs {
    #[arg(long, value_enum, default_value_t = SourceArg::Virtual)]
    pub source: SourceArg,

    #[arg(long, default_value = "192.168.1.254:9898")]
    pub listen: SocketAddr,

    #[arg(long, default_value = "192.168.1.254")]
    pub ice_ip: String,

    #[arg(long, default_value = "9900-9910", value_parser = parse_port_range)]
    pub ice_ports: PortRange,

    #[arg(long, default_value = "1920x1200", value_parser = parse_size)]
    pub encode_max_size: (u32, u32),

    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u32).range(1..=60))]
    pub fps: u32,

    #[arg(long, default_value_t = 6000, value_parser = clap::value_parser!(u32).range(500..=50000))]
    pub bitrate_kbps: u32,

    /// Select the H.264 encoder. Auto prefers NVIDIA NVENC when available.
    #[arg(long, value_enum, default_value_t = EncoderArg::Auto)]
    pub encoder: EncoderArg,

    /// Use an OpenGL upload/download bridge for PipeWire DMA-BUF negotiation.
    #[arg(long, default_value_t = false)]
    pub use_gl_fallback: bool,

    /// Disable session-token validation for local LAN testing.
    #[arg(long, default_value_t = false)]
    pub no_auth: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortRange {
    pub min: u16,
    pub max: u16,
}

fn parse_port_range(value: &str) -> Result<PortRange, String> {
    let (min, max) = value
        .split_once('-')
        .ok_or_else(|| "expected MIN-MAX".to_string())?;
    let min = u16::from_str(min).map_err(|_| "invalid minimum port".to_string())?;
    let max = u16::from_str(max).map_err(|_| "invalid maximum port".to_string())?;
    if min == 0 || min > max {
        return Err("port range must be non-zero and ascending".to_string());
    }
    Ok(PortRange { min, max })
}

fn parse_size(value: &str) -> Result<(u32, u32), String> {
    let (width, height) = value
        .split_once('x')
        .ok_or_else(|| "expected WIDTHxHEIGHT".to_string())?;
    let width = width
        .parse::<u32>()
        .map_err(|_| "invalid width".to_string())?;
    let height = height
        .parse::<u32>()
        .map_err(|_| "invalid height".to_string())?;
    if width < 320 || height < 240 {
        return Err("size must be at least 320x240".to_string());
    }
    Ok((width, height))
}

impl ServeArgs {
    pub fn validate(&self) -> Result<()> {
        if self.listen.ip().is_unspecified() {
            bail!(
                "refusing to bind signaling to every interface; choose the LAN address explicitly"
            );
        }
        let parsed_ip = self
            .ice_ip
            .parse::<std::net::IpAddr>()
            .with_context(|| format!("invalid ICE IP {}", self.ice_ip))?;
        if !is_private_lan_ip(parsed_ip) || parsed_ip.is_loopback() {
            bail!("ICE IP must be a concrete private LAN address");
        }
        if !is_private_lan_ip(self.listen.ip()) {
            bail!("signaling must listen on a private LAN address");
        }
        Ok(())
    }
}

pub fn is_private_lan_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_port_range() {
        assert_eq!(
            parse_port_range("9900-9910").unwrap(),
            PortRange {
                min: 9900,
                max: 9910
            }
        );
        assert!(parse_port_range("9910-9900").is_err());
    }

    #[test]
    fn parses_size() {
        assert_eq!(parse_size("1920x1200").unwrap(), (1920, 1200));
        assert!(parse_size("tiny").is_err());
    }

    #[test]
    fn recognizes_private_lan_addresses() {
        assert!(is_private_lan_ip(IpAddr::from([192, 168, 1, 254])));
        assert!(is_private_lan_ip(IpAddr::from([10, 0, 0, 1])));
        assert!(is_private_lan_ip(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)));
        assert!(!is_private_lan_ip(IpAddr::from([8, 8, 8, 8])));
    }
}
