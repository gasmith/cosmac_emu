//! A basic CDP1802 system.

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use color_eyre::Result;
use itertools::Itertools;

use crate::chips::cdp1802::{Cdp1802, Cdp1802Pins, Memory};
use crate::event::{
    EventLog, Flag, InputEvent, InputEventLog, OutputEvent, OutputEventLog, Port, Timed,
};
use crate::instr::InstrSchema;

#[derive(Debug, Clone, Copy, Hash)]
pub enum Status {
    Idle,
    Event,
    Ready,
    Breakpoint,
}

pub struct BasicSystem {
    cpu: Cdp1802,
    memory: Memory,
    pins: Cdp1802Pins,
    clock_cycle_time: Duration,
    clock_cycle: u64,
    input_events: InputEventLog,
    output_events: OutputEventLog,
    breakpoints: HashSet<u16>,
}
impl BasicSystem {
    pub fn new(cdp1802: Cdp1802, memory: Memory, clock_cycle_time: Duration) -> Self {
        let mut this = Self {
            cpu: cdp1802,
            memory,
            pins: Cdp1802Pins::default(),
            clock_cycle_time,
            clock_cycle: 0,
            input_events: InputEventLog::default(),
            output_events: OutputEventLog::default(),
            breakpoints: HashSet::default(),
        };
        this.reset();
        this
    }

    pub fn with_events(mut self, events: InputEventLog) -> Self {
        self.input_events = events;
        self
    }

    pub fn reset(&mut self) {
        self.pins.set_wait(true);
        self.pins.set_clear(false);
        self.cpu.tick(&mut self.pins);
        self.pins.set_clear(true);
        for _ in 0..9 {
            self.cpu.tick(&mut self.pins);
        }
        self.clock_cycle = 0;
        self.input_events.reset();
        self.output_events.clear();
    }

    pub fn cdp1802(&self) -> &Cdp1802 {
        &self.cpu
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
        self.clock_cycle_time
            .saturating_mul(u32::try_from(self.clock_cycle).unwrap_or(u32::MAX))
    }

    pub fn add_event(&mut self, event: InputEvent, offset: Duration) {
        let now = self.now();
        self.input_events.add(event, offset, now);
    }

    pub fn extend_events<P: AsRef<Path>>(&mut self, path: P, offset: Duration) -> Result<()> {
        let log = EventLog::from_file(path)?;
        let now = self.now();
        self.input_events.extend(log, offset, now);
        Ok(())
    }

    pub fn print_input_events(&self) {
        let iter = self.input_events.iter().sorted_unstable_by_key(|e| e.time);
        for Timed { time, event } in iter {
            let tick = (time.as_secs_f64() / self.clock_cycle_time.as_secs_f64()).trunc() as u64;
            println!("{tick:08x} {event:?}");
        }
    }

    pub fn print_output_events(&self) {
        let iter = self.output_events.iter().sorted_unstable_by_key(|e| e.time);
        for Timed { time, event } in iter {
            let tick = (time.as_secs_f64() / self.clock_cycle_time.as_secs_f64()).trunc() as u64;
            println!("{tick:08x} {event:?}");
        }
    }

    pub fn write_output_events<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.output_events.write_to_file(path)
    }

    pub fn events_mut(&mut self) -> &mut InputEventLog {
        &mut self.input_events
    }

    pub fn step(&mut self) -> Status {
        loop {
            let status = self.tick();
            if !matches!(status, Status::Ready) {
                return status;
            }
            if self.cpu.is_fetch_tick0() {
                return Status::Ready;
            }
        }
    }

    pub fn tick(&mut self) -> Status {
        let now = self.now();
        if let Some(e) = self.input_events.pop_next_at(now) {
            self.apply_event(e);
            return Status::Event;
        }

        let q_prev = self.pins.get_q();
        self.cpu.tick(&mut self.pins);

        // TODO: Log errors, add memory breakpoints
        let _ = self.memory.tick(&mut self.pins, true);

        let q = self.pins.get_q();
        if q != q_prev {
            self.output_events.push(OutputEvent::Q { value: q }.at(now))
        }

        // Output data strobe.
        match (self.pins.get_mrd(), self.pins.get_tpb(), self.pins.get_n()) {
            (false, true, n) if n > 0 => {
                let port = match n {
                    1 => Port::IO1,
                    2 => Port::IO2,
                    3 => Port::IO3,
                    4 => Port::IO4,
                    5 => Port::IO5,
                    6 => Port::IO6,
                    7 => Port::IO7,
                    _ => unreachable!(),
                };
                let value = self.pins.get_bus();
                self.output_events
                    .push(OutputEvent::Output { port, value }.at(now))
            }
            _ => (),
        }

        self.clock_cycle += 1;
        if self.cpu.is_waiting(self.pins) {
            Status::Idle
        } else if self.cpu.is_fetch_tick0() && self.has_breakpoint(self.cpu.rp()) {
            Status::Breakpoint
        } else {
            Status::Ready
        }
    }

    fn apply_event(&mut self, e: InputEvent) {
        match e {
            InputEvent::Interrupt => self.pins.set_intr(false),
            InputEvent::Flag { flag, value } => match flag {
                Flag::EF1 => self.pins.set_ef1(value),
                Flag::EF2 => self.pins.set_ef2(value),
                Flag::EF3 => self.pins.set_ef3(value),
                Flag::EF4 => self.pins.set_ef4(value),
            },
            InputEvent::Input { port, value } => {
                let select = self.pins.get_n();
                if select == port as u8 {
                    self.pins.set_bus(value);
                }
            }
        }
    }

    pub fn maybe_print_next_event(&self) {
        self.print_next(false)
    }

    pub fn print_next_cpu(&self) {
        self.print_next(true)
    }

    pub fn print_next(&self, cpu: bool) {
        let tick = self.clock_cycle;
        let time = self.now();
        if let Some(e) = self.input_events.peek_next_at(time) {
            println!("{tick:08x} Event: {e:?}");
        } else if cpu {
            println!("{tick:08x} {}", self.display());
        }
    }

    pub fn display(&self) -> String {
        let listing = if self.cpu.is_fetch_tick0() {
            let pc = self.cpu.rp();
            self.memory
                .get_instr_at(pc)
                .map_or("??".into(), |i| i.listing())
        } else {
            "".into()
        };
        format!("{}  {}", self.cpu, listing)
    }

    pub fn cpu(&self) -> &Cdp1802 {
        &self.cpu
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    pub fn memory_mut(&mut self) -> &mut Memory {
        &mut self.memory
    }

    pub fn pins(&self) -> Cdp1802Pins {
        self.pins
    }

    pub fn pins_mut(&mut self) -> &mut Cdp1802Pins {
        &mut self.pins
    }
}
