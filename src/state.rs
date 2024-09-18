use rand::{thread_rng, Rng};

use crate::event::{Flag, InputEvent, OutputEvent, Port};
use crate::instr::{Instr, InstrSchema};
use crate::memory::Memory;

#[derive(Default)]
pub struct State {
    /// Memory image.
    pub m: Memory,
    /// Number of machine cycles since last reset.
    cycle: u64,
    /// Accumulator register.
    d: u8,
    /// ALU carry flag.
    df: bool,
    /// External flags.
    ef: [bool; 4],
    /// Interrupt enable flag.
    ie: bool,
    /// Input data bus.
    inp: [u8; 7],
    /// Interrupt line.
    int: bool,
    /// Output data bus.
    out: [u8; 7],
    /// Program register select (4-bit).
    p: u8,
    /// Output flip-flop.
    q: bool,
    /// General purpose registers.
    r: [u16; 16],
    /// Interrupt context (packed X and P).
    t: u8,
    /// Data register select (4-bit).
    x: u8,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("cycle", &self.cycle)
            .field("d", &self.d)
            .field("df", &self.df)
            .field("ef", &self.ef)
            .field("ie", &self.ie)
            .field("inp", &self.inp)
            .field("int", &self.int)
            .field("out", &self.out)
            .field("p", &self.p)
            .field("q", &self.q)
            .field("r", &self.r)
            .field("t", &self.t)
            .field("x", &self.x)
            .finish()
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let rp = self.rp();
        let listing = self.decode_instr(rp).map_or("??".into(), |i| i.listing());
        write!(
            f,
            "{c:08x} d={d:02x}.{df} x={x:02x}:{rx:04x}:{mrx:02x} p={p:02x}:{rp:04x}:{listing}",
            c = self.cycle,
            d = self.d,
            df = u8::from(self.df),
            x = self.x,
            rx = self.r(self.x),
            mrx = self.load(self.x),
            p = self.p,
        )
    }
}

fn add(x: u8, y: u8) -> (u8, bool) {
    x.overflowing_add(y)
}

fn addc(x: u8, y: u8, df: bool) -> (u8, bool) {
    let (acc, df1) = add(x, y);
    let (acc, df2) = add(acc, df.into());
    (acc, df1 || df2)
}

fn sub(x: u8, y: u8) -> (u8, bool) {
    let (z, overflow) = x.overflowing_sub(y);
    (z, !overflow)
}

fn subc(x: u8, y: u8, df: bool) -> (u8, bool) {
    let (acc, df1) = sub(x, y);
    let (acc, df2) = sub(acc, df.into());
    (acc, df1 || df2)
}

impl State {
    pub fn new(m: Memory) -> Self {
        let mut rng = thread_rng();
        let mut state = Self {
            m,
            df: rng.gen(),
            t: rng.gen(),
            ..Default::default()
        };
        rng.fill(&mut state.r);
        rng.fill(&mut state.ef);
        state.reset();
        state
    }

    pub fn reset(&mut self) {
        // From the CDP1802 datasheet:
        //
        //  "Registers I, N, Q are reset, IE is set and 0’s (VSS) are placed on the data bus. The
        //   first machine cycle after termination of reset is an initialization cycle which requires
        //   9 clock pulses. During this cycle the CPU remains in S1 and register X, P, and R(0) are
        //   reset."
        //
        // Note that registers I & N are latches for the instruction opcode's high and low bits,
        // respectively.
        self.ie = true;
        self.q = false;
        self.x = 0;
        self.p = 0;
        self.r[0] = 0;

        // Reset external flags and I/O ports.
        self.int = false;
        for i in 0..4 {
            self.ef[i] = true;
        }
        for i in 0..7 {
            self.out[i] = 0;
            self.inp[i] = 0;
        }

        // Reset cycle count.
        self.cycle = 0;
    }

    /// Returns the number of machine cycles since the last reset.
    pub fn cycle(&self) -> u64 {
        self.cycle
    }

    fn r(&self, r: u8) -> u16 {
        self.r[usize::from(r)]
    }

    fn r_mut(&mut self, r: u8) -> &mut u16 {
        &mut self.r[usize::from(r)]
    }

    /// Returns the current instruction pointer.
    pub fn rp(&self) -> u16 {
        self.r(self.p)
    }

    fn store(&mut self, r: u8, v: u8) {
        self.m.store(self.r(r), v);
    }

    fn load(&self, r: u8) -> u8 {
        self.m.load(self.r(r))
    }

    fn dec(&mut self, r: u8) {
        let r = self.r_mut(r);
        *r = (*r).overflowing_sub(1).0;
    }

    fn inc(&mut self, r: u8) {
        let r = self.r_mut(r);
        *r = (*r).overflowing_add(1).0;
    }

    fn inc_by(&mut self, r: u8, n: u8) {
        let r = self.r_mut(r);
        *r = (*r).overflowing_add(n.into()).0;
    }

    fn phi(&mut self, r: u8, v: u8) {
        let r = self.r_mut(r);
        *r = (u16::from(v) << 8) | (*r & 0x00ff);
    }

    fn plo(&mut self, r: u8, v: u8) {
        let r = self.r_mut(r);
        *r = (*r & 0xff00) | u16::from(v);
    }

    fn ghi(&mut self, r: u8) -> u8 {
        u8::try_from(self.r(r) >> 8).unwrap()
    }

    fn glo(&mut self, r: u8) -> u8 {
        u8::try_from(self.r(r) & 0xff).unwrap()
    }

    /// Toggles the value of an external flag.
    pub fn toggle_flag(&mut self, flag: u8) {
        if flag > 0 && flag <= 4 {
            let index = usize::from(flag - 1);
            self.ef[index] = !self.ef[index];
        }
    }

    /// Prints flags to stdout.
    pub fn print_flags(&self) {
        println!(
            "df={df} /ef={ef0}{ef1}{ef2}{ef3} ie={ie} int={int} q={q}",
            df = u8::from(self.df),
            ef0 = u8::from(self.ef[0]),
            ef1 = u8::from(self.ef[1]),
            ef2 = u8::from(self.ef[2]),
            ef3 = u8::from(self.ef[3]),
            ie = u8::from(self.ie),
            int = u8::from(self.int),
            q = u8::from(self.q),
        );
    }

    /// Prints registers to stdout.
    pub fn print_registers(&self) {
        println!(
            "d={d:02x}.{df} p={p:x} x={x:x} t={t:04x}",
            d = self.d,
            df = u8::from(self.df),
            p = self.p,
            x = self.x,
            t = self.t,
        );
        for (n, r) in self.r.iter().enumerate() {
            print!("{n:x}={r:04x}");
            if n % 4 == 3 {
                println!();
            } else {
                print!(" ");
            }
        }
    }

    pub fn decode_instr(&self, addr: u16) -> Option<Instr> {
        Instr::decode(self.m.as_slice(addr, 3))
    }

    fn decode_rp(&self) -> Option<Instr> {
        self.decode_instr(self.rp())
    }

    /// Advances the state machine.
    #[must_use]
    pub fn step(&mut self) -> Option<OutputEvent> {
        if self.ie && self.int {
            self.handle_interrupt();
            None
        } else if let Some(instr) = self.decode_rp() {
            self.handle_instr(instr)
        } else {
            None
        }
    }

    /// Returns true if the current instruction is IDL.
    pub fn is_idle(&self) -> bool {
        matches!(self.decode_rp(), None | Some(Instr::Idl))
    }

    /// Handles an interrupt.
    fn handle_interrupt(&mut self) {
        self.t = (self.x << 4) | self.p;
        self.p = 1;
        self.x = 2;
        self.ie = false;
        self.cycle += 1;
    }

    /// Runs the next instruction.
    #[allow(clippy::too_many_lines)]
    fn handle_instr(&mut self, instr: Instr) -> Option<OutputEvent> {
        let size = instr.size();
        self.inc_by(self.p, size);
        self.cycle += if size == 3 { 3 } else { 2 };
        match instr {
            Instr::Idl => self.dec(self.p),
            Instr::Ldn(n) => self.d = self.load(n),
            Instr::Inc(n) => self.inc(n),
            Instr::Dec(n) => self.dec(n),
            Instr::Br(nn)
            | Instr::Bq(nn)
            | Instr::Bz(nn)
            | Instr::Bdf(nn)
            | Instr::B1(nn)
            | Instr::B2(nn)
            | Instr::B3(nn)
            | Instr::B4(nn)
            | Instr::Bnq(nn)
            | Instr::Bnz(nn)
            | Instr::Bnf(nn)
            | Instr::Bn1(nn)
            | Instr::Bn2(nn)
            | Instr::Bn3(nn)
            | Instr::Bn4(nn) => self.handle_bxx(instr.opcode(), nn),
            Instr::Skp => {
                // Also schematized as `NBR nn`, where `nn` is ignored.
                self.handle_bxx(instr.opcode(), 0);
                self.inc(self.p);
            }
            Instr::Lda(n) => {
                // M(R(N)) → D; R(N) + 1 → R(N)
                self.d = self.load(n);
                self.inc(n);
            }
            Instr::Str(n) => self.store(n, self.d),
            Instr::Irx => self.inc(self.x),
            Instr::Out(n) => {
                assert!(n > 0 && n <= 7);
                let value = self.load(self.x);
                self.out[usize::from(n - 1)] = value;
                self.inc(self.x);
                return Some(OutputEvent::Output {
                    port: match n {
                        0 => Port::IO1,
                        1 => Port::IO2,
                        2 => Port::IO3,
                        3 => Port::IO4,
                        4 => Port::IO5,
                        5 => Port::IO6,
                        6 => Port::IO7,
                        _ => unreachable!(),
                    },
                    value,
                });
            }
            Instr::Resv68 => (),
            Instr::Inp(n) => {
                assert!(n > 0 && n <= 7);
                self.store(self.x, self.inp[usize::from(n - 1)])
            }
            Instr::Ret => {
                // M(R(X)) → (X, P); R(X) + 1 → R(X), 1 → IE
                let v = self.load(self.x);
                self.x = v >> 4;
                self.p = v & 0x7;
                self.inc(self.x);
                self.ie = true;
            }
            Instr::Dis => {
                // M(R(X)) → (X, P); R(X) + 1 → R(X), 0 → IE
                let v = self.load(self.x);
                self.x = v >> 4;
                self.p = v & 0x7;
                self.inc(self.x);
                self.ie = false;
            }
            Instr::Ldxa => {
                // M(R(X)) → D; R(X) + 1 → R(X)
                self.d = self.load(self.x);
                self.inc(self.x);
            }
            Instr::Stxd => {
                // D → M(R(X)); R(X) - 1 → R(X)
                self.store(self.x, self.d);
                self.dec(self.x);
            }
            Instr::Adc => {
                // M(R(X)) + D + DF → DF, D
                (self.d, self.df) = addc(self.load(self.x), self.d, self.df);
            }
            Instr::Sdb => {
                // M(R(X)) - D - (Not DF) → DF, D
                (self.d, self.df) = subc(self.load(self.x), self.d, !self.df);
            }
            Instr::Shrc => {
                // SHIFT D RIGHT, LSB(D) → DF, DF → MSB(D)
                let df = self.d & 0x01 != 0;
                self.d >>= 1;
                if self.df {
                    self.d |= 0x80;
                }
                self.df = df;
            }
            Instr::Smb => {
                // D-M(R(X))-(NOT DF) → DF, D
                (self.d, self.df) = subc(self.d, self.load(self.x), !self.df);
            }
            Instr::Sav => {
                // T → M(R(X))
                self.store(self.x, self.t);
            }
            Instr::Mark => {
                // (X, P) → T; (X, P) → M(R(2)), THEN P → X; R(2) - 1 → R(2)
                self.t = (self.x << 4) | self.p;
                self.store(2, self.t);
                self.x = self.p;
                self.dec(2);
            }
            Instr::Req => {
                self.q = false;
                return Some(OutputEvent::Q { value: false });
            }
            Instr::Seq => {
                self.q = true;
                return Some(OutputEvent::Q { value: true });
            }
            Instr::Adci(nn) => {
                // M(R(P)) + D + DF → DF, D; R(P) + 1 → R(P)
                (self.d, self.df) = addc(nn, self.d, self.df);
            }
            Instr::Sdbi(nn) => {
                // M(R(P)) - D - (Not DF) → DF, D; R(P) + 1 → R(P)
                (self.d, self.df) = subc(nn, self.d, !self.df);
            }
            Instr::Shlc => {
                // SHIFT D LEFT, MSB(D) → DF, DF → LSB(D)
                let df = self.d & 0x80 != 0;
                self.d <<= 1;
                if self.df {
                    self.d |= 0x01;
                }
                self.df = df;
            }
            Instr::Smbi(nn) => {
                // SMBI: D-M(R(P))-(NOT DF) → DF, D; R(P) + 1 → R(P)
                (self.d, self.df) = subc(self.d, nn, !self.df);
                self.inc(self.p);
            }
            Instr::Glo(n) => self.d = self.glo(n),
            Instr::Ghi(n) => self.d = self.ghi(n),
            Instr::Plo(n) => self.plo(n, self.d),
            Instr::Phi(n) => self.phi(n, self.d),
            Instr::Lbr(hh, ll)
            | Instr::Lbq(hh, ll)
            | Instr::Lbz(hh, ll)
            | Instr::Lbdf(hh, ll)
            | Instr::Nlbr(hh, ll)
            | Instr::Lbnq(hh, ll)
            | Instr::Lbnz(hh, ll)
            | Instr::Lbnf(hh, ll) => self.handle_lbxx(instr.opcode(), hh, ll),
            Instr::Nop
            | Instr::Lsnq
            | Instr::Lsnz
            | Instr::Lsnf
            | Instr::Lsie
            | Instr::Lsq
            | Instr::Lsz
            | Instr::Lsdf => self.handle_lsxx(instr.opcode()),
            Instr::Sep(n) => self.p = n,
            Instr::Sex(n) => self.x = n,
            Instr::Ldx => self.d = self.load(self.x),
            Instr::Or => self.d |= self.load(self.x),
            Instr::And => self.d &= self.load(self.x),
            Instr::Xor => self.d ^= self.load(self.x),
            Instr::Add => (self.d, self.df) = add(self.d, self.load(self.x)),
            Instr::Sd => (self.d, self.df) = sub(self.load(self.x), self.d),
            Instr::Shr => (self.d, self.df) = (self.d >> 1, self.d & 0x01 != 0),
            Instr::Sm => (self.d, self.df) = sub(self.d, self.load(self.x)),
            Instr::Ldi(n) => self.d = n,
            Instr::Ori(n) => self.d |= n,
            Instr::Ani(n) => self.d &= n,
            Instr::Xri(n) => self.d ^= n,
            Instr::Adi(n) => (self.d, self.df) = add(self.d, n),
            Instr::Sdi(n) => (self.d, self.df) = sub(n, self.d),
            Instr::Shl => (self.d, self.df) = (self.d << 1, self.d & 0x80 != 0),
            Instr::Smi(n) => (self.d, self.df) = sub(self.d, n),
        };
        None
    }

    /// Handles the `Bxx` family of instructions.
    fn handle_bxx(&mut self, opcode: u8, nn: u8) {
        let op_lo = opcode & 0xf;
        let inv = (op_lo & 0x8) > 0;
        let mut br = match op_lo & 0x7 {
            0x0 => true,
            0x1 => self.q,
            0x2 => self.d == 0,
            0x3 => self.df,
            0x4 => !self.ef[0],
            0x5 => !self.ef[1],
            0x6 => !self.ef[2],
            0x7 => !self.ef[3],
            _ => unreachable!(),
        };
        if inv {
            br = !br;
        }
        if br {
            self.plo(self.p, nn);
        }
    }

    /// Handles the `LBxx` family of instructions.
    fn handle_lbxx(&mut self, opcode: u8, hh: u8, ll: u8) {
        let op_lo = opcode & 0xf;
        let inv = (op_lo & 0x8) > 0;
        let mut br = match op_lo & 0x3 {
            0x0 => true,
            0x1 => self.q,
            0x2 => self.d == 0,
            0x3 => self.df,
            _ => unreachable!(),
        };
        if inv {
            br = !br;
        }
        if br {
            self.phi(self.p, hh);
            self.plo(self.p, ll);
        }
    }

    /// Handles the `LSxx` family of instructions, including `NOP`.
    fn handle_lsxx(&mut self, opcode: u8) {
        let op_lo = opcode & 0xf;
        let inv = (op_lo & 0x8) > 0;
        let mut skp = match op_lo & 0x3 {
            0x0 => {
                if inv {
                    !self.ie
                } else {
                    false
                }
            }
            0x1 => !self.q,
            0x2 => self.d == 1,
            0x3 => !self.df,
            _ => unreachable!(),
        };
        if inv {
            skp = !skp;
        }
        if skp {
            self.inc_by(self.p, 2);
        }
    }

    /// Applies an external event.
    pub fn apply_event(&mut self, event: InputEvent) {
        match event {
            InputEvent::Interrupt => self.int = true,
            InputEvent::Flag { flag, value } => {
                let index = match flag {
                    Flag::EF1 => 0,
                    Flag::EF2 => 1,
                    Flag::EF3 => 2,
                    Flag::EF4 => 3,
                };
                self.ef[index] = value;
            }
            InputEvent::Input { port, value } => {
                let index = match port {
                    Port::IO1 => 0,
                    Port::IO2 => 1,
                    Port::IO3 => 2,
                    Port::IO4 => 3,
                    Port::IO5 => 4,
                    Port::IO6 => 6,
                    Port::IO7 => 7,
                };
                self.inp[index] = value;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(0, 0), (0, false));
        assert_eq!(add(0, 1), (1, false));
        assert_eq!(add(0xfe, 1), (0xff, false));
        assert_eq!(add(0xff, 1), (0, true));
        assert_eq!(add(0xff, 0xff), (0xfe, true));
    }

    #[test]
    fn test_addc() {
        assert_eq!(addc(0, 0, false), (0, false));
        assert_eq!(addc(0, 0, true), (1, false));
        assert_eq!(addc(0, 1, true), (2, false));
        assert_eq!(addc(1, 1, true), (3, false));
        assert_eq!(addc(0xff, 0, true), (0, true));
        assert_eq!(addc(0xff, 1, false), (0, true));
        assert_eq!(addc(0xfe, 1, true), (0, true));
        assert_eq!(addc(0xff, 1, true), (1, true));
        assert_eq!(addc(0xff, 2, true), (2, true));
    }

    #[test]
    fn test_sub() {
        assert_eq!(sub(0, 0), (0, false));
        assert_eq!(sub(0, 1), (0xff, true));
        assert_eq!(sub(1, 0), (1, false));
        assert_eq!(sub(0xff, 1), (0xfe, false));
        assert_eq!(sub(1, 0xff), (2, true));
    }

    #[test]
    fn test_subc() {
        assert_eq!(subc(0, 0, false), (0, false));
        assert_eq!(subc(1, 1, false), (0, false));
        assert_eq!(subc(1, 0, true), (0, false));
        assert_eq!(subc(1, 1, true), (0xff, true));
        assert_eq!(subc(0, 0, true), (0xff, true));
        assert_eq!(subc(0, 1, false), (0xff, true));
        assert_eq!(subc(0, 1, true), (0xfe, true));
    }
}
