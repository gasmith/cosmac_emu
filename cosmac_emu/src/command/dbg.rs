use std::{path::PathBuf, time::Duration};

use clap::Parser;

use crate::{
    chips::cdp1802::{Cdp1802, Memory},
    debugger,
    event::InputEventLog,
    systems::basic::BasicSystem,
};

use super::CommonRunArgs;

#[derive(Parser, Debug)]
pub struct DbgArgs {
    #[command(flatten)]
    common: CommonRunArgs,

    /// An event log to replay during program execution.
    #[arg(long)]
    pub input_events: Option<PathBuf>,

    /// An output event log to write on exit.
    #[arg(long)]
    pub output_events: Option<PathBuf>,
}

pub fn run(args: DbgArgs) -> color_eyre::Result<()> {
    let memory = Memory::builder()
        .with_capacity(args.common.memory_size)?
        .with_image_args(&args.common.ram)?
        .with_image_args(&args.common.rom)?
        .with_write_protect_ranges(&args.common.write_protect)
        .with_random()
        .build()?;
    let cdp1802 = Cdp1802::default();
    let cycle_time = Duration::from_secs(1) / args.common.clock_freq;
    let mut system = BasicSystem::new(cdp1802, memory, cycle_time);
    if let Some(path) = args.input_events {
        let events = InputEventLog::from_file(path)?;
        system = system.with_events(events);
    }
    debugger::run(&mut system);
    Ok(())
}
