use clap::Parser;
use clap_repl::ClapEditor;
use itertools::Itertools;
use std::sync::mpsc;

use crate::controller::{Controller, Status};
use crate::instr::InstrSchema;

#[derive(Debug, Copy, Clone, Parser)]
enum Command {
    #[command(alias = "d")]
    Display,
    #[command(alias = "r")]
    Registers,
    #[command(alias = "l")]
    List {
        #[arg(default_value = "10")]
        count: u16,
        #[arg(value_parser=parse_hex_u16)]
        addr: Option<u16>,
    },
    #[command(alias = "s")]
    Step {
        #[arg(default_value = "1")]
        count: u16,
    },
    #[command(alias = "c")]
    Continue,
    #[command(alias = "bl")]
    BreakpointList,
    #[command(alias = "b")]
    BreakpointSet {
        #[arg(value_parser=parse_hex_u16)]
        addr: u16,
    },
    #[command(alias = "bc")]
    BreakpointClear {
        #[arg(value_parser=parse_hex_u16)]
        addr: u16,
    },
    #[command(alias = "x")]
    Examine {
        #[arg(value_parser=parse_hex_u16)]
        addr: u16,
        #[arg(value_parser=parse_hex_u16, default_value="8")]
        count: u16,
    },
    #[command(alias = "f")]
    Flags,
    #[command(alias = "pf")]
    PokeFlag { flag: u8 },
    #[command(alias = "pm")]
    PokeMem {
        #[arg(value_parser=parse_hex_u16)]
        addr: u16,
        #[arg(value_parser=parse_hex_u8)]
        byte: u8,
    },
    #[command(alias = "z")]
    Reset,
}

#[derive(Debug, Parser)]
struct Address {
    #[clap(value_parser=parse_hex_u16)]
    addr: u16,
}

fn parse_hex_u16(arg: &str) -> Result<u16, std::num::ParseIntError> {
    u16::from_str_radix(arg, 16)
}

fn parse_hex_u8(arg: &str) -> Result<u8, std::num::ParseIntError> {
    u8::from_str_radix(arg, 16)
}

fn ctrlc_channel() -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::sync_channel(1);
    ctrlc::set_handler(move || {
        if tx.try_send(()).is_err() {
            println!("received ctrl-c twice!");
            std::process::exit(130);
        }
    })
    .expect("failed to configure ctrl-c handler");
    rx
}

pub fn run(controller: &mut Controller) {
    let mut rx = ctrlc_channel();
    let mut rl = ClapEditor::<Command>::new();
    controller.print_state();
    loop {
        if let Some(cmd) = rl.read_command() {
            handle_command(controller, cmd, &mut rx);
        }
    }
}

fn handle_command(controller: &mut Controller, cmd: Command, rx: &mut mpsc::Receiver<()>) {
    match cmd {
        Command::Reset => {
            controller.reset();
            controller.print_state();
        }
        Command::Continue => loop {
            if rx.try_recv().is_ok() {
                println!("interrupted");
                break;
            }
            let status = controller.step();
            match status {
                Status::Breakpoint => {
                    controller.print_state();
                    println!("breakpoint");
                    break;
                }
                Status::Idle => {
                    controller.print_state();
                    println!("idle");
                    break;
                }
                Status::Event => (),
                Status::Ready => {
                    controller.print_state();
                }
            }
        },
        Command::Display => {
            controller.print_state();
        }
        Command::List { count, addr } => {
            let mut addr = addr.unwrap_or(controller.state().rp());
            for _ in 0..count {
                let bp = if controller.has_breakpoint(addr) {
                    "*"
                } else {
                    " "
                };
                let (listing, size) = controller
                    .state()
                    .decode_instr(addr)
                    .map_or(("??".into(), 1), |i| (i.listing(), i.size()));
                println!("{addr:04x} {bp}{listing}");
                addr += u16::from(size);
            }
        }
        Command::Examine { addr, count } => {
            let mem = controller.state().m.as_slice(addr, count);
            for (ii, v) in mem.iter().enumerate() {
                if ii % 8 == 0 {
                    if ii != 0 {
                        println!();
                    }
                    print!("{:04x}  ", addr + u16::try_from(ii).expect("16-bit"));
                }
                print!("{v:02x} ");
            }
            println!();
        }
        Command::Step { count } => {
            for _ in 0..count {
                controller.step();
                controller.print_state();
            }
        }
        Command::Registers => {
            controller.state().print_registers();
        }
        Command::BreakpointList => {
            let bps: Vec<_> = controller.breakpoints().iter().sorted_unstable().collect();
            println!("breakpoints:");
            for bp in bps {
                let listing = controller
                    .state()
                    .decode_instr(*bp)
                    .map_or("??".into(), |instr| instr.listing());
                println!("{bp:04x} *{listing}");
            }
        }
        Command::BreakpointSet { addr } => {
            controller.breakpoints_mut().insert(addr);
        }
        Command::BreakpointClear { addr } => {
            controller.breakpoints_mut().remove(&addr);
        }
        Command::Flags => {
            controller.state().print_flags();
        }
        Command::PokeFlag { flag } => {
            let state = controller.state_mut();
            state.toggle_flag(flag);
            state.print_flags();
        }
        Command::PokeMem { addr, byte } => {
            controller.state_mut().m.store(addr, byte);
        }
    }
}
