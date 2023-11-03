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
        let listing = self
            .decode_instr(rp)
            .map(|i| i.listing())
            .unwrap_or("??".to_string());
        write!(
            f,
            "{c:08x} d={d:02x}.{df} x={x:02x}:{rx:04x}:{mrx:02x} p={p:02x}:{rp:04x}:{listing}",
            c = self.cycle,
            d = self.d,
            df = self.df as u8,
            x = self.x,
            rx = self.r(self.x),
            mrx = self.load(self.x),
            p = self.p,
        )
    }
}

fn add(x: u8, y: u8) -> (bool, u8) {
    let acc = x as u16 + y as u16;
    (acc > 0xff, (acc & 0xff) as u8)
}

fn addc(x: u8, y: u8, df: bool) -> (bool, u8) {
    let (df1, acc) = add(x, y);
    let (df2, acc) = add(acc, df as u8);
    (df1 || df2, acc)
}

fn sub(x: u8, y: u8) -> (bool, u8) {
    let acc = x as i16 - y as i16;
    (acc >= 0, acc as u8)
}

fn subc(x: u8, y: u8, df: bool) -> (bool, u8) {
    let (df1, acc) = sub(x, y);
    let (df2, acc) = sub(acc, df as u8);
    (df1 || df2, acc)
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
        // Registers l, N, Q are reset, IE is set and 0’s (VSS) are placed on
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
        self.r[r as usize]
    }

    fn r_mut(&mut self, r: u8) -> &mut u16 {
        &mut self.r[r as usize]
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
        *r = (*r as i32 - 1) as u16;
    }

    fn inc(&mut self, r: u8) {
        let r = self.r_mut(r);
        *r = (*r as u32 + 1) as u16;
    }

    fn inc_by(&mut self, r: u8, n: u32) {
        let r = self.r_mut(r);
        *r = (*r as u32 + n) as u16;
    }

    fn phi(&mut self, r: u8, v: u8) {
        let r = &mut self.r[r as usize];
        *r = ((v as u16) << 8) | (*r & 0x00ff);
    }

    fn plo(&mut self, r: u8, v: u8) {
        let r = &mut self.r[r as usize];
        *r = (*r & 0xff00) | v as u16;
    }

    fn ghi(&mut self, r: u8) -> u8 {
        (self.r[r as usize] >> 8) as u8
    }

    fn glo(&mut self, r: u8) -> u8 {
        (self.r[r as usize] & 0xff) as u8
    }

    pub fn set_breakpoint(&mut self, addr: u16) {
        self.breakpoints.insert(addr);
    }

    pub fn clear_breakpoint(&mut self, addr: &u16) {
        self.breakpoints.remove(addr);
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
            df = self.df as u8,
            ef0 = self.ef[0] as u8,
            ef1 = self.ef[1] as u8,
            ef2 = self.ef[2] as u8,
            ef3 = self.ef[3] as u8,
            q = self.q as u8,
        );
    }

    pub fn print_registers(&self) {
        println!(
            "d={d:02x}.{df} p={p:x} x={x:x} t={t:04x}",
            d = self.d,
            df = self.df as u8,
            p = self.p,
            x = self.x,
            t = self.t,
        );
        for (n, r) in self.r.iter().enumerate() {
            print!("{n:x}={r:04x}");
            if n % 4 == 3 {
                print!("\n");
            } else {
                print!(" ");
            }
        }
    }

    pub fn decode_instr(&self, addr: u16) -> Option<Instr> {
        Instr::decode(&self.m.as_slice(addr, 3))
    }

    pub fn decode_instr_str(&self, addr: u16) -> String {
        match self.decode_instr(addr) {
            Some(instr) => instr.disasm(),
            None => "??".to_string(),
        }
    }

    pub fn step(&mut self) -> Status {
        let addr = self.rp();
        match self.decode_instr(addr) {
            Some(instr) => self.do_step(instr),
            None => Status::Idle,
        }
    }

    pub fn do_step(&mut self, instr: Instr) -> Status {
        let size = instr.size();
        self.inc_by(self.p, size as u32);
        self.cycle += if size == 3 { 3 } else { 2 };
        match instr {
            Instr::Idl => {
                self.dec(self.p);
                return Status::Idle;
            }
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
            | Instr::Nbr(nn)
            | Instr::Bnq(nn)
            | Instr::Bnz(nn)
            | Instr::Bnf(nn)
            | Instr::Bn1(nn)
            | Instr::Bn2(nn)
            | Instr::Bn3(nn)
            | Instr::Bn4(nn) => {
                let op_lo = instr.opcode() & 0xf;
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
            Instr::Lda(n) => {
                // M(R(N)) → D; R(N) + 1 → R(N)
                self.d = self.load(n);
                self.inc(n);
            }
            Instr::Str(n) => self.store(n, self.d),
            Instr::Irx => self.inc(self.x),
            Instr::Out(n) => {
                let v = self.load(self.x);
                println!("OUT{}: {:02x}", n, v);
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
                (self.df, self.d) = addc(self.load(self.x), self.d, self.df);
            }
            Instr::Sdb => {
                // M(R(X)) - D - (Not DF) → DF, D
                (self.df, self.d) = subc(self.load(self.x), self.d, !self.df);
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
                (self.df, self.d) = subc(self.d, self.load(self.x), !self.df);
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
                (self.df, self.d) = addc(nn, self.d, self.df)
            }
            Instr::Sdbi(nn) => {
                // M(R(P)) - D - (Not DF) → DF, D; R(P) + 1 → R(P)
                (self.df, self.d) = subc(nn, self.d, !self.df)
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
                (self.df, self.d) = subc(self.d, nn, !self.df);
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
            | Instr::Lbnf(hh, ll) => {
                let op_lo = instr.opcode() & 0xf;
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
            Instr::Nop
            | Instr::Lsnq
            | Instr::Lsnz
            | Instr::Lsnf
            | Instr::Lsie
            | Instr::Lsq
            | Instr::Lsz
            | Instr::Lsdf => {
                let op_lo = instr.opcode() & 0xf;
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
            Instr::Sep(n) => self.p = n,
            Instr::Sex(n) => self.x = n,
            Instr::Ldx => self.d = self.load(self.x),
            Instr::Or => self.d |= self.load(self.x),
            Instr::And => self.d &= self.load(self.x),
            Instr::Xor => self.d ^= self.load(self.x),
            Instr::Add => (self.df, self.d) = add(self.d, self.load(self.x)),
            Instr::Sd => (self.df, self.d) = sub(self.d, self.load(self.x)),
            Instr::Shr => (self.df, self.d) = (self.d & 0x01 != 0, self.d >> 1),
            Instr::Sm => (self.df, self.d) = sub(self.load(self.x), self.d),
            Instr::Ldi(n) => self.d = n,
            Instr::Ori(n) => self.d |= n,
            Instr::Ani(n) => self.d &= n,
            Instr::Xri(n) => self.d ^= n,
            Instr::Adi(n) => (self.df, self.d) = add(self.d, n),
            Instr::Sdi(n) => (self.df, self.d) = sub(self.d, n),
            Instr::Shl => (self.df, self.d) = (self.d & 0x80 != 0, self.d << 1),
            Instr::Smi(n) => (self.df, self.d) = sub(n, self.d),
        }
        if self.breakpoints.contains(&self.rp()) {
            Status::Breakpoint
        } else {
            Status::Ready
        }
    }
}
