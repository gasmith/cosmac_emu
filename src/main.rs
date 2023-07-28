mod memory;
mod repl;
mod state;
pub use memory::Memory;
pub use state::{State, Status};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err(anyhow::anyhow!("usage: {} [bin]", args[0]));
    }

    let mut mem = Memory::new(32 * 1024);
    let image = std::fs::read(&args[1])?;
    mem.write_image(image);

    let mut state = State::new(mem);
    state.reset();
    repl::run(&mut state)?;
    Ok(())
}
