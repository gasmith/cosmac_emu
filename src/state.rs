use crate::Memory;
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
    m: Memory,
    p: u8,
    q: bool,
    r: [u16; 16],
    t: u8,
    x: u8,
    ef: [bool; 4],
    breakpoints: HashSet<u16>,
}

const MISC_INSTR_IMM0: [u8; 10] = [0x7c, 0x7d, 0x7f, 0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xff];

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let rp = self.r(self.p);
        let mrp = self.m.load(rp);
        let (imm0, imm1) = (self.m.load(rp + 1), self.m.load(rp + 2));
        let instr = if mrp & 0xf4 == 0xc0 {
            format!("{mrp:02x} {imm0:02x} {imm1:02x}")
        } else if mrp & 0xf0 == 0x30 || MISC_INSTR_IMM0.contains(&mrp) {
            format!("{mrp:02x} {imm0:02x}")
        } else {
            format!("{mrp:02x}")
        };
        write!(
            f,
            "{c:08x} d={d:02x}.{df} x={rx:04x}:{mrx:02x} in={rp:04x}:[{instr}]",
            c = self.cycle,
            d = self.d,
            df = self.df as u8,
            rx = self.r(self.x),
            mrx = self.load(self.x),
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
        Self {
            m,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        self.cycle = 0;
        self.bus = 0;
        self.ie = true;
        self.r[0] = 0;
        self.p = 0;
        self.x = 0;
    }

    fn r(&self, r: u8) -> u16 {
        self.r[r as usize]
    }

    fn r_mut(&mut self, r: u8) -> &mut u16 {
        &mut self.r[r as usize]
    }

    fn store(&mut self, r: u8, v: u8) {
        self.m.store(self.r(r), v);
    }

    fn load(&self, r: u8) -> u8 {
        self.m.load(self.r(r))
    }

    fn load2(&self, r: u8) -> (u8, u8) {
        let addr = self.r(r);
        (self.m.load(addr), self.m.load(addr + 1))
    }

    fn dec(&mut self, r: u8) {
        let r = self.r_mut(r);
        *r = (*r as i32 - 1) as u16;
    }

    fn inc(&mut self, r: u8) {
        let r = self.r_mut(r);
        *r = (*r as u32 + 1) as u16;
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

    pub fn print_registers(&self) {
        println!(
            "d={d:02x} df={df} p={p:x} x={x:x} t={t:04x} ie={ie} q={q}",
            d = self.d,
            df = self.df as u8,
            p = self.p,
            x = self.x,
            t = self.t,
            ie = self.ie as u8,
            q = self.q as u8,
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

    pub fn step(&mut self) -> Status {
        let inst = self.load(self.p);
        self.inc(self.p);
        let inst_hi = inst & 0xf0;
        let inst_lo = inst & 0x0f;
        self.cycle += 2;
        match inst_hi {
            0x00 => {
                if inst_lo == 0 {
                    self.dec(self.p);
                    return Status::Idle;
                } else {
                    // LDN: M(R(N)) → D; FOR N not 0
                    self.d = self.load(inst_lo);
                }
            }
            // INC: R(N) + 1 → R(N)
            0x10 => {
                self.inc(inst_lo);
            }
            // DEC: R(N) - 1 → R(N)
            0x20 => {
                self.dec(inst_lo);
            }
            // Short branch
            0x30 => {
                let imm = self.load(self.p);
                let inv = (inst_lo & 0x8) > 0;
                let mut br = match inst_lo & 0x7 {
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
                    self.plo(self.p, imm);
                } else {
                    self.inc(self.p);
                }
            }
            // LDA: M(R(N)) → D; R(N) + 1 → R(N)
            0x40 => {
                self.d = self.load(inst_lo);
                self.inc(inst_lo);
            }
            // STR: D → M(R(N))
            0x50 => {
                self.store(inst_lo, self.d);
            }
            // INP/OUT
            0x60 => {
                match inst_lo {
                    // IRX: R(X) + 1 → R(X)
                    0x0 => {
                        self.inc(self.x);
                    }
                    // OUT4:
                    0x4 => {
                        let v = self.load(self.x);
                        println!("OUT: {v:02x}");
                    }
                    _ => {}
                }
            }
            // Miscellaneous
            0x70 => match inst_lo {
                // RET: M(R(X)) → (X, P); R(X) + 1 → R(X), 1 → lE
                0x0 => {
                    let v = self.load(self.x);
                    self.x = v + 1;
                    self.p = v;
                    self.ie = true;
                }
                // DIS: M(R(X)) → (X, P); R(X) + 1 → R(X), 0 → lE
                0x1 => {
                    let v = self.load(self.x);
                    self.x = (v >> 4) + 1;
                    self.p = v & 0x3;
                    self.ie = false;
                }
                // LDXA: M(R(X)) → D; R(X) + 1 → R(X)
                0x2 => {
                    self.d = self.load(self.x);
                    self.inc(self.x);
                }
                // STXD: D → M(R(X)); R(X) - 1 → R(X)
                0x3 => {
                    self.store(self.x, self.d);
                    self.dec(self.x);
                }
                // ADDC: M(R(X)) + D + DF → DF, D
                0x4 => {
                    (self.df, self.d) = addc(self.load(self.x), self.d, self.df);
                }
                // SDB: M(R(X)) - D - (Not DF) → DF, D
                0x5 => {
                    (self.df, self.d) = subc(self.load(self.x), self.d, !self.df);
                }
                // SHRC: SHIFT D RIGHT, LSB(D) → DF, DF → MSB(D)
                0x6 => {
                    let df = self.d & 0x01 != 0;
                    self.d >>= 1;
                    if self.df {
                        self.d |= 0x80;
                    }
                    self.df = df;
                }
                // SMB: D-M(R(X))-(NOT DF) → DF, D
                0x7 => {
                    (self.df, self.d) = subc(self.d, self.load(self.x), !self.df);
                }
                // SAVE: T → M(R(X))
                0x8 => {
                    self.store(self.x, self.t);
                }
                // MARK: (X, P) → T; (X, P) → M(R(2)), THEN P → X; R(2) - 1 → R(2)
                0x9 => {
                    self.t = (self.x << 4) | self.p;
                    self.store(2, self.t);
                    self.x = self.p;
                    self.inc(2);
                }
                // REQ: 0→Q
                0xa => {
                    self.q = false;
                }
                // SEQ: 1→Q
                0xb => {
                    self.q = true;
                }
                // ADCI: M(R(P)) + D + DF → DF, D; R(P) + 1 → R(P)
                0xc => {
                    (self.df, self.d) = addc(self.load(self.p), self.d, self.df);
                    self.inc(self.p);
                }
                // SDBI: M(R(P)) - D - (Not DF) → DF, D; R(P) + 1 → R(P)
                0xd => {
                    (self.df, self.d) = subc(self.load(self.p), self.d, !self.df);
                    self.inc(self.p);
                }
                // SHLC: SHIFT D LEFT, MSB(D) → DF, DF → LSB(D)
                0xe => {
                    let df = self.d & 0x80 != 0;
                    self.d <<= 1;
                    if self.df {
                        self.d |= 0x01;
                    }
                    self.df = df;
                }
                // SMBI: D-M(R(P))-(NOT DF) → DF, D; R(P) + 1 → R(P)
                0xf => {
                    (self.df, self.d) = subc(self.d, self.load(self.p), !self.df);
                    self.inc(self.p);
                }
                _ => unreachable!(),
            },
            // GLO
            0x80 => {
                self.d = self.glo(inst_lo);
            }
            // GHI
            0x90 => {
                self.d = self.ghi(inst_lo);
            }
            // PLO
            0xa0 => {
                self.plo(inst_lo, self.d);
            }
            // PHI
            0xb0 => {
                self.phi(inst_lo, self.d);
            }
            // Long branch
            0xc0 => {
                let (imm1, imm2) = self.load2(self.p);
                self.cycle += 1;
                let inv = (inst_lo & 0x8) > 0;
                let skp = (inst_lo & 0x4) > 0;
                let mut br = match inst_lo & 0x3 {
                    0x0 => {
                        if skp && inv {
                            self.ie
                        } else {
                            true
                        }
                    }
                    0x1 => self.q,
                    0x2 => self.d == 0,
                    0x3 => self.df,
                    _ => unreachable!(),
                };
                if inv {
                    br = !br;
                }
                if !br {
                    self.inc(self.p);
                    self.inc(self.p);
                } else if !skp {
                    self.phi(self.p, imm1);
                    self.plo(self.p, imm2);
                }
            }
            // SEP
            0xd0 => {
                self.p = inst_lo;
            }
            // SEX
            0xe0 => {
                self.x = inst_lo;
            }
            // Arithmetic & logic
            0xf0 => {
                let imm = (inst_lo & 0x8 != 0) && inst_lo != 0xe;
                let operand = if imm {
                    let v = self.load(self.p);
                    self.inc(self.p);
                    v
                } else {
                    self.load(self.x)
                };
                (self.df, self.d) = match inst_lo & 0x7 {
                    0x0 => (self.df, operand),
                    0x1 => (self.df, self.d | operand),
                    0x2 => (self.df, self.d & operand),
                    0x3 => (self.df, self.d ^ operand),
                    0x4 => add(self.d, operand),
                    0x5 => sub(operand, self.d),
                    0x6 => match inst_lo {
                        0x6 => (self.d & 0x01 != 0, self.d >> 1),
                        0xe => (self.d & 0x80 != 0, self.d << 1),
                        _ => unreachable!(),
                    },
                    0x7 => sub(self.d, operand),
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
        if self.breakpoints.contains(&self.r(self.p)) {
            Status::Breakpoint
        } else {
            Status::Ready
        }
    }
}
