//! The main controller.

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use itertools::Itertools;

use crate::args::Args;
use crate::event::{EventLog, InputEvent, InputEventLog, OutputEventLog, Timed};
use crate::memory::Memory;
use crate::state::State;

#[derive(Debug, Clone, Copy, Hash)]
pub enum Status {
    Idle,
    Event,
    Ready,
    Breakpoint,
}

#[derive(Debug)]
pub struct Controller {
    state: State,
    input_events: InputEventLog,
    output_events: OutputEventLog,
    breakpoints: HashSet<u16>,
    cycle_time: Duration,
}
impl<'a> TryFrom<&'a Args> for Controller {
    type Error = anyhow::Error;

    fn try_from(args: &'a Args) -> Result<Self, Self::Error> {
        let memory = Memory::try_from(args)?;
        let state = State::new(memory);
        let events = InputEventLog::try_from(args)?;
        Ok(Self::new(state, args.cycle_time).with_events(events))
    }
}
impl Controller {
    pub fn new(state: State, cycle_time: Duration) -> Self {
        Self {
            state,
            input_events: InputEventLog::default(),
            output_events: OutputEventLog::default(),
            breakpoints: HashSet::default(),
            cycle_time,
        }
    }

    pub fn with_events(mut self, events: InputEventLog) -> Self {
        self.input_events = events;
        self
    }

    pub fn reset(&mut self) {
        self.state.reset();
        self.input_events.reset();
        self.output_events.clear();
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    pub fn breakpoints(&self) -> &HashSet<u16> {
        &self.breakpoints
    }

    pub fn breakpoints_mut(&mut self) -> &mut HashSet<u16> {
        &mut self.breakpoints
    }

    pub fn has_breakpoint(&self, addr: u16) -> bool {
        self.breakpoints.contains(&addr)
    }

    pub fn now(&self) -> Duration {
        let cycle = self.state.cycle();
        self.cycle_time * u32::try_from(cycle).unwrap_or(u32::MAX)
    }

    pub fn add_event(&mut self, event: InputEvent, offset: Duration) {
        let now = self.now();
        self.input_events.add(event, offset, now);
    }

    pub fn extend_events<P: AsRef<Path>>(
        &mut self,
        path: P,
        offset: Duration,
    ) -> anyhow::Result<()> {
        let log = EventLog::from_file(path)?;
        let now = self.now();
        self.input_events.extend(log, offset, now);
        Ok(())
    }

    pub fn print_input_events(&self) {
        let iter = self.input_events.iter().sorted_unstable_by_key(|e| e.time);
        for Timed { time, event } in iter {
            let cycle = (time.as_secs_f64() / self.cycle_time.as_secs_f64()).trunc() as u64;
            println!("{cycle:08x} {event:?}");
        }
    }

    pub fn print_output_events(&self) {
        let iter = self.output_events.iter().sorted_unstable_by_key(|e| e.time);
        for Timed { time, event } in iter {
            let cycle = (time.as_secs_f64() / self.cycle_time.as_secs_f64()).trunc() as u64;
            println!("{cycle:08x} {event:?}");
        }
    }

    pub fn events_mut(&mut self) -> &mut InputEventLog {
        &mut self.input_events
    }

    pub fn time(&self) -> Duration {
        let cycle = self.state.cycle();
        self.cycle_time * u32::try_from(cycle).unwrap_or(u32::MAX)
    }

    pub fn step(&mut self) -> Status {
        let time = self.time();
        if let Some(e) = self.input_events.pop_next_at(time) {
            self.state.apply_event(e);
            Status::Event
        } else {
            if let Some(event) = self.state.step() {
                self.output_events.push(event.at(self.time()));
            }
            if self.state.is_idle() {
                Status::Idle
            } else if self.has_breakpoint(self.state.rp()) {
                Status::Breakpoint
            } else {
                Status::Ready
            }
        }
    }

    pub fn print_next(&self) {
        let cycle = self.state.cycle();
        let time = self.time();
        if let Some(e) = self.input_events.peek_next_at(time) {
            println!("{cycle:08x} Event: {e:?}");
        } else {
            println!("{}", self.state);
        }
    }
}
