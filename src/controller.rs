//! The main controller.

use std::collections::HashSet;
use std::time::Duration;

use crate::args::Args;
use crate::event::EventLog;
use crate::memory::Memory;
use crate::state::State;

#[derive(Debug, Clone, Copy, Hash)]
pub enum Status {
    Idle,
    Ready,
    Breakpoint,
}

#[derive(Debug)]
pub struct Controller {
    state: State,
    events: EventLog,
    breakpoints: HashSet<u16>,
    cycle_time: Duration,
}
impl<'a> TryFrom<&'a Args> for Controller {
    type Error = anyhow::Error;

    fn try_from(args: &'a Args) -> Result<Self, Self::Error> {
        let memory = Memory::try_from(args)?;
        let state = State::new(memory);
        let events = EventLog::try_from(args)?;
        Ok(Self::new(state).with_events(events))
    }
}
impl Controller {
    pub fn new(state: State) -> Self {
        Self {
            state,
            events: EventLog::default(),
            breakpoints: HashSet::default(),
            cycle_time: Duration::from_micros(2),
        }
    }

    pub fn with_events(mut self, events: EventLog) -> Self {
        self.events = events;
        self
    }

    pub fn print_state(&self) {
        println!("{}", self.state);
    }

    pub fn reset(&mut self) {
        self.state.reset();
        self.events.reset();
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

    pub fn step(&mut self) -> Status {
        let time = self.cycle_time * u32::try_from(self.state.cycle()).unwrap_or(u32::MAX);
        if let Some(e) = self.events.pop_next_at(time) {
            println!("injecting {e:?}");
            self.state.apply_event(e);
        } else {
            self.state.step();
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
