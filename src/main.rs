#![allow(clippy::module_name_repetitions)]

use std::cmp::Ordering;

mod instr;
mod memory;
mod repl;
mod state;

use instr::{Instr, InstrSchema};
use memory::Memory;
use state::{State, Status};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let mut mem = Memory::new(64 * 1024);
    match args.len().cmp(&2) {
        Ordering::Greater => return Err(anyhow::anyhow!("usage: {} [bin]", args[0])),
        Ordering::Equal => {
            let image = std::fs::read(&args[1])?;
            mem.write_image(image);
        }
        Ordering::Less => (),
    }

    let mut state = State::new(mem);
    state.reset();
    repl::run(&mut state);
    Ok(())
}
