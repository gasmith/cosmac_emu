use std::fmt::Display;

use super::Ay51013Pins;

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
