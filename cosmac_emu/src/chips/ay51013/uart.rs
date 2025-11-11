use crate::uart::{Uart, UartMode, UartRxError};

use super::{Ay51013, Ay51013Pins};

pub struct Builder {
    mode: UartMode,
    clk_mul: u16,
    force_7bit_ascii: bool,
}
impl Default for Builder {
    fn default() -> Self {
        Self {
            mode: UartMode::default(),
            clk_mul: 1,
            force_7bit_ascii: false,
        }
    }
}
impl Builder {
    pub fn with_mode(self, mode: UartMode) -> Self {
        Self { mode, ..self }
    }

    pub fn with_clock_multiplier(self, clk_mul: u16) -> Self {
        Self { clk_mul, ..self }
    }

    pub fn with_force_7bit_ascii(self, force_7bit_ascii: bool) -> Self {
        Self {
            force_7bit_ascii,
            ..self
        }
    }

    pub fn with_baud(self, baud: u16, clk_freq: u32) -> Self {
        // The ay51013 requires 16 cycles per symbol.
        let target_freq = 16.0 * baud as f64;
        let clk_mul = (clk_freq as f64 / target_freq).round() as u16;
        self.with_clock_multiplier(clk_mul)
    }

    pub fn build(self) -> Ay51013Uart {
        let mut chip = Ay51013::default();
        let mut pins = Ay51013Pins::default();
        chip.configure(&mut pins, self.mode);
        Ay51013Uart {
            chip,
            pins,
            clk_mul: self.clk_mul,
            clk_seq: 0,
            force_7bit_ascii: self.force_7bit_ascii,
        }
    }
}

#[derive(Debug)]
pub struct Ay51013Uart {
    chip: Ay51013,
    pins: Ay51013Pins,
    clk_mul: u16,
    clk_seq: u16,
    force_7bit_ascii: bool,
}
impl Ay51013Uart {
    pub fn builder() -> Builder {
        Builder::default()
    }

    pub fn into_box(self) -> Box<dyn Uart> {
        Box::new(self)
    }
}
impl Uart for Ay51013Uart {
    fn reset(&mut self) {
        self.chip.reset(&mut self.pins);
    }

    fn tick(&mut self) {
        self.clk_seq += 1;
        if self.clk_seq == self.clk_mul {
            self.chip.tick(&mut self.pins);
            self.clk_seq = 0;
            self.pins.set_rdav(true);
            self.pins.set_ds(true);
        }
    }

    fn set_rx_pin(&mut self, val: bool) {
        self.pins.set_si(val)
    }

    fn get_tx_pin(&self) -> bool {
        self.pins.get_so()
    }

    fn rx(&mut self) -> Result<u8, crate::uart::UartRxError> {
        debug_assert!(self.is_rx_ready());
        let result = if self.pins.get_fe() {
            Err(UartRxError::Framing)
        } else if self.pins.get_pe() {
            Err(UartRxError::Parity)
        } else if self.pins.get_or() {
            Err(UartRxError::Overrun)
        } else {
            let mut byte = self.pins.get_rd();
            if self.force_7bit_ascii {
                byte &= 0x7f;
            }
            Ok(byte)
        };
        self.pins.set_rdav(false);
        result
    }

    fn tx(&mut self, byte: u8) {
        debug_assert!(self.is_tx_ready());
        self.pins.set_db(byte);
        self.pins.set_ds(false);
    }

    fn rx_hold_cycles(&self) -> u32 {
        let mult = self.clk_mul as u32;
        let seq = self.clk_seq as u32;
        debug_assert!(mult > seq);
        mult - seq
    }

    fn tx_hold_cycles(&self) -> u32 {
        let mult = self.clk_mul as u32;
        let seq = self.clk_seq as u32;
        debug_assert!(mult > seq);
        mult + (mult - seq)
    }

    fn is_rx_ready(&self) -> bool {
        self.pins.get_dav()
    }

    fn is_tx_ready(&self) -> bool {
        self.pins.get_tbmt()
    }

    fn is_tx_idle(&self) -> bool {
        self.chip.is_tx_idle()
    }
}
