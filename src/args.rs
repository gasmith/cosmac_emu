//! Command-line arguments

use std::{path::PathBuf, sync::OnceLock, time::Duration};

use anyhow::{anyhow, Result};
use byte_unit::Byte;
use clap::Parser;
use regex::Regex;

/// Command line arguments.
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Args {
    /// Image file to load into RAM. May be provided multiple times. By default the image is loaded
    /// at base address 0x0000. To load an image at an alternative base address, specify the image
    /// as `<path>@0x0800`. Images are loaded in the order they are provided on the command line.
    #[arg(short, long, value_parser=parse_image)]
    pub image: Vec<Image>,

    /// The size of addressable memory attached to the 1802. Cannot exceed 64KiB.
    #[arg(short, long, value_parser=parse_bytes, default_value="64KiB")]
    pub memory_size: usize,

    /// An event log to replay during program execution.
    #[arg(long)]
    pub input_events: Option<PathBuf>,

    /// Machine cycle duration (e.g., 2us for a 4MHz clock).
    #[arg(short, long, value_parser=parse_duration::parse, default_value="2us")]
    pub cycle_time: Duration,
}

/// An image to be loaded at the specified base address.
#[derive(Debug, Clone)]
pub struct Image {
    /// Path to the image binary.
    pub path: PathBuf,
    /// Where to load the image.
    pub base_addr: u16,
}

/// Parses a byte size from a string with an optional unit specification.
fn parse_bytes(arg: &str) -> Result<usize> {
    let byte = Byte::parse_str(arg, true)?;
    let size = usize::try_from(byte.as_u64())?;
    if size > 64 * 1024 {
        Err(anyhow!("memory size cannot exceed 64KiB"))
    } else {
        Ok(size)
    }
}

/// Parses an image from a string with an optional base address.
fn parse_image(image: &str) -> Result<Image> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^([^@]+)(@0x([0-9af]+))?$").unwrap());
    let cap = re.captures(image).ok_or(anyhow!("invalid image"))?;
    let path = PathBuf::from(cap.get(1).unwrap().as_str());
    let base_addr = match cap.get(3) {
        Some(m) => u16::from_str_radix(m.as_str(), 16)?,
        None => 0,
    };
    Ok(Image { path, base_addr })
}
