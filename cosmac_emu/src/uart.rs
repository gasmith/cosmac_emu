use std::{fmt::Display, str::FromStr};

#[derive(thiserror::Error, Debug, Clone, Copy)]
pub enum UartRxError {
    #[error("framing error")]
    Framing,
    #[error("parity error")]
    Parity,
    #[error("overrun error")]
    Overrun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    Even,
    Odd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UartMode {
    pub char_bits: u8,
    pub parity: Option<Parity>,
    pub stop_bits: u8,
}
impl Display for UartMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let p = match self.parity {
            None => 'n',
            Some(Parity::Even) => 'e',
            Some(Parity::Odd) => 'o',
        };
        write!(f, "{}{p}{}", self.char_bits, self.stop_bits)
    }
}
impl Default for UartMode {
    fn default() -> Self {
        Self {
            char_bits: 8,
            parity: None,
            stop_bits: 1,
        }
    }
}
impl FromStr for UartMode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        let char_bits: u8 = match chars.next() {
            Some('5') => 5,
            Some('6') => 6,
            Some('7') => 7,
            Some('8') => 8,
            _ => return Err("invalid char bits"),
        };
        let parity = match chars.next() {
            Some('n') => None,
            Some('e') => Some(Parity::Even),
            Some('o') => Some(Parity::Odd),
            _ => return Err("invalid parity"),
        };
        let stop_bits: u8 = match chars.next() {
            Some('1') => 1,
            Some('2') => 2,
            _ => return Err("invalid stop bits"),
        };
        if chars.next().is_some() {
            return Err("trailing garbage");
        }
        Ok(Self {
            char_bits,
            parity,
            stop_bits,
        })
    }
}
impl UartMode {
    #[cfg(test)]
    pub fn new(char_bits: u8, parity: impl Into<Option<Parity>>, stop_bits: u8) -> Self {
        Self {
            char_bits,
            parity: parity.into(),
            stop_bits,
        }
    }
}

pub trait Uart: std::fmt::Debug {
    fn reset(&mut self);
    fn tick(&mut self);

    fn set_rx_pin(&mut self, val: bool);
    fn get_tx_pin(&self) -> bool;

    fn rx(&mut self) -> Result<u8, UartRxError>;
    fn tx(&mut self, byte: u8);

    fn rx_hold_cycles(&self) -> u32;
    fn tx_hold_cycles(&self) -> u32;

    fn is_rx_ready(&self) -> bool;
    fn is_tx_ready(&self) -> bool;
    fn is_tx_idle(&self) -> bool;
}
