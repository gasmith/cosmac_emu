//! RCA CDP 1802 emulation
//!
//! # Cycle timing
//!
//! Cycle timing on the physical device is more granular than what is represented here. A physical
//! device would strobe certain signals on rising clock edges, and others on falling clock edges.
//! Here we treat each clock cycle as a single tick, so we need to make a distinction.
//!
//! 0: MRD hi, N sampling (S1)
//! 1: TPA hi, EF sampling (S1)
//! 2: TPA lo, MRD lo (for reads)
//! 3: bus sampling, update registers
//! 4: post-op inc/dec
//! 5: MWR lo (for writes)
//! 6: TPB hi
//! 7: TPB lo, MWR hi, DMA sampling (S1, S2, S3), INTR sampling (S1, S2)
//!

use std::fmt::Display;

use rand::distr::{Distribution, StandardUniform};

use super::math;

mod memory;
mod pins;
#[cfg(test)]
mod tests;

pub use memory::{Memory, MemoryRange};
pub use pins::Cdp1802Pins;

#[derive(Debug, Clone, Copy)]
enum Mode {
    Load,
    Reset,
    Pause,
    Run,
}
fn mode(pins: Cdp1802Pins) -> Mode {
    match (pins.get_clear(), pins.get_wait()) {
        (false, false) => Mode::Load,
        (false, true) => Mode::Reset,
        (true, false) => Mode::Pause,
        (true, true) => Mode::Run,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum State {
    Init(u8),
    Fetch(u8),
    Execute(u8),
    DmaIn(u8),
    DmaOut(u8),
    Interrupt(u8),
}
impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (c, t) = match self {
            State::Fetch(t) => ("S0", t),
            State::Init(t) | State::Execute(t) => ("S1", t),
            State::DmaIn(t) | State::DmaOut(t) => ("S2", t),
            State::Interrupt(t) => ("S3", t),
        };
        write!(f, "{c}.{t:x}")
    }
}
impl Distribution<State> for StandardUniform {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> State {
        let t: u8 = rng.random_range(0..16);
        match rng.random_range(0..6) {
            0 => State::Init(t),
            1 => State::Fetch(t),
            2 => State::Execute(t),
            3 => State::DmaIn(t),
            4 => State::DmaOut(t),
            5 => State::Interrupt(t),
            _ => unreachable!(),
        }
    }
}
impl State {
    fn tick(self) -> u8 {
        match self {
            Self::Init(t)
            | Self::Fetch(t)
            | Self::Execute(t)
            | Self::DmaIn(t)
            | Self::DmaOut(t)
            | Self::Interrupt(t) => t,
        }
    }

    fn as_sc(self) -> u8 {
        match self {
            Self::Fetch(_) => 0,
            Self::Init(_) | Self::Execute(_) => 1,
            Self::DmaIn(_) | Self::DmaOut(_) => 2,
            Self::Interrupt(_) => 3,
        }
    }
}

#[derive(Debug)]
pub struct Cdp1802 {
    pub state: State,

    // Standard registers (q is in pins_out).
    pub d: u8,
    pub df: bool,
    pub b: u8,
    pub r: [u16; 16],
    pub p: u8,
    pub x: u8,
    pub i: u8,
    pub n: u8,
    pub t: u8,
    pub ie: bool,

    /// Buffered pin outputs
    out: Cdp1802Pins,

    /// EF1 is the least significant bit.
    ef: u8,
    prev_mode: Mode,
}
impl Display for Cdp1802 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{s} ie={ie} d={d:02x}.{df} x={x:02x}:{rx:04x} p={p:02x}:{rp:04x} in={i:02x}",
            s = self.state,
            ie = self.ie as u8,
            d = self.d,
            df = self.df as u8,
            x = self.x,
            rx = self.r[self.x as usize],
            p = self.p,
            rp = self.r[self.p as usize],
            i = self.i << 4 | self.n,
        )
    }
}
impl Distribution<Cdp1802> for StandardUniform {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Cdp1802 {
        Cdp1802 {
            state: rng.random(),
            d: rng.random(),
            df: rng.random(),
            b: rng.random(),
            r: rng.random(),
            p: rng.random_range(0..16),
            x: rng.random_range(0..16),
            i: rng.random_range(0..16),
            n: rng.random_range(0..16),
            t: rng.random(),
            ie: rng.random(),
            out: Cdp1802Pins(rng.random::<u64>() & Cdp1802Pins::mask_bus_out()),
            ef: rng.random::<u8>() & 0xf,
            prev_mode: Mode::Reset,
        }
    }
}
impl Default for Cdp1802 {
    fn default() -> Self {
        Self {
            state: State::Init(0),
            d: 0,
            df: false,
            b: 0,
            r: [0; 16],
            p: 0,
            x: 0,
            i: 0,
            n: 0,
            t: 0,
            ie: true,
            out: Cdp1802Pins::default(),
            ef: 0,
            prev_mode: Mode::Reset,
        }
    }
}
impl Cdp1802 {
    pub fn reset(&mut self, pins: &mut Cdp1802Pins) {
        self.tick_reset();
        self.update_pins(pins);
    }

    /// Returns true if the device is waiting for an external event.
    pub fn is_waiting(&self, pins: Cdp1802Pins) -> bool {
        let dma_intr = !pins.get_dma_in() || !pins.get_dma_out() || !pins.get_intr();
        match (mode(pins), self.state) {
            (Mode::Load, State::Execute(7)) => !dma_intr,
            (Mode::Load, _) => false,
            (Mode::Reset, State::Init(0)) => true,
            (Mode::Reset, _) => false,
            (Mode::Pause, _) => true,
            (Mode::Run, State::Execute(7)) if matches!((self.i, self.n), (0, 0)) => !dma_intr,
            (Mode::Run, _) => false,
        }
    }

    pub fn is_fetch_tick0(&self) -> bool {
        matches!(self.state, State::Fetch(0))
    }

    /// Returns the current opcode, if the chip is entering S1.
    pub fn get_exec_opcode(&self) -> Option<u8> {
        if matches!(self.state, State::Execute(0)) {
            Some(self.i << 4 | self.n)
        } else {
            None
        }
    }

    pub fn rp(&self) -> u16 {
        self.r[self.p as usize]
    }

    pub fn tick(&mut self, pins: &mut Cdp1802Pins) {
        let mode = mode(*pins);
        match mode {
            Mode::Pause => return,
            Mode::Reset => self.tick_reset(),
            Mode::Load => self.tick_load(*pins),
            Mode::Run => self.tick_run(*pins),
        }
        self.prev_mode = mode;
        self.update_pins(pins);
    }

    fn update_pins(&self, pins: &mut Cdp1802Pins) {
        // Always drive the bus during init.
        let init = matches!(self.state, State::Init(_));
        // Drive the bus during write cycles...
        let mwr = !self.out.get_mwr();
        // ... but not during INP instructions.
        let inp = matches!(self.state, State::Execute(_)) && self.i == 6 && self.n >= 9;
        let mask = if init || (mwr && !inp) {
            Cdp1802Pins::mask_bus_out()
        } else {
            Cdp1802Pins::mask_out()
        };
        pins.set_masked(self.out, mask);
    }

    fn tick_reset(&mut self) {
        self.i = 0;
        self.n = 0;
        self.ie = true;
        self.state = State::Init(0);
        self.out = Cdp1802Pins(0);
        self.out.set_mrd(true);
        self.out.set_mwr(true);
        self.out.set_sc(self.state.as_sc());
    }

    fn tick_load(&mut self, pins: Cdp1802Pins) {
        // If we came from Pause or Run, initiate a reset. It's not clear from the datasheets
        // whether this is the right thing to do, but it seems sane.
        if matches!(self.prev_mode, Mode::Pause | Mode::Run) {
            self.tick_reset();
            return;
        }
        self.tick_timing_pulses(matches!(self.state, State::DmaIn(_)));
        match self.state {
            State::Init(t) => self.tick_init(t),
            State::DmaIn(t) => self.tick_dma_in(t),
            State::Execute(2) => {
                let addr = self.glo(self.p);
                if addr > 0 {
                    self.out.set_mrd(false);
                    self.out.set_ma(addr.overflowing_sub(1).0);
                }
            }
            State::Execute(_) => (),
            s => unreachable!("{:?}", s),
        }
        self.state = match self.state {
            State::Init(8) | State::Execute(7) | State::DmaIn(7) => {
                if !pins.get_dma_in() {
                    State::DmaIn(0)
                } else {
                    State::Execute(0)
                }
            }
            State::Init(t) => State::Init(t + 1),
            State::Execute(t) => State::Execute(t + 1),
            State::DmaIn(t) => State::DmaIn(t + 1),
            s => unreachable!("{:?}", s),
        };
    }

    fn tick_run(&mut self, pins: Cdp1802Pins) {
        self.tick_timing_pulses(true);
        match self.state {
            State::Init(t) => self.tick_init(t),
            State::Fetch(t) => self.tick_fetch(t, pins),
            State::Execute(t) => self.tick_execute(t, pins),
            State::DmaIn(t) => self.tick_dma_in(t),
            State::DmaOut(t) => self.tick_dma_out(t),
            State::Interrupt(t) => self.tick_interrupt(t),
        };
        self.state = match self.state {
            // Interrupts are forcibly disabled during this transition.
            State::Init(8) => self.sample_dma_intr(pins, false).unwrap_or(State::Fetch(0)),

            // IDL instruction stays in S1.
            State::Execute(7) if matches!((self.i, self.n), (0, 0)) => self
                .sample_dma_intr(pins, self.ie)
                .unwrap_or(State::Execute(0)),

            // Non-long-branch instructions return to S0.
            State::Execute(7) if self.i != 0xc => self
                .sample_dma_intr(pins, self.ie)
                .unwrap_or(State::Fetch(0)),

            // All other S1/S2/S3 completions return to S0.
            State::Execute(15) | State::DmaIn(7) | State::DmaOut(7) | State::Interrupt(7) => self
                .sample_dma_intr(pins, self.ie)
                .unwrap_or(State::Fetch(0)),

            // S0 always goes to S1.
            State::Fetch(7) => State::Execute(0),

            // Increment tick counts.
            State::Init(t) => State::Init(t + 1),
            State::Fetch(t) => State::Fetch(t + 1),
            State::Execute(t) => State::Execute(t + 1),
            State::DmaIn(t) => State::DmaIn(t + 1),
            State::DmaOut(t) => State::DmaOut(t + 1),
            State::Interrupt(t) => State::Interrupt(t + 1),
        }
    }

    fn tick_timing_pulses(&mut self, enable_tpa: bool) {
        match self.state.tick() & 7 {
            0 => {
                self.out.set_sc(self.state.as_sc());
                self.out.set_mrd(true);
            }
            1 if enable_tpa => self.out.set_tpa(true),
            2 => self.out.set_tpa(false),
            6 => self.out.set_tpb(true),
            7 => {
                self.out.set_mwr(true);
                self.out.set_tpb(false);
            }
            _ => (),
        };
    }

    fn tick_init(&mut self, tick: u8) {
        if tick == 1 {
            self.x = 0;
            self.p = 0;
            self.r[0] = 0;
        }
    }

    fn tick_fetch(&mut self, tick: u8, pins: Cdp1802Pins) {
        match tick {
            0 | 2 => self.tick_mrd(tick, self.p),
            3 => {
                let inst = pins.get_bus();
                self.i = (inst & 0xf0) >> 4;
                self.n = inst & 0x0f;
                // Don't increment the program counter for the IDL instruction.
                if inst != 0 {
                    self.inc(self.p);
                }
            }
            _ => (),
        }
    }

    fn tick_execute(&mut self, tick: u8, pins: Cdp1802Pins) {
        match (self.i, self.n, tick) {
            // Ensure that instr nibbles and ticks are within bounds.
            (16.., _, _) | (_, 16.., _) | (_, _, 16..) => unreachable!(),

            // IDL (00)
            (0, 0, t @ (0 | 2)) => self.tick_mrd(t, 0),
            (0, 0, _) => (),

            // LDN (0n) / LDA (4n)
            (0 | 4, n, t @ (0 | 2)) => self.tick_mrd(t, n),
            (0 | 4, _, 3) => self.d = pins.get_bus(),
            (4, n, 4) => self.inc(n),
            (0 | 4, _, _) => (),

            // LDXA (72)
            (7, 2, t @ (0 | 2)) => self.tick_mrd(t, self.x),
            (7, 2, 3) => self.d = pins.get_bus(),
            (7, 2, 4) => self.inc(self.x),
            (7, 2, _) => (),

            // LDX (f0)
            (0xf, 0, t @ (0 | 2)) => self.tick_mrd(t, self.x),
            (0xf, 0, 3) => self.d = pins.get_bus(),
            (0xf, 0, _) => (),

            // LDI (f8)
            (0xf, 8, t @ (0 | 2)) => self.tick_mrd(t, self.p),
            (0xf, 8, 3) => self.d = pins.get_bus(),
            (0xf, 8, 4) => self.inc(self.p),
            (0xf, 8, _) => (),

            // STR (5n)
            (5, n, t) => self.tick_store(t, n, self.d),

            // STXD (73)
            (7, 3, 4) => self.dec(self.x),
            (7, 3, t) => self.tick_store(t, self.x, self.d),

            // INC (1n) / DEC (2n)
            (1, n, 4) => self.inc(n),
            (2, n, 4) => self.dec(n),
            (1 | 2, _, _) => (),

            // IRX (60)
            (6, 0, 4) => self.inc(self.x),
            (6, 0, _) => (),

            // GLO (8n) / GHI (9n) / PLO (an) / PHI (bn)
            (8, n, 3) => self.d = self.glo(n),
            (9, n, 3) => self.d = self.ghi(n),
            (0xa, n, 3) => self.plo(n, self.d),
            (0xb, n, 3) => self.phi(n, self.d),
            (8..=0xb, _, _) => (),

            // OR (f1) / AND (f2) / XOR (f3)
            (0xf, 1..=3, t @ (0 | 2)) => self.tick_mrd(t, self.x),
            (0xf, 1, 3) => self.d |= pins.get_bus(),
            (0xf, 2, 3) => self.d &= pins.get_bus(),
            (0xf, 3, 3) => self.d ^= pins.get_bus(),
            (0xf, 1..=3, _) => (),

            // ORI (f9) / ANI (fa) / XRI (fb)
            (0xf, 9..=0xb, t @ (0 | 2)) => self.tick_mrd(t, self.p),
            (0xf, 9, 3) => {
                self.d |= pins.get_bus();
                self.inc(self.p);
            }
            (0xf, 0xa, 3) => {
                self.d &= pins.get_bus();
                self.inc(self.p);
            }
            (0xf, 0xb, 3) => {
                self.d ^= pins.get_bus();
                self.inc(self.p);
            }
            (0xf, 9..=0xb, _) => (),

            // SHR (f6) / SHL (fe)
            (0xf, 6, 3) => (self.d, self.df) = (self.d >> 1, self.d & 0x1 != 0),
            (0xf, 0xe, 3) => (self.d, self.df) = (self.d << 1, self.d & 0x80 != 0),
            (0xf, 6 | 0xe, _) => (),

            // SHRC (76) / SHLC (7e)
            (7, 6, 3) => {
                let df = self.d & 0x01 != 0;
                self.d >>= 1;
                if self.df {
                    self.d |= 0x80;
                }
                self.df = df;
            }
            (7, 0xe, 3) => {
                let df = self.d & 0x80 != 0;
                self.d <<= 1;
                if self.df {
                    self.d |= 0x01;
                }
                self.df = df;
            }
            (7, 6 | 0xe, _) => (),

            // ADD (f4) / ADC (74)
            (0xf, 4, t) => self.tick_add(pins, t, self.x, false),
            (7, 4, t) => self.tick_add(pins, t, self.x, true),

            // ADI (fc) / ADCI (7c)
            (0xf | 7, 0xc, 4) => self.inc(self.p),
            (0xf, 0xc, t) => self.tick_add(pins, t, self.p, false),
            (7, 0xc, t) => self.tick_add(pins, t, self.p, true),

            // SD (f5) / SDB (75)
            (0xf, 5, t) => self.tick_subd(pins, t, self.x, false),
            (7, 5, t) => self.tick_subd(pins, t, self.x, true),

            // SDI (fd) / SDBI (7d)
            (0xf | 7, 0xd, 4) => self.inc(self.p),
            (0xf, 0xd, t) => self.tick_subd(pins, t, self.p, false),
            (7, 0xd, t) => self.tick_subd(pins, t, self.p, true),

            // SM (f7) / SMB (77)
            (0xf, 7, t) => self.tick_subm(pins, t, self.x, false),
            (7, 7, t) => self.tick_subm(pins, t, self.x, true),

            // SMI (ff) / SMBI (7f)
            (0xf | 7, 0xf, 4) => self.inc(self.p),
            (0xf, 0xf, t) => self.tick_subm(pins, t, self.p, false),
            (7, 0xf, t) => self.tick_subm(pins, t, self.p, true),

            // NOP (c4)
            (0xc, 4, _) => (),

            // Bxx (3n) / LBxx (cn)
            (3, n, t) => self.tick_bxx(pins, t, n),
            (0xc, n, t) => self.tick_lbxx(pins, t, n),

            // SEP (dn) / SEX (en)
            (0xd, n, 3) => self.p = n,
            (0xe, n, 3) => self.x = n,
            (0xd | 0xe, _, _) => (),

            // REQ (7a) / SEQ (7b)
            (7, n @ (0xa | 0xb), 3) => self.out.set_q(n & 1 != 0),
            (7, 0xa | 0xb, _) => (),

            // SAV (78) / MARK (79)
            (7, 8, t) => self.tick_store(t, self.x, self.t),
            (7, 9, t) => self.tick_mark(t),

            // RET (70) / DIS (71)
            (7, 0, t) => self.tick_ret(pins, t, true),
            (7, 1, t) => self.tick_ret(pins, t, false),

            // OUT (61..67) / IN (69..6f)
            (6, n @ 1..=7, t) => self.tick_output(t, n),
            (6, n @ 9..=0xf, t) => self.tick_input(pins, t, n - 8),

            // Resv
            (6, 8, _) => (),
        }
    }

    fn tick_dma_in(&mut self, tick: u8) {
        match tick {
            0 => self.out.set_ma(self.ghi(0)),
            2 => self.out.set_ma(self.glo(0)),
            4 => self.inc(0),
            5 => self.out.set_mwr(false),
            _ => (),
        }
    }

    fn tick_dma_out(&mut self, tick: u8) {
        match tick {
            0 | 2 => self.tick_mrd(tick, 0),
            4 => self.inc(0),
            _ => (),
        }
    }

    fn tick_interrupt(&mut self, tick: u8) {
        if tick == 0 {
            self.t = (self.x << 4) | (self.p & 0xf);
            self.x = 2;
            self.p = 1;
            self.ie = false;
        }
    }

    fn sample_dma_intr(&mut self, pins: Cdp1802Pins, ie: bool) -> Option<State> {
        // DMA and Interrupt lines are sampled between the leading edge of TPB and the leading edge
        // of TPA. We interpret this to mean that we can sample at the last minute before making a
        // state transition, e.g. at the end of an S1/Execute machine cycle.
        let dma_in = !pins.get_dma_in();
        let dma_out = !pins.get_dma_out();
        let intr = !pins.get_intr();
        let state = match (dma_in, dma_out, intr, ie) {
            (true, _, _, _) => State::DmaIn(0),
            (_, true, _, _) => State::DmaOut(0),
            (_, _, true, true) => State::Interrupt(0),
            _ => return None,
        };
        Some(state)
    }

    fn glo(&self, n: u8) -> u8 {
        (self.r[n as usize] & 0xff) as u8
    }

    fn ghi(&self, n: u8) -> u8 {
        (self.r[n as usize] >> 8) as u8
    }

    fn plo(&mut self, n: u8, d: u8) {
        let r = &mut self.r[n as usize];
        *r = (*r & 0xff00) | (d as u16);
    }

    fn phi(&mut self, n: u8, d: u8) {
        let r = &mut self.r[n as usize];
        *r = (*r & 0xff) | ((d as u16) << 8);
    }

    fn dec(&mut self, n: u8) {
        let r = &mut self.r[n as usize];
        *r = (*r).overflowing_sub(1).0;
    }

    fn inc(&mut self, n: u8) {
        let r = &mut self.r[n as usize];
        *r = (*r).overflowing_add(1).0;
    }

    fn tick_mrd(&mut self, tick: u8, n: u8) {
        match tick {
            0 => self.out.set_ma(self.ghi(n)),
            2 => {
                self.out.set_mrd(false);
                self.out.set_ma(self.glo(n));
            }
            _ => unreachable!(),
        }
    }

    fn tick_store(&mut self, tick: u8, n: u8, value: u8) {
        match tick {
            0 => self.out.set_ma(self.ghi(n)),
            2 => self.out.set_ma(self.glo(n)),
            3 => self.out.set_bus(value),
            5 => self.out.set_mwr(false),
            _ => (),
        }
    }

    fn tick_mark(&mut self, tick: u8) {
        match tick {
            0 => self.out.set_ma(self.ghi(2)),
            2 => self.out.set_ma(self.glo(2)),
            3 => {
                self.t = (self.x << 4) | (self.p & 0xf);
                self.out.set_bus(self.t);
                self.x = self.p;
                self.dec(2);
            }
            5 => self.out.set_mwr(false),
            _ => (),
        }
    }

    fn tick_ret(&mut self, pins: Cdp1802Pins, tick: u8, ie: bool) {
        match tick {
            t @ (0 | 2) => self.tick_mrd(t, self.x),
            3 => {
                let m = pins.get_bus();
                self.x = m >> 4;
                self.p = m & 0xf;
                self.inc(self.x);
                self.ie = ie;
            }
            _ => (),
        }
    }

    fn tick_add(&mut self, pins: Cdp1802Pins, tick: u8, n: u8, carry: bool) {
        match tick {
            t @ (0 | 2) => self.tick_mrd(t, n),
            3 => {
                let m = pins.get_bus();
                if carry {
                    (self.d, self.df) = math::addc(self.d, m, self.df);
                } else {
                    (self.d, self.df) = math::add(self.d, m);
                }
            }
            _ => (),
        }
    }

    fn tick_subd(&mut self, pins: Cdp1802Pins, tick: u8, n: u8, borrow: bool) {
        match tick {
            t @ (0 | 2) => self.tick_mrd(t, n),
            3 => {
                let m = pins.get_bus();
                if borrow {
                    (self.d, self.df) = math::subc(m, self.d, !self.df);
                } else {
                    (self.d, self.df) = math::sub(m, self.d);
                }
            }
            _ => (),
        }
    }

    fn tick_subm(&mut self, pins: Cdp1802Pins, tick: u8, n: u8, borrow: bool) {
        match tick {
            t @ (0 | 2) => self.tick_mrd(t, n),
            3 => {
                let m = pins.get_bus();
                if borrow {
                    (self.d, self.df) = math::subc(self.d, m, !self.df);
                } else {
                    (self.d, self.df) = math::sub(self.d, m);
                }
            }
            _ => (),
        }
    }

    fn tick_bxx(&mut self, pins: Cdp1802Pins, tick: u8, n: u8) {
        match tick {
            t @ (0 | 2) => self.tick_mrd(t, self.p),
            1 => self.ef = pins.get_ef(),
            3 => {
                if self.test(n) {
                    let m = pins.get_bus();
                    self.plo(self.p, m);
                } else {
                    self.inc(self.p);
                }
            }
            _ => (),
        }
    }

    fn tick_lbxx(&mut self, pins: Cdp1802Pins, tick: u8, n: u8) {
        match tick {
            t @ (0 | 2) => self.tick_mrd(t, self.p),
            1 => self.ef = pins.get_ef(),
            3 => {
                self.b = pins.get_bus();
                self.inc(self.p);
            }
            t @ (8 | 10) => self.tick_mrd(t & 7, self.p),
            11 => {
                if self.test(n) {
                    let m = pins.get_bus();
                    self.plo(self.p, m);
                    self.phi(self.p, self.b);
                } else {
                    self.inc(self.p);
                }
            }
            _ => (),
        }
    }

    fn test(&self, n: u8) -> bool {
        let br = match n & 7 {
            0x0 => true,
            0x1 => self.out.get_q(),
            0x2 => self.d == 0,
            0x3 => self.df,
            0x4 => (self.ef & 1) == 0,
            0x5 => (self.ef & 2) == 0,
            0x6 => (self.ef & 4) == 0,
            0x7 => (self.ef & 8) == 0,
            _ => unreachable!(),
        };
        if (n & 8) > 0 { !br } else { br }
    }

    fn tick_output(&mut self, tick: u8, n: u8) {
        match tick {
            0 => {
                self.out.set_n(n);
                self.out.set_ma(self.ghi(self.x));
            }
            2 => {
                self.out.set_mrd(false);
                self.out.set_ma(self.glo(self.x));
            }
            3 => self.inc(self.x),
            7 => self.out.set_n(0),
            _ => (),
        }
    }

    fn tick_input(&mut self, pins: Cdp1802Pins, tick: u8, n: u8) {
        match tick {
            0 => {
                self.out.set_n(n);
                self.out.set_ma(self.ghi(self.x));
            }
            2 => self.out.set_ma(self.glo(self.x)),
            5 => self.out.set_mwr(false),
            6 => self.d = pins.get_bus(),
            7 => self.out.set_n(0),
            _ => (),
        }
    }
}
