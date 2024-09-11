//! Event replay

use core::time::Duration;
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

use anyhow::{anyhow, Result};
use itertools::Itertools;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{digit1, one_of},
    combinator::{all_consuming, recognize},
    multi::many_m_n,
    sequence::{preceded, separated_pair},
    IResult,
};
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

impl RawTimedEvent {
    fn parse_time(s: &str) -> IResult<&str, u64> {
        let (s, time) = digit1(s)?;
        let time = time.parse().unwrap();
        Ok((s, time))
    }

    fn parse_event(s: &str) -> IResult<&str, Event> {
        let (s, event_type) = alt((tag("int"), tag("flag"), tag("input")))(s)?;
        let (s, event) = match event_type {
            "int" => (s, Event::Interrupt),
            "flag" => {
                let (s, n) = preceded(tag(",ef"), one_of("1234"))(s)?;
                let flag = match n {
                    '1' => Flag::EF1,
                    '2' => Flag::EF2,
                    '3' => Flag::EF3,
                    '4' => Flag::EF4,
                    _ => unreachable!(),
                };
                let (s, v) = preceded(tag(","), one_of("01"))(s)?;
                let value = match v {
                    '0' => false,
                    '1' => true,
                    _ => unreachable!(),
                };
                (s, Event::Flag { flag, value })
            }
            "input" => {
                let (s, n) = preceded(tag(",io"), one_of("1234567"))(s)?;
                let port = match n {
                    '1' => Port::IO1,
                    '2' => Port::IO2,
                    '3' => Port::IO3,
                    '4' => Port::IO4,
                    '5' => Port::IO5,
                    '6' => Port::IO6,
                    '7' => Port::IO7,
                    _ => unreachable!(),
                };
                let (s, v) = preceded(
                    tag(",0x"),
                    recognize(many_m_n(1, 2, one_of("0123456789abcdef"))),
                )(s)?;
                let value = u8::from_str_radix(v, 16).unwrap();
                (s, Event::Input { port, value })
            }
            _ => unreachable!(),
        };
        Ok((s, event))
    }

    fn parse_lst_line(s: &str) -> Result<Self> {
        let (_, (time, event)) = all_consuming(separated_pair(
            RawTimedEvent::parse_time,
            tag(","),
            RawTimedEvent::parse_event,
        ))(s)
        .map_err(|e| anyhow!("failed to parse: {e}"))?;
        Ok(RawTimedEvent { time, event })
    }
}

/// A raw deserialized event log.
#[derive(Debug, Clone, Deserialize)]
struct RawEventLog {
    /// A stream of events, in no particular order.
    events: Vec<RawTimedEvent>,
}
impl RawEventLog {
    fn from_json_reader<R: Read>(reader: R) -> Result<Self> {
        let log = serde_json::from_reader(reader)?;
        Ok(log)
    }

    fn from_json_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        Self::from_json_reader(file)
    }

    fn from_lst_reader<R: Read>(reader: R) -> Result<Self> {
        let br = BufReader::new(reader);
        let mut events = vec![];
        for line in br.lines() {
            let line = line?;
            let event = RawTimedEvent::parse_lst_line(&line)?;
            events.push(event);
        }
        Ok(Self { events })
    }

    fn from_lst_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        Self::from_lst_reader(file)
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
            Some(path) if path.extension().is_some_and(|e| e == "json") => {
                let raw = RawEventLog::from_json_file(path)?;
                raw.into()
            }
            Some(path) if path.extension().is_some_and(|e| e == "lst") => {
                let raw = RawEventLog::from_lst_file(path)?;
                raw.into()
            }
            Some(_) => return Err(anyhow!("unsupported event file format")),
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
