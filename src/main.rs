#![allow(clippy::module_name_repetitions)]

use anyhow::Result;
use clap::Parser;

mod args;
mod instr;
mod memory;
mod repl;
mod state;

use self::args::Args;
use self::instr::{Instr, InstrSchema};
use self::memory::Memory;
use self::state::{State, Status};

fn main() -> Result<()> {
    let args: Args = Args::parse();

    let mut mem = Memory::new(args.memory_size);
    for image in args.image {
        let bin = std::fs::read(image.path)?;
        mem.write_image(bin, image.base_addr.into())?;
    }

    let mut state = State::new(mem);
    state.reset();
    repl::run(&mut state);
    Ok(())
}
