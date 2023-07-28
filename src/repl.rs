use anyhow;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use nom::branch::alt;
use nom::character::complete::{hex_digit1, one_of, space1};
use nom::combinator::{eof, opt};
use nom::error::{ErrorKind, FromExternalError, VerboseError};
use nom::sequence::{preceded, terminated};
use nom::IResult;

use crate::{State, Status};

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
enum Command {
    Display,
    Registers,
    Step,
    Continue,
    BreakpointSet { addr: u16 },
    BreakpointClear { addr: u16 },
    BreakpointList,
    Reset,
}

enum Sign {
    Pos,
    Neg,
}

struct SignedHexU16 {
    sign: Sign,
    val: u16,
}

fn parse_hex_u16(input: &str) -> IResult<&str, u16, VerboseError<&str>> {
    let (input, hex) = hex_digit1(input)?;
    let val = u16::from_str_radix(hex, 16).map_err(|e| {
        nom::Err::Failure(VerboseError::from_external_error(
            input,
            ErrorKind::TooLarge,
            e,
        ))
    })?;
    Ok((input, val))
}

fn parse_signed_hex_u16(input: &str) -> IResult<&str, SignedHexU16, VerboseError<&str>> {
    let (input, sign) = opt(one_of("+-"))(input)?;
    let sign = match sign {
        Some('-') => Sign::Neg,
        _ => Sign::Pos,
    };
    let (input, val) = parse_hex_u16(input)?;
    Ok((input, SignedHexU16 { sign, val }))
}

fn parse_breakpoint_args(input: &str) -> IResult<&str, Command, VerboseError<&str>> {
    let (input, hu16) = opt(preceded(space1, parse_signed_hex_u16))(input)?;
    Ok((
        input,
        match hu16 {
            Some(SignedHexU16 {
                sign: Sign::Pos,
                val: addr,
            }) => Command::BreakpointSet { addr },
            Some(SignedHexU16 {
                sign: Sign::Neg,
                val: addr,
            }) => Command::BreakpointClear { addr },
            None => Command::BreakpointList,
        },
    ))
}

fn parse_command(input: &str) -> IResult<&str, Command, VerboseError<&str>> {
    let (input, c) = terminated(one_of("bcdrsz"), alt((eof, space1)))(input)?;
    Ok(match c {
        'b' => parse_breakpoint_args(input)?,
        'c' => (input, Command::Continue),
        'd' => (input, Command::Display),
        'r' => (input, Command::Registers),
        's' => (input, Command::Step),
        'z' => (input, Command::Reset),
        _ => unreachable!(),
    })
}

pub fn run(state: &mut State) -> anyhow::Result<()> {
    let mut rl = DefaultEditor::new()?;
    loop {
        let line = rl.readline(">> ");
        match line {
            Ok(line) => {
                let line = line.trim();
                rl.add_history_entry(line)?;
                match parse_command(line) {
                    Ok(("", cmd)) => match cmd {
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
                    Ok((line, _)) => {
                        println!("trailing garbage: {line}");
                    }
                    Err(e) => {
                        println!("failed to parse: {e:#?}");
                    }
                };
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                println!("readline: {e:?}");
            }
        }
    }
    Ok(())
}
