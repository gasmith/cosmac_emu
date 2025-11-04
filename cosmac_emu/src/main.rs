#![allow(clippy::module_name_repetitions)]

use clap::Parser;
use color_eyre::Result;

mod args;
mod chips;
mod command;
mod controller;
mod event;
mod instr;
mod repl;
mod systems;
mod time;
mod tui;

use command::Cli;

fn main() -> Result<()> {
    color_eyre::install()?;
    Cli::parse().run()
}
