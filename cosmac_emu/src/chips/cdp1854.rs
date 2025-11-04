//! RCA CDP1854 UART emulation
//!
//! This implements the CDP1854 UART chip which is designed to work with
//! CDP1800 series microprocessors including the CDP1802.
#![allow(dead_code)]

use flume::{Receiver, Sender, TryRecvError, TrySendError};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Pins {
    // Data Bus (Transmitter)
    TBus0,
    TBus1,
    TBus2,
    TBus3,
    TBus4,
    TBus5,
    TBus6,
    TBus7,

    // Data Bus (Receiver)
    RBus0,
    RBus1,
    RBus2,
    RBus3,
    RBus4,
    RBus5,
    RBus6,
    RBus7,

    // Chip Select
    Cs1, // High to select
    Cs2, // Low to select
    Cs3, // High to select

    // Control
    Clear,
    RdWr,
    Rsel,
    Tpb,

    // Serial Data
    Sdi, // Serial Data In
    Sdo, // Serial Data Out

    // Status Signals
    Thre, // Transmitter Holding Register Empty
    Da,   // Data Available
    Fe,   // Framing Error
    PeOe, // Parity Error/Overrun Error
    Int,  // Interrupt

    // Flow Control
    Rts, // Request to Send
    Cts, // Clear to Send
    Psi, // Peripheral Status Interrupt
    Es,  // External Status

    // Mode select (only 1 is supported for now)
    Mode,
}

impl Pins {
    pub fn mask_all() -> u64 {
        (1 << ((Self::Mode as u8) + 1)) - 1
    }

    fn mask(self) -> u64 {
        1 << (self as u8)
    }

    pub fn get(self, pins: u64) -> bool {
        (pins & self.mask()) > 0
    }

    pub fn get3(self, pins: u64) -> u8 {
        assert!(matches!(self, Self::Cs1));
        let lsb = self as u8;
        ((pins >> lsb) & 0x7) as u8
    }

    pub fn get8(self, pins: u64) -> u8 {
        assert!(matches!(self, Self::TBus0 | Self::RBus0));
        let lsb = self as u8;
        ((pins >> lsb) & 0xff) as u8
    }

    pub fn set(self, pins: &mut u64, val: bool) {
        let val = (val as u64) << (self as u8);
        *pins = (*pins & !self.mask()) | val;
    }

    pub fn set3(self, pins: &mut u64, val: u8) {
        assert!(matches!(self, Self::Cs1));
        let lsb = self as u8;
        let mask = 0x7 << lsb;
        let val = (val as u64) << lsb;
        *pins = (*pins & !mask) | val;
    }

    pub fn set8(self, pins: &mut u64, val: u8) {
        assert!(matches!(self, Self::TBus0 | Self::RBus0));
        let lsb = self as u8;
        let mask = 0xff << lsb;
        let val = (val as u64) << lsb;
        *pins = (*pins & !mask) | val;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    Even,
    Odd,
}

/// Control register.
///
/// Bit layout:
/// - 0: PI: Parity inhibit
/// - 1: EPE: Even parity enable
/// - 2: BSS: Stop bit select
/// - 3: WLS1: Word length select 1
/// - 4: WLS2: Word length select 2
/// - 5: IE: Interrupt enable
/// - 6: TBRK: Transmit break
/// - 7: TR: Transmit request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    /// Word length in bits, on the range [5..8].
    word_length: u8,

    /// Parity configuration.
    parity: Option<Parity>,

    /// Number of stop bits, either 1 or 2.
    ///
    /// If `char_bits` is 5, then a `stop_bits` value of 2 should be interpreted as 1.5.
    stop_bits: u8,

    /// Interrupt enable.
    ///
    /// When set high /THRE, DA, THRE + TSRE, /CTS, and PSI interrupts are enabled.
    ie: bool,

    /// Transmit break.
    ///
    /// Holds SDO low when set. Once the break bit in the control register has been set high, SDO
    /// will stay low until the break bit is reset low and one of the following occurs: /CLEAR goes
    /// low; /CTS goes high; or a word is transmitted. (The transmitted word will not be valid
    /// since there can be no start bit if SDO is already low. SDO can be set high without
    /// intermediate transitions by transmitting a word consisting of all zeros).
    tx_break: bool,

    /// Transmit request.
    ///
    /// When set high, /RTS is set low and data transfer through the transmitter is initiated by
    /// the initial THRE interrupt. (When loading the Control Register from the bus, this (TR) bit
    /// inhibits changing of other control flip-flops).
    tr: bool,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            word_length: 8,
            parity: None,
            stop_bits: 1,
            ie: false,
            tx_break: false,
            tr: false,
        }
    }
}
impl Control {
    pub fn new(word_length: u8, parity: impl Into<Option<Parity>>, stop_bits: u8) -> Self {
        Self {
            word_length,
            parity: parity.into(),
            stop_bits,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn with_ie(mut self, ie: bool) -> Self {
        self.ie = ie;
        self
    }

    #[must_use]
    pub fn with_tx_break(mut self, tx_break: bool) -> Self {
        self.tx_break = tx_break;
        self
    }

    #[must_use]
    pub fn with_tr(mut self, tr: bool) -> Self {
        self.tr = tr;
        self
    }

    pub fn set_ie(&mut self, ie: bool) {
        self.ie = ie;
    }

    pub fn set_tx_break(&mut self, tx_break: bool) {
        self.tx_break = tx_break;
    }

    pub fn set_tr(&mut self, tr: bool) {
        self.tr = tr;
    }

    // Returns the word mask.
    fn word_mask(&self) -> u8 {
        match self.word_length {
            5 => 0x1f,
            6 => 0x3f,
            7 => 0x7f,
            8 => 0xff,
            _ => unreachable!(),
        }
    }

    /// Returns the number of ticks from the time the start bit is detected to the time at which
    /// data is made available (half-way through the first stop bit).
    fn rx_ticks(&self) -> u64 {
        8 + 16 + (self.word_length as u64) + 16 * (self.parity.is_some() as u64) + 16
    }

    /// Returns the number of ticks from the time the start bit is initiated to the time at which
    /// the transmission is complete.
    #[cfg(test)]
    fn tx_ticks(&self) -> u64 {
        16 + 16 * (self.word_length as u64)
            + 16 * (self.parity.is_some() as u64)
            + self.stop_ticks()
    }

    /// Returns the number of stop ticks used by the transmitter.
    fn stop_ticks(&self) -> u64 {
        match (self.word_length, self.stop_bits) {
            (_, 1) => 16,
            (5, 2) => 24,
            (_, 2) => 32,
            _ => unreachable!(),
        }
    }
}
impl From<Control> for u8 {
    fn from(c: Control) -> u8 {
        let (pi, epe) = match c.parity {
            None => (1, 0),
            Some(Parity::Odd) => (0, 0),
            Some(Parity::Even) => (0, 1),
        };
        let bss = (c.stop_bits > 1) as u8;
        let (wls1, wls2) = match c.word_length {
            0..=5 => (0, 0),
            6 => (1, 0),
            7 => (0, 1),
            8.. => (1, 1),
        };
        pi | epe << 1
            | bss << 2
            | wls1 << 3
            | wls2 << 4
            | (c.ie as u8) << 5
            | (c.tx_break as u8) << 6
            | (c.tr as u8) << 7
    }
}
impl From<u8> for Control {
    fn from(v: u8) -> Self {
        let parity = match v & 0x03 {
            0x01 | 0x03 => None,
            0x00 => Some(Parity::Odd),
            0x02 => Some(Parity::Even),
            _ => unreachable!(),
        };
        let stop_bits = 1 + ((v >> 2) & 1);
        let char_bits = 5 + ((v >> 3) & 0x3);
        let ie = ((v >> 5) & 1) != 0;
        let tx_break = ((v >> 6) & 1) != 0;
        let tr = (v >> 7) != 0;
        Self {
            word_length: char_bits,
            parity,
            stop_bits,
            ie,
            tx_break,
            tr,
        }
    }
}

/// Status register.
///
/// Bit layout:
/// - 0: DA: Data available
/// - 1: OE: Overrun error
/// - 2: PE: Parity error
/// - 3: FE: Framing error
/// - 4: ES: External status
/// - 5: PSI: Peripheral status interrupt
/// - 6: TSRE: Transmitter shift register empty
/// - 7: THRE: Transmitter holding register empty
#[derive(Debug, Default, Clone, Copy)]
pub struct Status {
    /// Data available.
    da: bool,
    /// Overrun error.
    oe: bool,
    /// Parity error.
    pe: bool,
    /// Framing error.
    fe: bool,
    /// External status.
    es: bool,
    /// Peripheral status interrupt.
    psi: bool,
    /// Transmitter shift register empty.
    tsre: bool,
    /// Transmitter holding register empty.
    thre: bool,
}
impl From<Status> for u8 {
    fn from(s: Status) -> u8 {
        (s.da as u8)
            | (s.oe as u8) << 1
            | (s.pe as u8) << 2
            | (s.fe as u8) << 3
            | (s.es as u8) << 4
            | (s.psi as u8) << 5
            | (s.tsre as u8) << 6
            | (s.thre as u8) << 7
    }
}
impl From<u8> for Status {
    fn from(v: u8) -> Self {
        Self {
            da: v & 1 != 0,
            oe: (v >> 1) & 1 != 0,
            pe: (v >> 2) & 1 != 0,
            fe: (v >> 3) & 1 != 0,
            es: (v >> 4) & 1 != 0,
            psi: (v >> 5) & 1 != 0,
            tsre: (v >> 6) & 1 != 0,
            thre: (v >> 7) & 1 != 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct Cdp1854 {
    /// Control register.
    control: Control,
    /// Receiver state.
    rx: Rx,
    /// Transmitter state.
    tx: Tx,
    /// Interrupt latches.
    interrupts: Interrupts,
    /// Previous PSI value.
    prev_psi: bool,
    /// Previous CTS value.
    prev_cts: bool,
}

#[derive(Debug, Default)]
struct Interrupts {
    da: bool,
    tx_ready: bool,
    tx_done: bool,
    psi: bool,
    cts: bool,
}
impl Interrupts {
    fn any(&self) -> bool {
        self.da || self.tx_ready || self.tx_done || self.psi || self.cts
    }
}

#[derive(Debug, Default)]
struct Rx {
    /// Current state.
    state: RxState,
    /// Tick counter for state machine.
    tick: u64,
    /// Shift register.
    shift: u8,
    /// Holding register.
    holding: u8,
    /// Parity tracking.
    parity: bool,
    /// Data available.
    da: bool,
    /// Overrun error.
    oe: bool,
    /// Parity error.
    pe: bool,
    /// Framing error.
    fe: bool,
    /// Paravirt receiver.
    pv_rx: Option<Receiver<u8>>,
}

#[derive(Debug, Default, Clone, Copy)]
enum RxState {
    /// Waiting for SDI to go high.
    #[default]
    WaitSdiHigh,
    /// Waiting for SDI to go low.
    WaitSdiLow,
    /// Received paravirt data.
    ParavirtData(u64),
    /// Receive start bit.
    Start,
    /// Receive data.
    Data(u8),
    /// Receive parity bit.
    Parity,
    /// Receive stop bit(s).
    Stop,
}

#[derive(Debug)]
struct Tx {
    /// Current state.
    state: TxState,
    /// Tick counter for state machine.
    tick: u64,
    /// Holding register.
    holding: u8,
    /// Shift register.
    shift: u8,
    /// Parity tracking.
    parity: bool,
    /// Latched value from [`Control::tx_break`].
    break_latch: bool,
    /// Transmitter shift register empty.
    tsre: bool,
    /// Transmitter holding register empty.
    thre: bool,
    /// Serial line output.
    sdo: bool,
    /// Paravirt sender.
    pv_tx: Option<Sender<u8>>,
}

#[derive(Debug, Default, Clone, Copy)]
enum TxState {
    /// Waiting for !thre and cts.
    #[default]
    Idle,
    /// Transmit start bit.
    Start,
    /// Transmit data.
    Data(u8),
    /// Transmit parity bit.
    Parity,
    /// Transmit stop bit(s).
    Stop,
}

impl Cdp1854 {
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure a paravirt channel for writing data to the receiver.
    #[must_use]
    pub fn with_pv_rx(mut self, rx: Receiver<u8>) -> Self {
        self.rx.pv_rx = Some(rx);
        self
    }

    /// Configure a paravirt channel for reading data from the sender.
    #[must_use]
    pub fn with_pv_tx(mut self, tx: Sender<u8>) -> Self {
        self.tx.pv_tx = Some(tx);
        self
    }

    /// Resets the device.
    fn clear(&mut self, pins: &mut u64) {
        // We only support mode 1 for now.
        assert!(Pins::Mode.get(*pins));
        self.control = Control::default();
        self.interrupts = Interrupts::default();
        self.rx.clear();
        self.tx.clear();
        self.prev_psi = Pins::Psi.get(*pins);
        self.prev_cts = Pins::Cts.get(*pins);
        self.update_pins(pins);
    }

    pub fn tick_tpb(&mut self, pins: &mut u64) {
        // Reset
        if !Pins::Clear.get(*pins) {
            self.clear(pins);
            return;
        }

        // Detect /PSI falling edge.
        let psi = Pins::Psi.get(*pins);
        let psi_falling = self.prev_psi && !psi;
        self.prev_psi = psi;
        if psi_falling {
            self.interrupts.psi = true;
        }

        // Detect /CTS rising edge.
        self.update_pv_cts(pins);
        let cts = Pins::Cts.get(*pins);
        let cts_rising = !self.prev_cts && cts;
        self.prev_cts = cts;
        if cts_rising {
            // Clears the tx break latch.
            self.tx.break_latch = false;

            // Raises an interrupt only if there's data to transmit.
            if !self.tx.thre || !self.tx.tsre {
                self.interrupts.cts = true;
            }
        }

        // Bus I/O.
        if Pins::Cs1.get3(*pins) == 0b101 && Pins::Tpb.get(*pins) {
            let rsel = Pins::Rsel.get(*pins);
            let rd_wr = Pins::RdWr.get(*pins);
            match (rsel, rd_wr) {
                (false, false) => self.bus_get_data(*pins),
                (false, true) => self.bus_put_data(pins),
                (true, false) => self.bus_get_control(*pins),
                (true, true) => self.bus_put_status(pins),
            }
        }
        self.update_pins(pins);
    }

    pub fn tick_rclock(&mut self, pins: &mut u64) {
        self.rx.tick(self.control, Pins::Sdi.get(*pins));
        if self.rx.da {
            self.interrupts.da = true;
        }
        self.update_pins(pins);
    }

    pub fn tick_tclock(&mut self, pins: &mut u64) {
        let prev_thre = self.tx.thre;
        let prev_tsre = self.tx.tsre;
        self.tx.tick(self.control, !Pins::Cts.get(*pins));
        if self.control.tr && !prev_thre && self.tx.thre {
            self.interrupts.tx_ready = true;
        }
        if self.tx.thre && !prev_tsre && self.tx.tsre {
            self.interrupts.tx_done = true;
        }
        self.update_pins(pins);
    }

    fn update_pv_cts(&self, pins: &mut u64) {
        if let Some(pv_tx) = &self.tx.pv_tx {
            Pins::Cts.set(pins, pv_tx.is_full());
        }
    }

    fn update_pins(&self, pins: &mut u64) {
        self.update_pv_cts(pins);
        Pins::Rts.set(pins, !self.control.tr);
        Pins::Da.set(pins, !self.rx.da);
        Pins::Fe.set(pins, self.rx.fe);
        Pins::PeOe.set(pins, self.rx.pe || self.rx.oe);
        Pins::Sdo.set(pins, !self.tx.break_latch && self.tx.sdo);
        Pins::Thre.set(pins, !self.tx.thre);
        Pins::Int.set(pins, !(self.control.ie && self.interrupts.any()));
    }

    fn bus_get_data(&mut self, pins: u64) {
        self.tx.holding = Pins::TBus0.get8(pins);
        self.tx.thre = false;
        self.interrupts.tx_ready = false;
        self.interrupts.tx_done = false;
    }

    fn bus_get_control(&mut self, pins: u64) {
        let control = Control::from(Pins::TBus0.get8(pins));
        // Loading the control register with TR inhibits changing other control bits.
        if control.tr {
            self.control.tr = true;
        } else {
            // Latch tx_break
            self.tx.break_latch |= control.tx_break;
            self.control = control;
        }
    }

    fn bus_put_data(&mut self, pins: &mut u64) {
        Pins::RBus0.set8(pins, self.rx.holding);
        self.rx.da = false;
        self.interrupts.da = false;
    }

    fn bus_put_status(&mut self, pins: &mut u64) {
        let status = Status {
            da: self.rx.da,
            oe: self.rx.oe,
            pe: self.rx.pe,
            fe: self.rx.fe,
            es: !Pins::Es.get(*pins),
            psi: self.interrupts.psi,
            tsre: self.tx.tsre,
            thre: self.tx.thre,
        };
        Pins::RBus0.set8(pins, status.into());
        self.interrupts.tx_ready = false;
        self.interrupts.tx_done = false;
        self.interrupts.psi = false;
        self.interrupts.cts = false;
    }
}

impl Rx {
    fn clear(&mut self) {
        let pv_rx = self.pv_rx.take();
        *self = Self {
            pv_rx,
            ..Default::default()
        }
    }

    fn tick(&mut self, ctrl: Control, sdi: bool) {
        (self.state, self.tick) = match (self.state, sdi, self.tick) {
            // Wait for line to go high.
            (RxState::WaitSdiHigh, true, _) => (RxState::WaitSdiLow, 0),
            // Initiate paravirt receive while SDI is high.
            (RxState::WaitSdiLow, true, t) => {
                let result = &self.pv_rx.as_ref().map(|pv_rx| pv_rx.try_recv());
                let byte = match result {
                    None | Some(Err(TryRecvError::Empty)) => None,
                    Some(Ok(byte)) => Some(byte),
                    Some(Err(TryRecvError::Disconnected)) => {
                        tracing::warn!("pv sender disconnected");
                        self.pv_rx = None;
                        None
                    }
                };
                if let Some(byte) = byte {
                    self.shift = *byte & ctrl.word_mask();
                    let ticks = ctrl.rx_ticks();
                    (RxState::ParavirtData(ticks), 0)
                } else {
                    (RxState::WaitSdiLow, t + 1)
                }
            }
            // Wait for transition from high->low.
            (RxState::WaitSdiLow, false, _) => (RxState::Start, 0),
            // Validate still low after 8 cycles.
            (RxState::Start, true, 7) => (RxState::WaitSdiLow, 0),
            (RxState::Start, false, 7) => {
                self.shift = 0;
                self.parity = matches!(ctrl.parity, Some(Parity::Even));
                (RxState::Data(0), 0)
            }
            // Read next bit after 16 cycles.
            (RxState::Data(mut bit), sdi, 15) => {
                self.shift |= (sdi as u8) << bit;
                self.parity ^= sdi;
                bit += 1;
                let state = if bit < ctrl.word_length {
                    RxState::Data(bit)
                } else if ctrl.parity.is_some() {
                    RxState::Parity
                } else {
                    RxState::Stop
                };
                (state, 0)
            }
            // Validate parity bit after 16 cycles.
            (RxState::Parity, sdi, 15) => {
                self.pe = sdi != self.parity;
                (RxState::Stop, 0)
            }
            // Validate first stop bit after 16 cycles.
            (RxState::Stop, sdi, 15) => {
                self.holding = self.shift;
                self.fe = !sdi;
                self.oe = self.da;
                self.da = true;
                if sdi {
                    (RxState::WaitSdiLow, 0)
                } else {
                    (RxState::WaitSdiHigh, 0)
                }
            }
            // Complete paravirt receive.
            (RxState::ParavirtData(ticks), sdi, t) if t == ticks - 1 => {
                self.holding = self.shift;
                self.fe = false;
                self.oe = self.da;
                self.da = true;
                if sdi {
                    (RxState::WaitSdiLow, 0)
                } else {
                    (RxState::WaitSdiHigh, 0)
                }
            }
            (s, _, t) => (s, t.wrapping_add(1)),
        }
    }
}

impl Default for Tx {
    fn default() -> Self {
        Self {
            state: TxState::Idle,
            tick: 0,
            holding: 0,
            shift: 0,
            parity: false,
            break_latch: false,
            tsre: true,
            thre: true,
            sdo: true,
            pv_tx: None,
        }
    }
}

impl Tx {
    fn clear(&mut self) {
        let pv_tx = self.pv_tx.take();
        *self = Self {
            pv_tx,
            ..Default::default()
        }
    }

    fn tick(&mut self, ctrl: Control, cts: bool) {
        (self.state, self.tick) = match (self.state, self.tick) {
            (TxState::Idle, _) if cts && !self.thre => {
                // Shift register loaded 1/2 cycle after the trailing edge of TPB
                self.shift = self.holding;
                self.tsre = false;
                (TxState::Start, 0)
            }
            (TxState::Start, 0) => {
                // Start bit 1/2 cycle after loading shift register. This also resets the break
                // latch.
                self.sdo = false;
                self.break_latch = false;
                // THRE updated 1 cycle after loading shift register.
                self.thre = true;
                // Initialize parity tracking.
                self.parity = ctrl.parity.is_some_and(|p| matches!(p, Parity::Even));
                // Paravirt send.
                if let Some(pv_tx) = &self.pv_tx {
                    let byte = self.shift & ctrl.word_mask();
                    if let Err(e) = pv_tx.try_send(byte) {
                        match e {
                            TrySendError::Full(_) => unreachable!("cts invalid"),
                            TrySendError::Disconnected(_) => {
                                tracing::warn!("pv receiver disconnected");
                                self.pv_tx = None;
                            }
                        }
                    }
                }
                (TxState::Start, 1)
            }
            (TxState::Start, 15) => (TxState::Data(0), 0),
            (TxState::Data(n), 0) => {
                let bit = self.shift & 1 == 1;
                self.sdo = bit;
                self.parity ^= bit;
                self.shift >>= 1;
                (TxState::Data(n), 1)
            }
            (TxState::Data(mut n), 15) => {
                n += 1;
                let state = if n < ctrl.word_length {
                    TxState::Data(n)
                } else if ctrl.parity.is_some() {
                    TxState::Parity
                } else {
                    TxState::Stop
                };
                (state, 0)
            }
            (TxState::Parity, 0) => {
                self.sdo = self.parity;
                (TxState::Parity, 1)
            }
            (TxState::Parity, 15) => (TxState::Stop, 0),
            (TxState::Stop, 0) => {
                self.sdo = true;
                (TxState::Stop, 1)
            }
            (TxState::Stop, t) if t == ctrl.stop_ticks() - 1 => {
                if self.thre || !cts {
                    self.tsre = true;
                    (TxState::Idle, 0)
                } else {
                    self.shift = self.holding;
                    (TxState::Start, 0)
                }
            }
            (s, t) => (s, t.wrapping_add(1)),
        };
    }
}
