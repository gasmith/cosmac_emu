use clap::Parser;
use color_eyre::{Result, eyre};

use crate::chips::ay51013::Ay51013Uart;
use crate::tui::mc::MembershipCardTui;
use crate::uart::UartMode;
use crate::{chips::cdp1802::Memory, cli::parse_addr, systems::mc::MembershipCard};

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

    /// UART mode.
    #[arg(long, default_value = "8n1")]
    uart_mode: UartMode,

    /// Forcibly interpret 8n1 bytes as 7-bit ascii, by ignoring the msb.
    #[arg(long)]
    uart_force_7bit_ascii: bool,

    /// Address to jump to during reset.
    #[arg(long, value_parser=parse_addr)]
    jump_to: Option<u16>,

    /// Whether to invert EF.
    #[arg(long)]
    invert_ef: bool,

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
    let uart = Ay51013Uart::builder()
        .with_baud(args.uart_baud, args.common.clock_freq)
        .with_mode(args.uart_mode)
        .with_force_7bit_ascii(args.uart_force_7bit_ascii)
        .build();
    let mc = MembershipCard::builder()
        .with_clock_freq(args.common.clock_freq)
        .with_invert_ef(args.invert_ef)
        .with_invert_q(args.invert_q)
        .with_memory(memory)
        .with_speed(args.speed)
        .with_uart(uart.into_box())
        .build();
    let tui = MembershipCardTui::new(mc);
    tui.run()
}
