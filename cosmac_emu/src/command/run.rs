use std::{path::PathBuf, time::Duration};

use clap::Parser;

use crate::{
    chips::cdp1802::{Cdp1802, Memory},
    controller::{Controller, Status},
    event::InputEventLog,
    repl,
};

use super::{CommonRunArgs, parse_duration};

#[derive(Parser, Debug)]
pub struct RunArgs {
    #[command(flatten)]
    common: CommonRunArgs,

    /// An event log to replay during program execution.
    #[arg(long)]
    pub input_events: Option<PathBuf>,

    /// An output event log to write on exit.
    #[arg(long)]
    pub output_events: Option<PathBuf>,

    /// Runs until the specified duration, as measured from the controller's clock, then exits.
    #[arg(long, value_parser=parse_duration)]
    pub run_duration: Option<Duration>,
}

pub fn run(args: RunArgs) -> color_eyre::Result<()> {
    let memory = Memory::builder()
        .with_capacity(args.common.memory_size)?
        .with_image_args(&args.common.ram)?
        .with_image_args(&args.common.rom)?
        .with_write_protect_ranges(&args.common.write_protect)
        .with_random()
        .build()?;
    let cdp1802 = Cdp1802::default();
    let cycle_time = Duration::from_secs(1) / args.common.clock_freq;
    let mut controller = Controller::new(cdp1802, memory, cycle_time);
    if let Some(path) = args.input_events {
        let events = InputEventLog::from_file(path)?;
        controller = controller.with_events(events);
    }
    if let Some(duration) = args.run_duration {
        while controller.now() < duration {
            if let Status::Idle = controller.step() {
                break;
            }
        }
        if let Some(path) = args.output_events {
            controller.write_output_events(&path)?;
        }
    } else {
        repl::run(&mut controller);
    }
    Ok(())
}
