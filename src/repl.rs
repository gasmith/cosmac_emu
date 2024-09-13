use std::fmt::Display;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::anyhow;
use clap::Parser;
use clap_repl::ClapEditor;
use itertools::Itertools;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::one_of;
use nom::combinator::{all_consuming, map, map_res, rest};
use nom::sequence::pair;
use nom::IResult;

use crate::controller::{Controller, Status};
use crate::event::Event;
use crate::instr::InstrSchema;

#[derive(Debug, Clone, Parser)]
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
    /// Adds a single event.
    #[command(alias = "e")]
    EventAdd {
        /// The event, in lst form.
        ///
        /// Examples:
        ///  - int
        ///  - flag,ef[1-4],[01]
        ///  - input,io[1-7],0x[0-1a-f]
        #[arg(value_parser=Event::from_lst_str, verbatim_doc_comment)]
        event: Event,

        /// The offset from the start of execution, in nanoseconds. If not specified, defaults to
        /// "now", i.e. nanoseconds since the last reset.
        #[arg(value_parser=When::parse, default_value_t=When::Now)]
        when: When,
    },
    /// Adds events from the specified log file.
    #[command(alias = "ee")]
    EventExtend {
        /// Path to the event log file.
        #[arg()]
        path: PathBuf,

        /// The offset from the start of execution, in nanoseconds. If not specified, defaults to
        /// "now", i.e. nanoseconds since the last reset.
        #[arg(value_parser=When::parse, default_value_t=When::Now)]
        when: When,
    },
    /// Lists events.
    #[command(alias = "el")]
    EventList,
    /// Clears all events.
    #[command(alias = "ec")]
    EventClear,
}

#[derive(Debug, Clone, Default)]
pub enum When {
    #[default]
    Now,
    Absolute(Duration),
    Future(Duration),
    Past(Duration),
}
impl Display for When {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            When::Now => f.write_str("now"),
            When::Absolute(d) => write!(f, "@{d:?}"),
            When::Future(d) => write!(f, "+{d:?}"),
            When::Past(d) => write!(f, "-{d:?}"),
        }
    }
}
impl When {
    fn parse_partial(s: &str) -> IResult<&str, Self> {
        alt((
            map(tag("now"), |_| When::Now),
            map(
                pair(
                    one_of("@+-"),
                    map_res(rest, |s: &str| parse_duration::parse(s)),
                ),
                |(t, d)| match t {
                    '@' => When::Absolute(d),
                    '+' => When::Future(d),
                    '-' => When::Past(d),
                    _ => unreachable!(),
                },
            ),
        ))(s)
    }

    fn parse(s: &str) -> anyhow::Result<Self> {
        all_consuming(When::parse_partial)(s)
            .map_err(|e| anyhow!("{e}"))
            .map(|x| x.1)
    }

    fn into_absolute(self, now: Duration) -> Duration {
        match self {
            When::Now => now,
            When::Absolute(d) => d,
            When::Future(d) => now.saturating_add(d),
            When::Past(d) => now.saturating_sub(d),
        }
    }
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
    controller.print_next();
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
            controller.print_next();
        }
        Command::Continue => loop {
            if rx.try_recv().is_ok() {
                println!("interrupted");
                break;
            }
            let status = controller.step();
            controller.print_next();
            match status {
                Status::Breakpoint => {
                    println!("breakpoint");
                    break;
                }
                Status::Idle => {
                    println!("idle");
                    break;
                }
                _ => (),
            }
        },
        Command::Display => {
            println!("{}", controller.state())
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
                controller.print_next();
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
        Command::EventAdd { event, when } => {
            let offset = when.into_absolute(controller.now());
            controller.add_event(event, offset);
        }
        Command::EventExtend { path, when } => {
            let offset = when.into_absolute(controller.now());
            if let Err(e) = controller.extend_events(path, offset) {
                eprintln!("{e}");
            }
        }
        Command::EventList => controller.print_events(),
        Command::EventClear => controller.events_mut().clear(),
    }
}
