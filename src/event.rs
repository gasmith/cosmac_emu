//! Event replay

use core::time::Duration;
use std::{collections::VecDeque, fs::File, io::Read, path::Path};

use itertools::Itertools;
use serde::Deserialize;

use crate::args::Args;

/// An external flag.
#[derive(Debug, Clone, Copy, Deserialize)]
pub enum Flag {
    EF1,
    EF2,
    EF3,
    EF4,
}

/// I/O ports.
#[derive(Debug, Clone, Copy, Deserialize)]
pub enum Port {
    IO1,
    IO2,
    IO3,
    IO4,
    IO5,
    IO6,
    IO7,
}

/// An event.
#[derive(Debug, Clone, Copy, Deserialize)]
pub enum Event {
    /// Raise interrupt line for one instruction cycle.
    Interrupt,
    /// Set or clear a flag.
    Flag { flag: Flag, value: bool },
    /// Set the data bus payload for the specified port.
    Input { port: Port, value: u8 },
}

/// A raw deserialized timed event.
#[derive(Debug, Clone, Deserialize)]
struct RawTimedEvent {
    /// The relative time at which this event fires, in nanoseconds.
    time: u64,

    /// The event itself.
    event: Event,
}

/// A raw deserialized event log.
#[derive(Debug, Clone, Deserialize)]
struct RawEventLog {
    /// A stream of events, in no particular order.
    events: Vec<RawTimedEvent>,
}
impl RawEventLog {
    fn from_json_reader<R: Read>(reader: R) -> anyhow::Result<Self> {
        let log = serde_json::from_reader(reader)?;
        Ok(log)
    }

    fn from_json_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        Self::from_json_reader(file)
    }
}

/// A timed event.
#[derive(Debug, Clone)]
struct TimedEvent {
    time: Duration,
    event: Event,
}
impl From<RawTimedEvent> for TimedEvent {
    fn from(e: RawTimedEvent) -> Self {
        TimedEvent {
            time: Duration::from_nanos(e.time),
            event: e.event,
        }
    }
}

/// An event log.
#[derive(Debug, Default, Clone)]
pub struct EventLog {
    /// A stream of events in ascending order.
    pending: VecDeque<TimedEvent>,

    /// Expired events in descending order.
    expired: VecDeque<TimedEvent>,
}
impl From<RawEventLog> for EventLog {
    fn from(log: RawEventLog) -> Self {
        let pending: VecDeque<_> = log
            .events
            .into_iter()
            .map(TimedEvent::from)
            .sorted_unstable_by_key(|e| e.time)
            .collect();
        let expired = VecDeque::with_capacity(pending.len());
        EventLog { pending, expired }
    }
}
impl<'a> TryFrom<&'a Args> for EventLog {
    type Error = anyhow::Error;

    fn try_from(args: &'a Args) -> Result<Self, Self::Error> {
        let log = match &args.event_log {
            Some(path) => {
                let raw = RawEventLog::from_json_file(path)?;
                raw.into()
            }
            None => EventLog::default(),
        };
        Ok(log)
    }
}
impl EventLog {
    pub fn pop_next_at(&mut self, time: Duration) -> Option<Event> {
        match self.pending.front() {
            Some(e) if e.time <= time => {
                let e = self.pending.pop_front().unwrap();
                let ev = e.event;
                self.expired.push_front(e);
                Some(ev)
            }
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        for e in self.expired.drain(..) {
            self.pending.push_front(e)
        }
    }
}
