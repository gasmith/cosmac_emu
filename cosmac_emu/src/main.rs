#![allow(clippy::module_name_repetitions)]

use clap::Parser;
use color_eyre::Result;

mod chips;
mod cli;
mod debugger;
mod event;
mod instr;
mod systems;
mod time;
mod tui;
mod uart;

use cli::Cli;

fn main() -> Result<()> {
    color_eyre::install()?;
    Cli::parse().run()
}
