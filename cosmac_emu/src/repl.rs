use std::fmt::Display;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use clap::Parser;
use color_eyre::Result;
use itertools::Itertools;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

use crate::command::parse_duration;
use crate::controller::{Controller, Status};
use crate::event::{InputEvent, ParseEvLog};
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
    #[command(alias = "t")]
    Tick {
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
    #[command(alias = "ie")]
    AddInputEvent {
        /// The event, in evlog form.
        ///
        /// Examples:
        ///  - int
        ///  - flag,ef[1-4],[01]
        ///  - input,io[1-7],0x[0-1a-f]
        #[arg(value_parser=InputEvent::from_evlog, verbatim_doc_comment)]
        event: InputEvent,

        /// The offset from the start of execution, in nanoseconds. If not specified, defaults to
        /// "now", i.e. nanoseconds since the last reset.
        #[arg(value_parser=When::parse, default_value_t=When::Now)]
        when: When,
    },
    /// Adds events from the specified log file.
    #[command(alias = "ies")]
    ExtendInputEvents {
        /// Path to the event log file.
        #[arg()]
        path: PathBuf,

        /// The offset from the start of execution, in nanoseconds. If not specified, defaults to
        /// "now", i.e. nanoseconds since the last reset.
        #[arg(value_parser=When::parse, default_value_t=When::Now)]
        when: When,
    },
    /// Lists events.
    #[command(alias = "lie")]
    ListInputEvents,
    /// Clears all events.
    #[command(alias = "cie")]
    ClearInputEvents,
    /// Lists events.
    #[command(alias = "loe")]
    ListOutputEvents,
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
    fn parse(s: &str) -> Result<Self> {
        if s == "now" {
            return Ok(When::Now);
        }
        let when = match s.chars().next() {
            Some('@') => When::Absolute(parse_duration(&s[1..])?),
            Some('+') => When::Future(parse_duration(&s[1..])?),
            Some('-') => When::Past(parse_duration(&s[1..])?),
            _ => When::Absolute(parse_duration(s)?),
        };
        Ok(when)
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
    let mut rl = DefaultEditor::new().unwrap();
    controller.print_next();
    loop {
        match rl.readline(">> ") {
            Ok(line) => {
                rl.add_history_entry(&line).ok();
                handle_line(controller, &line, &mut rx);
            }
            Err(ReadlineError::Interrupted) => (),
            Err(ReadlineError::Eof) => break,
            Err(err) => eprintln!("error: {:?}", err),
        }
    }
}

fn handle_line(controller: &mut Controller, line: &str, rx: &mut mpsc::Receiver<()>) {
    match shlex::split(line) {
        Some(mut parts) => {
            parts.insert(0, "".to_string());
            match Command::try_parse_from(parts) {
                Ok(c) => handle_command(controller, c, rx),
                Err(e) => eprintln!("{e}"),
            }
        }
        None => eprintln!("bad quotes: {line}"),
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
            println!("{}", controller.display())
        }
        Command::List { count, addr } => {
            let mut addr = addr.unwrap_or(controller.cdp1802().rp());
            for _ in 0..count {
                let bp = if controller.has_breakpoint(addr) {
                    "*"
                } else {
                    " "
                };
                let (listing, size) = controller
                    .decode_instr(addr)
                    .map_or(("??".into(), 1), |i| (i.listing(), i.size()));
                println!("{addr:04x} {bp}{listing}");
                addr += u16::from(size);
            }
        }
        Command::Examine { addr, count } => {
            let addr = addr as usize;
            let end = addr + count as usize;
            let mem = &controller.memory().as_slice()[addr..end];
            for (ii, v) in mem.iter().enumerate() {
                if ii % 8 == 0 {
                    if ii != 0 {
                        println!();
                    }
                    print!("{:04x}  ", addr + ii);
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
        Command::Tick { count } => {
            for _ in 0..count {
                controller.tick();
                controller.print_next();
            }
        }
        Command::Registers => {
            let cdp1802 = controller.cdp1802();
            println!(
                "d={d:02x}.{df} p={p:x} x={x:x} t={t:04x}",
                d = cdp1802.d,
                df = u8::from(cdp1802.df),
                p = cdp1802.p,
                x = cdp1802.x,
                t = cdp1802.t,
            );
            for (n, r) in cdp1802.r.iter().enumerate() {
                print!("{n:x}={r:04x}");
                if n % 4 == 3 {
                    println!();
                } else {
                    print!(" ");
                }
            }
        }
        Command::BreakpointList => {
            let bps: Vec<_> = controller.breakpoints().iter().sorted_unstable().collect();
            println!("breakpoints:");
            for bp in bps {
                let listing = controller
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
            let pins = controller.pins();
            println!("{pins}");
        }
        Command::PokeFlag { flag } => {
            let pins = controller.pins_mut();
            match flag {
                1 => pins.set_ef1(!pins.get_ef1()),
                2 => pins.set_ef2(!pins.get_ef2()),
                3 => pins.set_ef3(!pins.get_ef3()),
                4 => pins.set_ef4(!pins.get_ef4()),
                _ => println!("invalid flag"),
            };
        }
        Command::PokeMem { addr, byte } => {
            controller.memory_mut().as_mut_slice()[addr as usize] = byte;
        }
        Command::AddInputEvent { event, when } => {
            let offset = when.into_absolute(controller.now());
            controller.add_event(event, offset);
        }
        Command::ExtendInputEvents { path, when } => {
            let offset = when.into_absolute(controller.now());
            if let Err(e) = controller.extend_events(path, offset) {
                eprintln!("{e}");
            }
        }
        Command::ListInputEvents => controller.print_input_events(),
        Command::ClearInputEvents => controller.events_mut().clear(),
        Command::ListOutputEvents => controller.print_output_events(),
    }
}
