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
    combinator::{all_consuming, map, map_res, recognize},
    multi::many_m_n,
    sequence::{preceded, separated_pair, tuple},
    IResult,
};

use crate::args::Args;

/// A trait for parsing an object from an lst line.
pub trait ParseLst: Sized {
    fn parse_lst(s: &str) -> IResult<&str, Self>;

    fn to_lst(&self) -> String;

    fn from_lst(s: &str) -> Result<Self> {
        let (_, event) =
            all_consuming(Self::parse_lst)(s).map_err(|e| anyhow!("failed to parse: {e}"))?;
        Ok(event)
    }
}

/// An external flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    EF1,
    EF2,
    EF3,
    EF4,
}
impl ParseLst for Flag {
    fn parse_lst(s: &str) -> IResult<&str, Flag> {
        map(preceded(tag("ef"), one_of("1234")), |c: char| match c {
            '1' => Flag::EF1,
            '2' => Flag::EF2,
            '3' => Flag::EF3,
            '4' => Flag::EF4,
            _ => unreachable!(),
        })(s)
    }

    fn to_lst(&self) -> String {
        match self {
            Flag::EF1 => "ef1",
            Flag::EF2 => "ef2",
            Flag::EF3 => "ef3",
            Flag::EF4 => "ef4",
        }
        .into()
    }
}

/// I/O ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Port {
    IO1,
    IO2,
    IO3,
    IO4,
    IO5,
    IO6,
    IO7,
}
impl ParseLst for Port {
    fn parse_lst(s: &str) -> IResult<&str, Port> {
        map_res(preceded(tag(",io"), one_of("1234567")), |c: char| {
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
        })(s)
    }

    fn to_lst(&self) -> String {
        match self {
            Port::IO1 => "io1",
            Port::IO2 => "io2",
            Port::IO3 => "io3",
            Port::IO4 => "io4",
            Port::IO5 => "io5",
            Port::IO6 => "io6",
            Port::IO7 => "io7",
        }
        .into()
    }
}

fn parse_bool(s: &str) -> IResult<&str, bool> {
    map_res(one_of("01"), |c: char| {
        Ok::<bool, anyhow::Error>(match c {
            '0' => false,
            '1' => true,
            _ => unreachable!(),
        })
    })(s)
}

fn parse_hex_byte(s: &str) -> IResult<&str, u8> {
    map_res(
        preceded(
            tag("0x"),
            recognize(many_m_n(1, 2, one_of("0123456789abcdef"))),
        ),
        |s: &str| u8::from_str_radix(s, 16),
    )(s)
}

/// An event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// Raise interrupt line for one instruction cycle.
    Interrupt,
    /// Set or clear a flag.
    Flag { flag: Flag, value: bool },
    /// Inputs a byte on the specified port.
    Input { port: Port, value: u8 },
}
impl ParseLst for InputEvent {
    fn parse_lst(s: &str) -> IResult<&str, InputEvent> {
        let (s, event_type) = alt((tag("int"), tag("flag"), tag("input")))(s)?;
        match event_type {
            "int" => Ok((s, InputEvent::Interrupt)),
            "flag" => map(
                tuple((
                    preceded(tag(","), Flag::parse_lst),
                    preceded(tag(","), parse_bool),
                )),
                |(flag, value)| InputEvent::Flag { flag, value },
            )(s),
            "input" => map(
                tuple((
                    preceded(tag(","), Port::parse_lst),
                    preceded(tag(","), parse_hex_byte),
                )),
                |(port, value)| InputEvent::Input { port, value },
            )(s),
            _ => unreachable!(),
        }
    }

    fn to_lst(&self) -> String {
        match self {
            InputEvent::Interrupt => "int".into(),
            InputEvent::Flag { flag, value } => {
                format!("flag,{},{}", flag.to_lst(), u8::from(*value))
            }
            InputEvent::Input { port, value } => format!("input,{},0x{value:02x}", port.to_lst()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputEvent {
    /// Sets or resets the Q line.
    Q { value: bool },
    /// Outputs a byte on the specified port.
    Output { port: Port, value: u8 },
}
impl ParseLst for OutputEvent {
    fn parse_lst(s: &str) -> IResult<&str, OutputEvent> {
        let (s, event_type) = alt((tag("q"), tag("output")))(s)?;
        match event_type {
            "q" => map(preceded(tag(","), parse_bool), |value| OutputEvent::Q {
                value,
            })(s),
            "output" => map(
                tuple((
                    preceded(tag(","), Port::parse_lst),
                    preceded(tag(","), parse_hex_byte),
                )),
                |(port, value)| OutputEvent::Output { port, value },
            )(s),
            _ => unreachable!(),
        }
    }

    fn to_lst(&self) -> String {
        match self {
            OutputEvent::Q { value } => format!("q,{}", u8::from(*value)),
            OutputEvent::Output { port, value } => {
                format!("output,{},0x{value:02x}", port.to_lst())
            }
        }
    }
}
impl OutputEvent {
    pub fn at(self, time: Duration) -> Timed<OutputEvent> {
        Timed::new(time, self)
    }
}

/// A timed event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timed<E> {
    /// The relative time at which this event fires.
    pub time: Duration,

    /// The event itself.
    pub event: E,
}
impl<E> Timed<E> {
    pub fn new(time: Duration, event: E) -> Self {
        Self { time, event }
    }
}
impl<E> From<E> for Timed<E> {
    fn from(event: E) -> Self {
        Timed::new(Duration::ZERO, event)
    }
}
impl<E: ParseLst> ParseLst for Timed<E> {
    fn parse_lst(s: &str) -> IResult<&str, Self> {
        map(
            separated_pair(
                map_res(digit1, |t: &str| t.parse().map(Duration::from_nanos)),
                tag(","),
                E::parse_lst,
            ),
            |(time, event)| Self { time, event },
        )(s)
    }

    fn to_lst(&self) -> String {
        format!("{},{}", self.time.as_nanos(), self.event.to_lst())
    }
}
impl<E: Eq + PartialEq> PartialOrd for Timed<E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<E: Eq> Ord for Timed<E> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time).reverse()
    }
}

/// An event log.
#[derive(Debug, Clone)]
pub struct EventLog<E> {
    /// A stream of events, in no particular order.
    events: Vec<Timed<E>>,
}
impl<E> Default for EventLog<E> {
    fn default() -> Self {
        Self { events: vec![] }
    }
}
impl<E: ParseLst> EventLog<E> {
    fn from_lst_reader<R: Read>(reader: R) -> Result<Self> {
        let br = BufReader::new(reader);
        let mut events = vec![];
        for line in br.lines() {
            let line = line?;
            let event = ParseLst::from_lst(&line)?;
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
            Some(e) if e == "lst" => EventLog::from_lst_file(path)?,
            _ => return Err(anyhow!("unrecognized file extension")),
        })
    }

    /// Appends an event to the log. The caller is responsible for ordering.
    pub fn push(&mut self, event: Timed<E>) {
        self.events.push(event)
    }

    /// Clears the log.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Iterates over all events in the log, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = &Timed<E>> {
        self.events.iter()
    }
}

impl<E> IntoIterator for EventLog<E> {
    type Item = Timed<E>;

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.into_iter()
    }
}

/// An input event log.
#[derive(Debug, Default, Clone)]
pub struct InputEventLog {
    /// Pending events in a min-heap.
    pending: BinaryHeap<Timed<InputEvent>>,

    /// Expired events, in no particular order.
    expired: Vec<Timed<InputEvent>>,
}
impl From<EventLog<InputEvent>> for InputEventLog {
    fn from(log: EventLog<InputEvent>) -> Self {
        let pending: BinaryHeap<_> = log.events.into_iter().collect();
        let expired = Vec::with_capacity(pending.len());
        InputEventLog { pending, expired }
    }
}
impl<'a> TryFrom<&'a Args> for InputEventLog {
    type Error = anyhow::Error;

    fn try_from(args: &'a Args) -> Result<Self, Self::Error> {
        Ok(match &args.input_events {
            Some(path) => EventLog::from_file(path)?.try_into()?,
            None => Self::default(),
        })
    }
}
impl InputEventLog {
    /// Adds a new event to the event log, with the specified `offset`. An event that occured
    /// before `now` is treated as expired, whereas an event that occurs at or after `now` is
    /// treated as pending.
    pub fn add<E: Into<Timed<InputEvent>>>(&mut self, event: E, offset: Duration, now: Duration) {
        let mut event = event.into();
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
    pub fn extend<E: Into<Timed<InputEvent>>, I: IntoIterator<Item = E>>(
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
    pub fn peek_next_at(&self, time: Duration) -> Option<&InputEvent> {
        self.pending
            .peek()
            .filter(|e| e.time <= time)
            .as_ref()
            .map(|e| &e.event)
    }

    /// Pops the next pending event.
    pub fn pop_next_at(&mut self, time: Duration) -> Option<InputEvent> {
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
    pub fn iter(&self) -> impl Iterator<Item = &Timed<InputEvent>> {
        self.expired.iter().chain(self.pending.iter())
    }
}

pub type OutputEventLog = EventLog<OutputEvent>;
