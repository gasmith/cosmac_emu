use clap::Parser;
use color_eyre::{Result, eyre};

use crate::tui::mc::MembershipCardTui;
use crate::{chips::cdp1802::Memory, command::parse_addr, systems::mc::MembershipCard};

use super::CommonRunArgs;

#[derive(Parser, Debug)]
pub struct TuiArgs {
    #[command(flatten)]
    common: CommonRunArgs,

    /// The speed at which the emulator will run, expressed as a positive floating point value.
    ///
    /// If not specified, the processor runs as fast as the emulation will allow.
    #[arg(long, value_parser=parse_speed)]
    speed: Option<f64>,

    /// UART baud rate.
    #[arg(long, default_value = "4800")]
    uart_baud: u16,

    /// Address to jump to during reset.
    #[arg(long, value_parser=parse_addr)]
    jump_to: Option<u16>,

    /// Whether to invert EF3.
    #[arg(long)]
    invert_ef3: bool,

    /// Whether to invert Q.
    #[arg(long)]
    invert_q: bool,
}

fn parse_speed(s: &str) -> Result<f64> {
    let speed: f64 = s.parse()?;
    if speed <= 0. {
        eyre::bail!("speed must be positive");
    }
    Ok(speed)
}

pub fn run(args: TuiArgs) -> Result<()> {
    let mut builder = Memory::builder()
        .with_capacity(args.common.memory_size)?
        .with_image_args(&args.common.ram)?
        .with_image_args(&args.common.rom)?
        .with_write_protect_ranges(&args.common.write_protect);
    if let Some(addr) = args.jump_to {
        let hi = ((addr & 0xff00) >> 8) as u8;
        let lo = (addr & 0xff) as u8;
        builder = builder.with_image(0, [0xc0, hi, lo]);
    }
    let memory = builder.build()?;
    let mc = MembershipCard::builder()
        .with_clock_freq(args.common.clock_freq)
        .with_invert_ef3(args.invert_ef3)
        .with_invert_q(args.invert_q)
        .with_memory(memory)
        .with_speed(args.speed)
        .with_uart_baud(args.uart_baud)
        .build();
    let tui = MembershipCardTui::new(mc);
    tui.run()
}
