//! Event replay

use core::time::Duration;
use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

use anyhow::{anyhow, Result};
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{digit1, one_of},
    combinator::{all_consuming, map_res, recognize},
    multi::many_m_n,
    sequence::{preceded, separated_pair},
    IResult,
};
use serde::Deserialize;

use crate::args::Args;

/// An external flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Flag {
    EF1,
    EF2,
    EF3,
    EF4,
}

/// I/O ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Event {
    /// Raise interrupt line for one instruction cycle.
    Interrupt,
    /// Set or clear a flag.
    Flag { flag: Flag, value: bool },
    /// Set the data bus payload for the specified port.
    Input { port: Port, value: u8 },
}
impl Event {
    fn parse_lst_event(s: &str) -> IResult<&str, Event> {
        let (s, event_type) = alt((tag("int"), tag("flag"), tag("input")))(s)?;
        let (s, event) = match event_type {
            "int" => (s, Event::Interrupt),
            "flag" => {
                let (s, flag) = map_res(preceded(tag(",ef"), one_of("1234")), |c: char| {
                    Ok::<Flag, anyhow::Error>(match c {
                        '1' => Flag::EF1,
                        '2' => Flag::EF2,
                        '3' => Flag::EF3,
                        '4' => Flag::EF4,
                        _ => unreachable!(),
                    })
                })(s)?;
                let (s, value) = map_res(preceded(tag(","), one_of("01")), |c: char| {
                    Ok::<bool, anyhow::Error>(match c {
                        '0' => false,
                        '1' => true,
                        _ => unreachable!(),
                    })
                })(s)?;
                (s, Event::Flag { flag, value })
            }
            "input" => {
                let (s, port) = map_res(preceded(tag(",io"), one_of("1234567")), |c: char| {
                    Ok::<Port, anyhow::Error>(match c {
                        '1' => Port::IO1,
                        '2' => Port::IO2,
                        '3' => Port::IO3,
                        '4' => Port::IO4,
                        '5' => Port::IO5,
                        '6' => Port::IO6,
                        '7' => Port::IO7,
                        _ => unreachable!(),
                    })
                })(s)?;
                let (s, value) = map_res(
                    preceded(
                        tag(",0x"),
                        recognize(many_m_n(1, 2, one_of("0123456789abcdef"))),
                    ),
                    |s: &str| u8::from_str_radix(s, 16),
                )(s)?;
                (s, Event::Input { port, value })
            }
            _ => unreachable!(),
        };
        Ok((s, event))
    }

    pub fn from_lst_str(s: &str) -> Result<Self> {
        let (_, event) =
            all_consuming(Self::parse_lst_event)(s).map_err(|e| anyhow!("failed to parse: {e}"))?;
        Ok(event)
    }
}

/// A raw deserialized timed event.
#[derive(Debug, Clone, Deserialize)]
pub struct RawTimedEvent {
    /// The relative time at which this event fires, in nanoseconds.
    time: u64,

    /// The event itself.
    event: Event,
}

impl RawTimedEvent {
    /// Parses a raw timed event from a line in an LST file.
    fn parse_lst_line(s: &str) -> Result<Self> {
        let (_, (time, event)) = all_consuming(separated_pair(
            map_res(digit1, |t: &str| t.parse()),
            tag(","),
            Event::parse_lst_event,
        ))(s)
        .map_err(|e| anyhow!("failed to parse: {e}"))?;
        Ok(RawTimedEvent { time, event })
    }
}

/// A raw deserialized event log.
#[derive(Debug, Clone, Deserialize)]
pub struct RawEventLog {
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

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Ok(match path.as_ref().extension() {
            Some(e) if e == "json" => RawEventLog::from_json_file(path)?,
            Some(e) if e == "lst" => RawEventLog::from_lst_file(path)?,
            _ => return Err(anyhow!("unrecognized file extension")),
        })
    }
}
impl IntoIterator for RawEventLog {
    type Item = RawTimedEvent;

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.into_iter()
    }
}

/// A timed event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedEvent {
    pub time: Duration,
    pub event: Event,
}
impl From<Event> for TimedEvent {
    fn from(event: Event) -> Self {
        TimedEvent {
            time: Duration::ZERO,
            event,
        }
    }
}
impl From<RawTimedEvent> for TimedEvent {
    fn from(e: RawTimedEvent) -> Self {
        TimedEvent {
            time: Duration::from_nanos(e.time),
            event: e.event,
        }
    }
}
impl PartialOrd for TimedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TimedEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time).reverse()
    }
}

/// An event log.
#[derive(Debug, Default, Clone)]
pub struct EventLog {
    /// Pending events in a min-heap.
    pending: BinaryHeap<TimedEvent>,

    /// Expired events, in no particular order.
    expired: Vec<TimedEvent>,
}
impl From<RawEventLog> for EventLog {
    fn from(log: RawEventLog) -> Self {
        let pending: BinaryHeap<_> = log.events.into_iter().map(TimedEvent::from).collect();
        let expired = Vec::with_capacity(pending.len());
        EventLog { pending, expired }
    }
}
impl<'a> TryFrom<&'a Args> for EventLog {
    type Error = anyhow::Error;

    fn try_from(args: &'a Args) -> Result<Self, Self::Error> {
        Ok(match &args.event_log {
            Some(path) => RawEventLog::from_file(path)?.into(),
            None => Self::default(),
        })
    }
}
impl EventLog {
    /// Adds a new event to the event log, with the specified `offset`. An event that occured
    /// before `now` is treated as expired, whereas an event that occurs at or after `now` is
    /// treated as pending.
    pub fn add<E: Into<TimedEvent>>(&mut self, event: E, offset: Duration, now: Duration) {
        let mut event: TimedEvent = event.into();
        event.time += offset;
        if event.time < now {
            self.expired.push(event);
        } else {
            self.pending.push(event);
        }
    }

    /// Merges new events into the event log, with the specified `offset`. Events that occured
    /// before `now` are treated as expired, whereas events that occur at or after `now` are
    /// treated as pending.
    pub fn extend<E: Into<TimedEvent>, I: IntoIterator<Item = E>>(
        &mut self,
        events: I,
        offset: Duration,
        now: Duration,
    ) {
        for event in events {
            self.add(event, offset, now);
        }
    }

    /// Removes all events from the log.
    pub fn clear(&mut self) {
        self.expired.clear();
        self.pending.clear();
    }

    /// Peeks at the next pending event.
    pub fn peek_next_at(&self, time: Duration) -> Option<&Event> {
        self.pending
            .peek()
            .filter(|e| e.time <= time)
            .as_ref()
            .map(|e| &e.event)
    }

    /// Pops the next pending event.
    pub fn pop_next_at(&mut self, time: Duration) -> Option<Event> {
        match self.pending.peek() {
            Some(e) if e.time <= time => {
                let e = self.pending.pop().unwrap();
                let ev = e.event;
                self.expired.push(e);
                Some(ev)
            }
            _ => None,
        }
    }

    /// Moves all expired events back into the pending heap.
    pub fn reset(&mut self) {
        for e in self.expired.drain(..).rev() {
            self.pending.push(e)
        }
    }

    /// Iterates over all events in the log, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = &TimedEvent> {
        self.expired.iter().chain(self.pending.iter())
    }
}
