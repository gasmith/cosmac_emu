#![allow(clippy::module_name_repetitions)]

use anyhow::Result;
use clap::Parser;

mod args;
mod controller;
mod event;
mod instr;
mod memory;
mod repl;
mod state;

use self::args::Args;
use self::controller::Controller;

fn main() -> Result<()> {
    let args: Args = Args::parse();
    let mut controller = Controller::try_from(&args)?;
    repl::run(&mut controller);
    Ok(())
}
