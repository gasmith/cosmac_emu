use anyhow;
use clap::Parser;
use clap_repl::ClapEditor;

use crate::{State, Status};

#[derive(Debug, Parser)]
enum Command {
    #[command(alias = "d")]
    Display,
    #[command(alias = "r")]
    Registers,
    #[command(alias = "s")]
    Step,
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

pub fn run(state: &mut State) -> anyhow::Result<()> {
    let mut rl = ClapEditor::<Command>::new();
    loop {
        match rl.read_command() {
            None => {}
            Some(cmd) => match cmd {
                Command::Reset => {
                    state.reset();
                    println!("{state}");
                }
                Command::Continue => loop {
                    let status = state.step();
                    println!("{state}");
                    match status {
                        Status::Breakpoint => {
                            println!("breakpoint");
                            break;
                        }
                        Status::Idle => {
                            println!("idle");
                            break;
                        }
                        _ => {}
                    }
                },
                Command::Display => {
                    println!("{state}");
                }
                Command::Step => {
                    state.step();
                    println!("{state}");
                }
                Command::Registers => {
                    state.print_registers();
                }
                Command::BreakpointList => {
                    let mut bps: Vec<_> = state.breakpoints().iter().collect();
                    bps.sort();
                    println!("breakpoints:");
                    for bp in bps {
                        println!("{bp:04x}");
                    }
                }
                Command::BreakpointSet { addr } => {
                    state.set_breakpoint(addr);
                }
                Command::BreakpointClear { addr } => {
                    state.clear_breakpoint(&addr);
                }
            },
        };
    }
}
