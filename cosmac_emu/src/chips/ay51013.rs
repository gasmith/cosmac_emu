//! General Instrument AY-5-1013 emulation

mod chip;
mod pins;
#[cfg(test)]
mod tests;
mod uart;
pub use chip::{Ay51013, Parity};
pub use pins::Ay51013Pins;
pub use uart::Ay51013Uart;
