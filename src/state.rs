use crate::{Instr, InstrSchema, Memory};
use rand::{thread_rng, Rng};
use std::collections::HashSet;

pub enum Status {
    Ready,
    Idle,
    Breakpoint,
}

#[derive(Default)]
pub struct State {
    cycle: u64,
    bus: u8,
    d: u8,
    df: bool,
    ie: bool,
    pub m: Memory,
    p: u8,
    q: bool,
    r: [u16; 16],
    t: u8,
    x: u8,
    ef: [bool; 4],
    breakpoints: HashSet<u16>,
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
    x.overflowing_sub(y)
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
        state
    }

    pub fn reset(&mut self) {
        self.cycle = 0;
        // Registers I, N, Q are reset, IE is set and 0’s (VSS) are placed on
        // the data bus.
        self.bus = 0;
        self.ie = true;
        self.q = false;
        // The first machine cycle after termination of reset is an
        // initialization cycle which requires 9 clock pulses. During this cycle
        // the CPU remains in S1 and register X, P, and R(0) are reset.
        self.x = 0;
        self.p = 0;
        self.r[0] = 0;
    }

    fn r(&self, r: u8) -> u16 {
        self.r[usize::from(r)]
    }

    fn r_mut(&mut self, r: u8) -> &mut u16 {
        &mut self.r[usize::from(r)]
    }

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

    pub fn set_breakpoint(&mut self, addr: u16) {
        self.breakpoints.insert(addr);
    }

    pub fn clear_breakpoint(&mut self, addr: u16) {
        self.breakpoints.remove(&addr);
    }

    pub fn breakpoints(&self) -> &HashSet<u16> {
        &self.breakpoints
    }

    pub fn toggle_flag(&mut self, n: usize) {
        if n < self.ef.len() {
            self.ef[n] = !self.ef[n];
        }
    }

    pub fn print_flags(&self) {
        println!(
            "df={df} ef={ef0}{ef1}{ef2}{ef3} q={q}",
            df = u8::from(self.df),
            ef0 = u8::from(self.ef[0]),
            ef1 = u8::from(self.ef[1]),
            ef2 = u8::from(self.ef[2]),
            ef3 = u8::from(self.ef[3]),
            q = u8::from(self.q),
        );
    }

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

    pub fn step(&mut self) -> Status {
        let addr = self.rp();
        match self.decode_instr(addr) {
            Some(instr) => self.do_step(instr),
            None => Status::Idle,
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn do_step(&mut self, instr: Instr) -> Status {
        let size = instr.size();
        self.inc_by(self.p, size);
        self.cycle += if size == 3 { 3 } else { 2 };
        match instr {
            Instr::Idl => {
                self.dec(self.p);
                return Status::Idle;
            }
            Instr::Ldn(n) => self.d = self.load(n),
            Instr::Inc(n) => self.inc(n),
            Instr::Dec(n) => self.dec(n),
            // Branch
            Instr::Br(nn)
            | Instr::Bq(nn)
            | Instr::Bz(nn)
            | Instr::Bdf(nn)
            | Instr::B1(nn)
            | Instr::B2(nn)
            | Instr::B3(nn)
            | Instr::B4(nn)
            | Instr::Nbr(nn)
            | Instr::Bnq(nn)
            | Instr::Bnz(nn)
            | Instr::Bnf(nn)
            | Instr::Bn1(nn)
            | Instr::Bn2(nn)
            | Instr::Bn3(nn)
            | Instr::Bn4(nn) => self.handle_bxx(instr.opcode(), nn),
            Instr::Lda(n) => {
                // M(R(N)) → D; R(N) + 1 → R(N)
                self.d = self.load(n);
                self.inc(n);
            }
            Instr::Str(n) => self.store(n, self.d),
            Instr::Irx => self.inc(self.x),
            Instr::Out(n) => {
                let v = self.load(self.x);
                self.inc(self.x);
                println!("OUT{n}: {v:02x}");
            }
            Instr::Resv68 => (),
            Instr::Inp(_) => self.store(self.x, 0),
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
            Instr::Req => self.q = false,
            Instr::Seq => self.q = true,
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
            // Long branch
            Instr::Lbr(hh, ll)
            | Instr::Lbq(hh, ll)
            | Instr::Lbz(hh, ll)
            | Instr::Lbdf(hh, ll)
            | Instr::Nlbr(hh, ll)
            | Instr::Lbnq(hh, ll)
            | Instr::Lbnz(hh, ll)
            | Instr::Lbnf(hh, ll) => self.handle_lbxx(instr.opcode(), hh, ll),
            // Long skip
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
        }
        if self.breakpoints.contains(&self.rp()) {
            Status::Breakpoint
        } else {
            Status::Ready
        }
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
            0x4 => self.ef[0],
            0x5 => self.ef[1],
            0x6 => self.ef[2],
            0x7 => self.ef[3],
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
