//! General Instrument AY-5-1013 emulation

use std::fmt::Display;

use crate::chips::bits;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub struct Ay51013Pins(pub u64);
impl Default for Ay51013Pins {
    fn default() -> Self {
        Self(u64::MAX & Self::mask_all())
    }
}
impl std::fmt::Debug for Ay51013Pins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Ay51013Pins")
            .field(&format!("{:034b}", self.0))
            .finish()
    }
}
impl Ay51013Pins {
    pub const fn mask_all() -> u64 {
        (1 << ((Pin::Xr as u8) + 1)) - 1
    }

    /// Read buffer.
    pub fn get_rd(self) -> u8 {
        bits::get_8(self.0, Pin::Rd1 as u8)
    }
    fn set_rd(&mut self, val: u8) {
        self.0 = bits::set_8(self.0, Pin::Rd1 as u8, val);
    }

    /// Data available.
    pub fn get_dav(self) -> bool {
        bits::get_1(self.0, Pin::Dav as u8)
    }
    fn set_dav(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Dav as u8, val);
    }

    /// Parity error.
    pub fn get_pe(self) -> bool {
        bits::get_1(self.0, Pin::Pe as u8)
    }
    fn set_pe(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Pe as u8, val);
    }

    /// Framing error.
    pub fn get_fe(self) -> bool {
        bits::get_1(self.0, Pin::Fe as u8)
    }
    fn set_fe(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Fe as u8, val);
    }

    /// Overrun.
    pub fn get_or(self) -> bool {
        bits::get_1(self.0, Pin::Or as u8)
    }
    fn set_or(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Or as u8, val);
    }

    /// Transmit buffer empty.
    pub fn get_tbmt(self) -> bool {
        bits::get_1(self.0, Pin::Tbmt as u8)
    }
    fn set_tbmt(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Tbmt as u8, val);
    }

    /// Transmit end of character.
    #[cfg(test)]
    fn get_eoc(self) -> bool {
        bits::get_1(self.0, Pin::Eoc as u8)
    }
    fn set_eoc(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Eoc as u8, val);
    }

    /// Transmit output.
    pub fn get_so(self) -> bool {
        bits::get_1(self.0, Pin::So as u8)
    }
    fn set_so(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::So as u8, val);
    }

    /// Receive input.
    fn get_si(self) -> bool {
        bits::get_1(self.0, Pin::Si as u8)
    }
    pub fn set_si(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Si as u8, val);
    }

    fn get_db(self) -> u8 {
        bits::get_8(self.0, Pin::Db1 as u8)
    }
    pub fn set_db(&mut self, val: u8) {
        self.0 = bits::set_8(self.0, Pin::Db1 as u8, val);
    }

    /// Number of stop bits per character.
    fn get_tsb(self) -> bool {
        bits::get_1(self.0, Pin::Tsb as u8)
    }
    fn set_tsb(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Tsb as u8, val);
    }

    /// Parity select.
    fn get_eps(self) -> bool {
        bits::get_1(self.0, Pin::Eps as u8)
    }
    fn set_eps(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Eps as u8, val);
    }

    /// No parity.
    fn get_np(self) -> bool {
        bits::get_1(self.0, Pin::Np as u8)
    }
    fn set_np(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Np as u8, val);
    }

    /// Number of bits per character.
    fn get_nb(self) -> u8 {
        bits::get_2(self.0, Pin::Nb1 as u8)
    }
    fn set_nb(&mut self, val: u8) {
        self.0 = bits::set_2(self.0, Pin::Nb1 as u8, val);
    }

    /// Configuration strobe.
    fn get_cs(self) -> bool {
        bits::get_1(self.0, Pin::Cs as u8)
    }
    fn set_cs(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Cs as u8, val);
    }

    /// Data strobe (negated).
    fn get_ds(self) -> bool {
        bits::get_1(self.0, Pin::Ds as u8)
    }
    pub fn set_ds(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Ds as u8, val);
    }

    /// Read data bits enable (negated).
    ///
    /// When set to 0, the RD1..RD8 lines are active.
    fn get_rde(self) -> bool {
        bits::get_1(self.0, Pin::Rde as u8)
    }
    fn set_rde(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Rde as u8, val);
    }

    /// Status word enable (negated).
    ///
    /// When set to 0, puts PE, FE, OR, DAV, TBMT on the output lines.
    fn get_swe(self) -> bool {
        bits::get_1(self.0, Pin::Swe as u8)
    }
    fn set_swe(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Swe as u8, val);
    }

    /// Reset DAV (negated).
    ///
    /// When set to 0, resets the DAV line.
    fn get_rdav(self) -> bool {
        bits::get_1(self.0, Pin::Rdav as u8)
    }
    pub fn set_rdav(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Rdav as u8, val);
    }

    /// External reset.
    fn get_xr(self) -> bool {
        bits::get_1(self.0, Pin::Xr as u8)
    }
    fn set_xr(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Xr as u8, val);
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum Pin {
    // out
    Rd1,
    Rd2,
    Rd3,
    Rd4,
    Rd5,
    Rd6,
    Rd7,
    Rd8,
    Dav,
    Pe,
    Fe,
    Or,
    Tbmt,
    Eoc,
    So,

    // in
    Si,
    Db1,
    Db2,
    Db3,
    Db4,
    Db5,
    Db6,
    Db7,
    Db8,
    Tsb,
    Eps,
    Np,
    Nb1,
    Nb2,
    Cs,
    Ds,
    Rde,
    Swe,
    Rdav,
    Xr,
}

#[derive(Debug, Default)]
pub struct Ay51013 {
    ctrl: Ctrl,
    tx: Tx,
    rx: Rx,
    cycle: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum Parity {
    Even,
    Odd,
}

#[derive(Debug, Clone, Copy)]
struct Ctrl {
    char_bits: u8,
    parity: Option<Parity>,
    stop_bits: u8,
}

#[derive(Debug, Default)]
struct Rx {
    // internal state
    state: RxState,
    tick: u64,
    shift: u8,
    parity: bool,
    /// output pins
    buffer: u8,
    dav: bool,
    pe: bool,
    fe: bool,
    or: bool,
}

#[derive(Default, Debug, Clone, Copy)]
enum RxState {
    #[default]
    Idle,
    WaitSi,
    Start,
    Data(u8),
    Parity,
    Stop,
    Dav,
}

#[derive(Debug)]
struct Tx {
    // internal state
    state: TxState,
    tick: u8,
    buffer: u8,
    shift: u8,
    parity: bool,
    prev_ds: bool,
    // output pins
    tbmt: bool,
    eoc: bool,
    so: bool,
}

#[derive(Default, Debug, Clone, Copy)]
enum TxState {
    #[default]
    Idle,
    Start,
    Data(u8),
    Parity,
    Stop,
    Eoc,
}

impl Display for Ay51013 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Tx({:?} tick:{} so:{}) Rx({:?} tick:{} shift:{:x})",
            self.tx.state,
            self.tx.tick,
            self.tx.so as u8,
            self.rx.state,
            self.rx.tick,
            self.rx.shift
        )
    }
}

impl Ay51013 {
    /// Configures the device and resets it.
    ///
    /// When this function returns, the following pins are set:
    ///  - NB1, NB2, TSB, NP, EPS: According to configuration.
    ///  - CS: 0
    ///  - /DS: 1
    ///  - /RDE: 0
    ///  - /SWE: 0
    ///  - /RDAV: 1
    ///  - XR: 0
    pub fn configure(
        &mut self,
        pins: &mut Ay51013Pins,
        char_bits: u8,
        parity: impl Into<Option<Parity>>,
        stop_bits: u8,
    ) {
        assert!((5..=8).contains(&char_bits));
        assert!((1..=2).contains(&stop_bits));
        pins.set_nb(char_bits - 5);
        let parity = parity.into();
        pins.set_eps(parity.is_some_and(|p| matches!(p, Parity::Even)));
        pins.set_np(parity.is_none());
        pins.set_tsb(stop_bits == 2);
        pins.set_cs(true);
        self.reset(pins);
        pins.set_cs(false);
    }

    /// Resets the device without updating its configuration.
    ///
    /// When this function returns, the following pins are set:
    ///  - /DS: 1
    ///  - /RDE: 0
    ///  - /SWE: 0
    ///  - /RDAV: 1
    ///  - XR: 0
    pub fn reset(&mut self, pins: &mut Ay51013Pins) {
        pins.set_ds(true);
        pins.set_rde(false);
        pins.set_swe(false);
        pins.set_rdav(true);
        pins.set_xr(true);
        self.tick(pins);
        pins.set_xr(false);
    }

    pub fn tick(&mut self, pins: &mut Ay51013Pins) {
        if pins.get_xr() {
            self.rx.reset();
            self.tx.reset();
        }

        self.ctrl.tick(*pins);
        self.rx.tick(self.ctrl, pins);
        self.tx.tick(self.ctrl, pins);
        self.cycle += 1;
    }

    pub fn is_tx_idle(&self) -> bool {
        matches!(self.tx.state, TxState::Idle)
    }
}

impl Display for Ctrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let p = match self.parity {
            None => 'n',
            Some(Parity::Odd) => 'o',
            Some(Parity::Even) => 'e',
        };
        write!(f, "{}{p}{}", self.char_bits, self.stop_bits)
    }
}
impl Default for Ctrl {
    fn default() -> Self {
        Self {
            char_bits: 8,
            parity: None,
            stop_bits: 1,
        }
    }
}
impl Ctrl {
    fn tick(&mut self, pins: Ay51013Pins) {
        if pins.get_cs() {
            self.parity = match (pins.get_np(), pins.get_eps()) {
                (true, _) => None,
                (false, false) => Some(Parity::Odd),
                (false, true) => Some(Parity::Even),
            };
            self.char_bits = pins.get_nb() + 5;
            self.stop_bits = (pins.get_tsb() as u8) + 1;
        }
    }
}

impl Rx {
    fn reset(&mut self) {
        self.dav = false;
        self.pe = false;
        self.fe = false;
        self.or = false;
        self.shift = 0;
    }

    fn tick(&mut self, ctrl: Ctrl, pins: &mut Ay51013Pins) {
        (self.state, self.tick) = match (self.state, pins.get_si(), self.tick) {
            (RxState::Idle, true, _) => (RxState::WaitSi, 0),
            (RxState::WaitSi, false, _) => (RxState::Start, 0),
            (RxState::Start, true, 7) => (RxState::WaitSi, 0),
            (RxState::Start, false, 7) => {
                self.shift = 0;
                self.parity = matches!(ctrl.parity, Some(Parity::Even));
                (RxState::Data(0), 0)
            }
            (RxState::Data(mut bit), si, 15) => {
                self.shift |= (si as u8) << bit;
                self.parity ^= si;
                bit += 1;
                let state = if bit < ctrl.char_bits {
                    RxState::Data(bit)
                } else if ctrl.parity.is_some() {
                    RxState::Parity
                } else {
                    RxState::Stop
                };
                (state, 0)
            }
            (RxState::Parity, si, 15) => {
                self.pe = si != self.parity;
                (RxState::Stop, 0)
            }
            (RxState::Stop, si, 15) => {
                self.fe = !si;
                self.or = self.dav;
                self.dav = false;
                (RxState::Dav, 0)
            }
            (RxState::Dav, si, 0) => {
                self.buffer = self.shift;
                self.dav = true;
                let state = if si { RxState::WaitSi } else { RxState::Idle };
                (state, 0)
            }
            (s, _, t) => (s, t.overflowing_add(1).0),
        };

        // Reset DAV
        if !pins.get_rdav() {
            self.dav = false;
        }

        // Read status
        if pins.get_swe() {
            pins.set_dav(false);
            pins.set_pe(false);
            pins.set_fe(false);
            pins.set_or(false);
        } else {
            pins.set_dav(self.dav);
            pins.set_pe(self.pe);
            pins.set_fe(self.fe);
            pins.set_or(self.or);
        }

        // Read data
        pins.set_rd(if pins.get_rde() { 0 } else { self.buffer });
    }
}

impl Default for Tx {
    fn default() -> Self {
        Self {
            state: TxState::Idle,
            prev_ds: true,
            tick: 0,
            buffer: 0,
            shift: 0,
            parity: false,
            tbmt: true,
            eoc: true,
            so: true,
        }
    }
}
impl Tx {
    fn reset(&mut self) {
        *self = Default::default();
    }

    fn tick(&mut self, ctrl: Ctrl, pins: &mut Ay51013Pins) {
        // Latch data bits on the DS rising edge, regardless of what state we're in.
        let ds = pins.get_ds();
        if !self.prev_ds && ds {
            self.buffer = pins.get_db();
            self.tbmt = false;
            self.prev_ds = true;
        }
        self.prev_ds = ds;

        (self.state, self.tick) = match (self.state, self.tick) {
            (TxState::Idle, _) if !self.tbmt => {
                self.start(ctrl);
                (TxState::Start, 1)
            }
            (TxState::Start, 1) => {
                // Reset EOC after first cycle.
                self.eoc = false;
                (TxState::Start, 2)
            }
            (TxState::Start, 5) => {
                // Reset TBMT within 6 cycles.
                self.tbmt = true;
                (TxState::Start, 6)
            }
            (TxState::Start, 15) => (TxState::Data(0), 0),
            (TxState::Data(bit), 0) => {
                let val = self.shift & 0x1 == 1;
                self.so = val;
                self.parity ^= val;
                self.shift >>= 1;
                (TxState::Data(bit), 1)
            }
            (TxState::Data(mut bit), 15) => {
                bit += 1;
                let state = if bit < ctrl.char_bits {
                    TxState::Data(bit)
                } else if ctrl.parity.is_some() {
                    TxState::Parity
                } else {
                    TxState::Stop
                };
                (state, 0)
            }
            (TxState::Parity, 0) => {
                self.so = self.parity;
                (TxState::Parity, 1)
            }
            (TxState::Parity, 15) => (TxState::Stop, 0),
            (TxState::Stop, 0) => {
                self.so = true;
                (TxState::Stop, 1)
            }
            (TxState::Stop, t) if t == ctrl.stop_bits * 16 - 1 => (TxState::Eoc, 0),
            (TxState::Eoc, 0) => {
                self.eoc = true;
                if !self.tbmt {
                    self.start(ctrl);
                    (TxState::Start, 1)
                } else {
                    (TxState::Idle, 0)
                }
            }
            (s, t) => (s, t.overflowing_add(1).0),
        };

        pins.set_tbmt(if pins.get_swe() { false } else { self.tbmt });
        pins.set_eoc(self.eoc);
        pins.set_so(self.so);
    }

    fn start(&mut self, ctrl: Ctrl) {
        self.parity = ctrl.parity.is_some_and(|p| matches!(p, Parity::Even));
        self.shift = self.buffer;
        self.so = false;
    }
}
