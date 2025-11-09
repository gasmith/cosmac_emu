//! Event replay

use core::time::Duration;
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
    str::FromStr,
};

use color_eyre::{Result, eyre};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputKind {
    Q,
    Io1,
    Io2,
    Io3,
    Io4,
    Io5,
    Io6,
    Io7,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputKind {
    Intr,
    Ef1,
    Ef2,
    Ef3,
    Ef4,
    Io1,
    Io2,
    Io3,
    Io4,
    Io5,
    Io6,
    Io7,
}
impl FromStr for InputKind {
    type Err = eyre::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let kind = match s.to_lowercase().as_str() {
            "intr" => Self::Intr,
            "ef1" => Self::Ef1,
            "ef2" => Self::Ef2,
            "ef3" => Self::Ef3,
            "ef4" => Self::Ef4,
            "io1" => Self::Io1,
            "io2" => Self::Io2,
            "io3" => Self::Io3,
            "io4" => Self::Io4,
            "io5" => Self::Io5,
            "io6" => Self::Io6,
            "io7" => Self::Io7,
            _ => eyre::bail!("invalid input event kind: {s}"),
        };
        Ok(kind)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct RawEvent<K> {
    timestamp_nanos: u64,
    kind: K,
    value: u8,
}

fn read_csv<K>(r: impl Read) -> Result<Vec<Event<K>>>
where
    K: for<'de> Deserialize<'de>,
{
    let mut reader = csv::ReaderBuilder::new().has_headers(false).from_reader(r);
    let mut events: Vec<Event<K>> = vec![];
    for event in reader.deserialize() {
        let raw: RawEvent<K> = event?;
        events.push(Event::from(raw));
    }
    Ok(events)
}

fn write_csv<K: Serialize + Copy>(w: impl Write, events: &[Event<K>]) -> Result<()> {
    let mut writer = csv::WriterBuilder::new().has_headers(false).from_writer(w);
    for event in events {
        writer.serialize(RawEvent::from(*event))?;
    }
    writer.flush()?;
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Event<K> {
    pub timestamp: Duration,
    pub kind: K,
    pub value: u8,
}
impl<K> Event<K> {
    pub fn new(timestamp: Duration, kind: K, value: u8) -> Self {
        Self {
            timestamp,
            kind,
            value,
        }
    }
}
impl<K> From<RawEvent<K>> for Event<K> {
    fn from(raw: RawEvent<K>) -> Self {
        Self {
            timestamp: Duration::from_nanos(raw.timestamp_nanos),
            kind: raw.kind,
            value: raw.value,
        }
    }
}
impl<K> From<Event<K>> for RawEvent<K> {
    fn from(ev: Event<K>) -> Self {
        Self {
            timestamp_nanos: ev.timestamp.as_nanos() as u64,
            kind: ev.kind,
            value: ev.value,
        }
    }
}

pub type InputEvent = Event<InputKind>;

/// An input event log.
#[derive(Debug, Default, Clone)]
pub struct InputEventLog {
    /// Pending events, sorted ascending by timestamp.
    pending: VecDeque<InputEvent>,

    /// Expired events, in no particular order.
    expired: Vec<InputEvent>,
}
impl InputEventLog {
    /// Reads an input event log from a file path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let file = BufReader::new(File::open(path)?);
        let events = read_csv(file)?;
        let expired = Vec::with_capacity(events.len());
        let mut pending: Vec<_> = events.into_iter().collect();
        pending.sort_unstable_by(|a, b| a.timestamp.cmp(&b.timestamp));
        Ok(Self {
            pending: pending.into_iter().collect(),
            expired,
        })
    }

    /// Adds a new event to the event log. An event that occurred before `now` is treated as
    /// expired, whereas an event that occurs at or after `now` is treated as pending.
    pub fn add(&mut self, event: InputEvent, now: Duration) {
        if event.timestamp < now {
            self.expired.push(event);
        } else {
            match self
                .pending
                .binary_search_by(|e| e.timestamp.cmp(&event.timestamp))
            {
                Ok(idx) | Err(idx) => self.pending.insert(idx, event),
            }
        }
    }

    /// Removes all events from the log.
    pub fn clear(&mut self) {
        self.expired.clear();
        self.pending.clear();
    }

    /// Peeks at the next pending event that expires before `when`.
    pub fn peek_next_at(&self, when: Duration) -> Option<InputEvent> {
        self.pending
            .front()
            .filter(|e| e.timestamp <= when)
            .copied()
    }

    /// Pops the next pending event that expires before `when`.
    pub fn pop_next_at(&mut self, when: Duration) -> Option<InputEvent> {
        match self.pending.front() {
            Some(e) if e.timestamp <= when => {
                let e = self.pending.pop_front().unwrap();
                self.expired.push(e);
                Some(e)
            }
            _ => None,
        }
    }

    /// Moves all expired events back into the sorted pending list.
    pub fn reset(&mut self) {
        let mut expired: Vec<_> = self.expired.drain(..).collect();
        expired.sort_unstable_by(|a, b| a.timestamp.cmp(&b.timestamp));
        assert!(
            expired
                .last()
                .zip(self.pending.front())
                .is_none_or(|(e, p)| e.timestamp <= p.timestamp)
        );
        for e in expired.into_iter().rev() {
            self.pending.push_front(e)
        }
    }

    /// Iterates over all events in the log, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = InputEvent> {
        self.expired.iter().chain(self.pending.iter()).copied()
    }
}

pub type OutputEvent = Event<OutputKind>;

#[derive(Default)]
pub struct OutputEventLog(Vec<OutputEvent>);
impl OutputEventLog {
    pub fn to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let file = BufWriter::new(File::create(path)?);
        write_csv(file, &self.0)
    }

    /// Adds an event to the event log.
    pub fn push(&mut self, event: OutputEvent) {
        self.0.push(event);
    }

    /// Removes all events from the log.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Iterates over all events in the log, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = OutputEvent> {
        self.0.iter().copied()
    }
}
