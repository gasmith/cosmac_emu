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
mod micro_ops;
mod pins;
#[cfg(test)]
mod tests;

pub use memory::{Memory, MemoryRange};
use micro_ops::{
    Access, AluOp, Bit, Cycle, DMA_IN_CYCLE, DMA_OUT_CYCLE, FETCH_CYCLE, INSTR_CYCLE_TABLE,
    MicroOp, Reg,
};
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
    pub br: bool,
    pub r: [u16; 16],
    pub p: u8,
    pub x: u8,
    pub instr: u8,
    pub t: u8,
    pub ie: bool,

    /// Buffered pin outputs
    out: Cdp1802Pins,

    /// EF1 is the least significant bit.
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
            i = self.instr,
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
            br: rng.random(),
            r: rng.random(),
            p: rng.random_range(0..16),
            x: rng.random_range(0..16),
            instr: rng.random(),
            t: rng.random(),
            ie: rng.random(),
            out: Cdp1802Pins(rng.random::<u64>() & Cdp1802Pins::mask_bus_out()),
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
            br: false,
            r: [0; 16],
            p: 0,
            x: 0,
            instr: 0,
            t: 0,
            ie: true,
            out: Cdp1802Pins::default(),
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
            (Mode::Run, State::Execute(7)) if self.instr == 0 => !dma_intr,
            (Mode::Run, _) => false,
        }
    }

    pub fn is_fetch_tick0(&self) -> bool {
        matches!(self.state, State::Fetch(0))
    }

    /// Returns the current opcode, if the chip is entering S1.
    pub fn get_exec_opcode(&self) -> Option<u8> {
        if matches!(self.state, State::Execute(0)) {
            Some(self.instr)
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
        let inp = matches!(self.state, State::Execute(_)) && (0x69..0x70).contains(&self.instr);
        let mask = if init || (mwr && !inp) {
            Cdp1802Pins::mask_bus_out()
        } else {
            Cdp1802Pins::mask_out()
        };
        pins.set_masked(self.out, mask);
    }

    fn tick_reset(&mut self) {
        self.instr = 0;
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
            State::DmaIn(t) => self.tick_cycle(t, pins, &DMA_IN_CYCLE),
            State::Execute(2) => {
                let addr = self.glo(self.p);
                if addr > 0 {
                    self.out.set_mrd(false);
                    self.out.set_ma(addr.wrapping_sub(1));
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
            State::Fetch(t) => self.tick_cycle(t, pins, &FETCH_CYCLE),
            State::Execute(t) => self.tick_cycle(t, pins, &INSTR_CYCLE_TABLE[self.instr as usize]),
            State::DmaIn(t) => self.tick_cycle(t, pins, &DMA_IN_CYCLE),
            State::DmaOut(t) => self.tick_cycle(t, pins, &DMA_OUT_CYCLE),
            State::Interrupt(t) => self.tick_interrupt(t),
        };
        self.state = match self.state {
            // Interrupts are forcibly disabled during this transition.
            State::Init(8) => self.sample_dma_intr(pins, false).unwrap_or(State::Fetch(0)),

            // IDL instruction stays in S1.
            State::Execute(7) if self.instr == 0 => self
                .sample_dma_intr(pins, self.ie)
                .unwrap_or(State::Execute(0)),

            // Non-long-branch instructions return to S0.
            State::Execute(7) if (self.instr >> 4) != 0xc => self
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
        if tick == 0 {
            self.x = 0;
            self.p = 0;
            self.r[0] = 0;
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

    fn tick_cycle(&mut self, tick: u8, pins: Cdp1802Pins, cycle: &Cycle) {
        // Helper to resolve register reference to register index.
        let reg = |reg| match reg {
            Reg::R(n) => n,
            Reg::X => self.x,
            Reg::P => self.p,
        };

        // Memory addressing and MWR/MRD/N signaling.
        let phase = tick & 7;
        match cycle.access {
            Access::None => (),
            Access::Read(r) => match phase {
                0 => self.out.set_ma(self.ghi(reg(r))),
                2 => {
                    self.out.set_ma(self.glo(reg(r)));
                    self.out.set_mrd(false);
                }
                _ => (),
            },
            Access::Write(r) => match phase {
                0 => self.out.set_ma(self.ghi(reg(r))),
                2 => self.out.set_ma(self.glo(reg(r))),
                5 => self.out.set_mwr(false),
                _ => (),
            },
            Access::Out(r, n) => match phase {
                0 => {
                    self.out.set_n(n);
                    self.out.set_ma(self.ghi(reg(r)));
                }
                2 => {
                    self.out.set_ma(self.glo(reg(r)));
                    self.out.set_mrd(false);
                }
                7 => self.out.set_n(0),
                _ => (),
            },
            Access::Inp(r, n) => match phase {
                0 => {
                    self.out.set_n(n);
                    self.out.set_ma(self.ghi(reg(r)));
                }
                2 => self.out.set_ma(self.glo(reg(r))),
                5 => self.out.set_mwr(false),
                7 => self.out.set_n(0),
                _ => (),
            },
        }

        // Micro-ops
        match cycle.ticks[tick as usize] {
            MicroOp::Idle => (),
            MicroOp::Fetch => {
                self.instr = pins.get_bus();
                // Don't increment the program counter for the IDL instruction.
                if self.instr != 0 {
                    self.inc(self.p);
                }
            }
            MicroOp::Inc(r) => self.inc(reg(r)),
            MicroOp::Dec(r) => self.dec(reg(r)),
            MicroOp::SetBusD => self.out.set_bus(self.d),
            MicroOp::SetBusT => self.out.set_bus(self.t),
            MicroOp::GetBus => self.d = pins.get_bus(),
            MicroOp::GetLo(r) => self.d = self.glo(reg(r)),
            MicroOp::GetHi(r) => self.d = self.ghi(reg(r)),
            MicroOp::PutLo(r) => self.plo(reg(r), self.d),
            MicroOp::PutHi(r) => self.phi(reg(r), self.d),
            MicroOp::SetQ(q) => self.out.set_q(q),
            MicroOp::SetP(n) => self.p = n,
            MicroOp::SetX(n) => self.x = n,
            MicroOp::Alu(alu) => match alu {
                AluOp::Or => self.d |= pins.get_bus(),
                AluOp::And => self.d &= pins.get_bus(),
                AluOp::Xor => self.d ^= pins.get_bus(),
                AluOp::Shl => (self.d, self.df) = (self.d << 1, self.d & 0x80 != 0),
                AluOp::Shr => (self.d, self.df) = (self.d >> 1, self.d & 0x1 != 0),
                AluOp::Shlc => {
                    let df = self.d & 0x80 != 0;
                    self.d <<= 1;
                    if self.df {
                        self.d |= 0x01;
                    }
                    self.df = df;
                }
                AluOp::Shrc => {
                    let df = self.d & 0x01 != 0;
                    self.d >>= 1;
                    if self.df {
                        self.d |= 0x80;
                    }
                    self.df = df;
                }
                AluOp::Add => {
                    let m = pins.get_bus();
                    (self.d, self.df) = math::add(self.d, m);
                }
                AluOp::Adc => {
                    let m = pins.get_bus();
                    (self.d, self.df) = math::addc(self.d, m, self.df);
                }
                AluOp::Sd => {
                    let m = pins.get_bus();
                    let (d, borrow) = math::sub(m, self.d);
                    self.d = d;
                    self.df = !borrow;
                }
                AluOp::Sdb => {
                    let m = pins.get_bus();
                    let (d, borrow) = math::subb(m, self.d, !self.df);
                    self.d = d;
                    self.df = !borrow;
                }
                AluOp::Sm => {
                    let m = pins.get_bus();
                    let (d, borrow) = math::sub(self.d, m);
                    self.d = d;
                    self.df = !borrow;
                }
                AluOp::Smb => {
                    let m = pins.get_bus();
                    let (d, borrow) = math::subb(self.d, m, !self.df);
                    self.d = d;
                    self.df = !borrow;
                }
            },
            MicroOp::Test { bit, inv } => {
                self.br = match bit {
                    Bit::True => true,
                    Bit::Q => self.out.get_q(),
                    Bit::DZ => self.d == 0,
                    Bit::DF => self.df,
                    Bit::IE => self.ie,
                    Bit::NEF1 => !pins.get_ef1(),
                    Bit::NEF2 => !pins.get_ef2(),
                    Bit::NEF3 => !pins.get_ef3(),
                    Bit::NEF4 => !pins.get_ef4(),
                };
                if inv {
                    self.br = !self.br;
                }
            }
            MicroOp::Br => {
                if self.br {
                    let m = pins.get_bus();
                    self.plo(self.p, m);
                } else {
                    self.inc(self.p);
                }
            }
            MicroOp::LbrLatch => {
                // Store the high byte in a temporary register, so that we can use
                // r[p] to fetch the low byte in the subsequent machine cycle.
                self.b = pins.get_bus();
                self.inc(self.p);
            }
            MicroOp::Lbr => {
                if self.br {
                    let m = pins.get_bus();
                    self.plo(self.p, m);
                    self.phi(self.p, self.b);
                } else {
                    self.inc(self.p);
                }
            }
            MicroOp::Ls => {
                if self.br {
                    self.inc(self.p);
                }
            }
            MicroOp::Mark => {
                self.t = (self.x << 4) | (self.p & 0xf);
                self.out.set_bus(self.t);
                self.x = self.p;
                self.dec(2);
            }
            MicroOp::Ret { ie } => {
                let m = pins.get_bus();
                self.x = m >> 4;
                self.p = m & 0xf;
                self.inc(self.x);
                self.ie = ie;
            }
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
}
