use clap::Parser;
use clap_repl::ClapEditor;
use std::sync::mpsc;

use crate::{InstrSchema, State, Status};

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

pub fn run(state: &mut State) {
    let mut rx = ctrlc_channel();
    let mut rl = ClapEditor::<Command>::new();
    println!("{state}");
    loop {
        if let Some(cmd) = rl.read_command() {
            handle_command(state, cmd, &mut rx);
        }
    }
}

fn handle_command(state: &mut State, cmd: Command, rx: &mut mpsc::Receiver<()>) {
    match cmd {
        Command::Reset => {
            state.reset();
            println!("{state}");
        }
        Command::Continue => loop {
            if rx.try_recv().is_ok() {
                println!("interrupted");
                break;
            }
            let status = state.step();
            match status {
                Status::Breakpoint => {
                    println!("{state}");
                    println!("breakpoint");
                    break;
                }
                Status::Idle => {
                    println!("idle");
                    break;
                }
                Status::Ready => println!("{state}"),
            }
        },
        Command::Display => {
            println!("{state}");
        }
        Command::List { count, addr } => {
            let mut addr = addr.unwrap_or(state.rp());
            for _ in 0..count {
                let bp = if state.breakpoints().contains(&addr) {
                    "*"
                } else {
                    " "
                };
                let (listing, size) = match state.decode_instr(addr) {
                    Some(instr) => (instr.listing(), instr.size()),
                    None => ("??".into(), 1),
                };
                println!("{addr:04x} {bp}{listing}");
                addr += u16::from(size);
            }
        }
        Command::Examine { addr, count } => {
            let mem = state.m.as_slice(addr, count);
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
                state.step();
                println!("{state}");
            }
        }
        Command::Registers => {
            state.print_registers();
        }
        Command::BreakpointList => {
            let mut bps: Vec<_> = state.breakpoints().iter().collect();
            bps.sort();
            println!("breakpoints:");
            for bp in bps {
                let listing = state
                    .decode_instr(*bp)
                    .map_or("??".into(), |instr| instr.listing());
                println!("{bp:04x} *{listing}");
            }
        }
        Command::BreakpointSet { addr } => {
            state.set_breakpoint(addr);
        }
        Command::BreakpointClear { addr } => {
            state.clear_breakpoint(addr);
        }
        Command::Flags => {
            state.print_flags();
        }
        Command::PokeFlag { flag } => {
            state.toggle_flag((flag - 1) as usize);
            state.print_flags();
        }
        Command::PokeMem { addr, byte } => {
            state.m.store(addr, byte);
        }
    }
}
