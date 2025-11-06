//! Command definitions

use std::{
    path::PathBuf,
    sync::{LazyLock, OnceLock},
    time::Duration,
};

use byte_unit::Byte;
use clap::{Parser, Subcommand};
use color_eyre::{
    Result,
    eyre::{self, OptionExt},
};
use regex::Regex;

use crate::chips::cdp1802::MemoryRange;

mod dbg;
mod dis;
mod run;
mod tui;

use dbg::DbgArgs;
use dis::DisArgs;
use run::RunArgs;
use tui::TuiArgs;

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}
impl Cli {
    pub fn run(self) -> Result<()> {
        self.command.run()
    }
}

#[derive(Subcommand)]
pub enum Command {
    //Asm(AsmArgs),
    /// Debugger
    Dbg(DbgArgs),
    /// Disassembler
    Dis(DisArgs),
    /// Headless runner
    Run(RunArgs),
    /// Terminal UI
    Tui(TuiArgs),
}
impl Command {
    pub fn run(self) -> Result<()> {
        match self {
            //Command::Asm(_) => todo!(),
            Command::Dbg(args) => dbg::run(args),
            Command::Dis(args) => dis::run(args),
            Command::Run(args) => run::run(args),
            Command::Tui(args) => tui::run(args),
        }
    }
}

#[derive(Parser, Debug)]
struct CommonRunArgs {
    /// RAM image file to load.
    ///
    /// By default the image is loaded at base address 0x0000. To load an image at an alternative
    /// base address, specify the image as `<path>@0x8000`. RAM images are loaded in the order they
    /// are provided on the command line. RAM images are loaded before ROM images.
    #[arg(long, value_parser=parse_ram_image)]
    pub ram: Vec<ImageArg>,

    /// ROM image file to load.
    ///
    /// By default the image is loaded at base address 0x8000. To load an image at an alternative
    /// base address, specify the image as `<path>@0x0000`. ROM images are loaded in the order they
    /// are provided on the command line. ROM images are loaded after RAM images.
    ///
    /// ROM images are automatically write-protected, as if the user had given the --write-protect
    /// option.
    #[arg(long, value_parser=parse_rom_image)]
    pub rom: Vec<ImageArg>,

    /// Write protect region. May be provided multiple times.
    #[arg(short, long, value_parser=parse_memory_range)]
    pub write_protect: Vec<MemoryRange>,

    /// The size of addressable memory attached to the 1802. Must be a power of 2, and cannot
    /// exceed 64KiB.
    #[arg(short, long, value_parser=parse_memory_size, default_value="64KiB")]
    pub memory_size: usize,

    /// Clock frequency
    #[arg(long, default_value = "4MHz", value_parser=parse_hz)]
    pub clock_freq: u32,
}

fn parse_addr(s: &str) -> Result<u16> {
    if let Some(hex) = s.to_lowercase().strip_prefix("0x") {
        Ok(u16::from_str_radix(hex, 16)?)
    } else {
        Ok(s.parse::<u16>()?)
    }
}

fn parse_memory_range(s: &str) -> Result<MemoryRange> {
    static REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?x)
            ^
            (?P<start>0[xX][0-9A-Fa-f]+|\d+)?  # optional start (hex or decimal)
            \.\.                            # required range operator
            (?P<end>0[xX][0-9A-Fa-f]+|\d+)?    # optional end (hex or decimal)
            $",
        )
        .unwrap()
    });
    let caps = REGEX
        .captures(s)
        .ok_or_eyre("must of the form A..B, A.., ..B")?;
    let start = caps
        .name("start")
        .map(|m| parse_addr(m.as_str()))
        .transpose()?;
    let end = caps
        .name("end")
        .map(|m| parse_addr(m.as_str()))
        .transpose()?;
    Ok(MemoryRange { start, end })
}

/// Parses a frequency value.
fn parse_hz(s: &str) -> Result<u32> {
    static REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new("^([0-9.]+)([kmg])?(hz)?$").unwrap());
    let lower = s.to_lowercase();
    let cap = REGEX
        .captures(&lower)
        .ok_or_eyre("must match [0-9]+.[0-9]+[km]?(hz)? (case insensitive)")?;
    let base: f32 = cap.get(1).unwrap().as_str().parse()?;
    let factor = match cap.get(2).map(|s| s.as_str()) {
        None => 1.,
        Some("k") => 1_000.,
        Some("m") => 1_000_000.,
        Some(_) => unreachable!(),
    };
    let value = (base * factor).round() as u32;
    Ok(value)
}

/// An image to be loaded at the specified base address.
#[derive(Debug, Clone)]
pub struct ImageArg {
    /// Path to the image binary.
    pub path: PathBuf,
    /// Where to load the image.
    pub base_addr: u16,
    /// Whether the imgae should be write-protected.
    pub write_protect: bool,
}

/// Parses a byte size from a string with an optional unit specification.
fn parse_memory_size(arg: &str) -> Result<usize> {
    let byte = Byte::parse_str(arg, true)?;
    let size = usize::try_from(byte.as_u64())?;
    if size > 64 * 1024 {
        eyre::bail!("memory size cannot exceed 64KiB");
    }
    if size & (size - 1) != 0 {
        eyre::bail!("memory size must be a power of two");
    }
    Ok(size)
}

/// Parses a duration.
pub fn parse_duration(input: &str) -> Result<Duration> {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    let captures = REGEX
        .get_or_init(|| Regex::new(r"^([0-9]+)([nuμm]s|[smhd])?$").expect("valid regex"))
        .captures(input)
        .ok_or_eyre("invalid duration")?;
    let value: u64 = captures[1]
        .parse()
        .map_err(|e| eyre::eyre!("invalid duration value: {e}"))?;
    if value == 0 {
        eyre::bail!("duration must be non-zero")
    }
    Ok(match captures.get(2).map(|s| s.as_str()) {
        Some("ns") => Duration::from_nanos(value),
        Some("us" | "μs") => Duration::from_micros(value),
        Some("ms") => Duration::from_millis(value),
        Some("s") | None => Duration::from_secs(value),
        Some("m") => Duration::from_secs(value * 60),
        Some("h") => Duration::from_secs(value * 60 * 60),
        Some("d") => Duration::from_secs(value * 60 * 60 * 24),
        _ => unreachable!("unsupported suffix"),
    })
}

/// Parses an image from a string with an optional base address.
fn parse_image(image: &str, default_base_addr: u16, write_protect: bool) -> Result<ImageArg> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"^(?x)(?<path>[^@]+)
            (@(?<addr>(0x)?[0-9a-fA-F]+))?$",
        )
        .unwrap()
    });
    let cap = re.captures(image).ok_or_eyre("invalid image")?;
    let path = PathBuf::from(cap.name("path").unwrap().as_str());
    let base_addr = match cap.name("addr") {
        Some(m) => {
            let s = m.as_str();
            match s.strip_prefix("0x") {
                Some(hex) => u16::from_str_radix(hex, 16)?,
                None => s.parse()?,
            }
        }
        None => default_base_addr,
    };
    Ok(ImageArg {
        path,
        base_addr,
        write_protect,
    })
}

/// Parses an RAM image from a string.
fn parse_ram_image(image: &str) -> Result<ImageArg> {
    parse_image(image, 0, false)
}

/// Parses an ROM image from a string.
fn parse_rom_image(image: &str) -> Result<ImageArg> {
    parse_image(image, 0x8000, true)
}
