use const_for::const_for;

/// Numbered or named register.
#[derive(Clone, Copy, Debug)]
pub enum Reg {
    R(u8),
    X,
    P,
}

/// ALU operation
#[derive(Clone, Copy, Debug)]
pub enum AluOp {
    Or,
    And,
    Xor,
    Shl,
    Shr,
    Shlc,
    Shrc,
    Add,
    Adc,
    Sd,
    Sdb,
    Sm,
    Smb,
}

/// Sampled bit for test ops.
#[derive(Clone, Copy, Debug)]
pub enum Bit {
    True, // true
    Q,    // Q
    DZ,   // D == 0
    DF,   // DF
    IE,   // IE
    NEF1, // !EF1
    NEF2, // !EF2
    NEF3, // !EF3
    NEF4, // !EF4
}

/// Micro-operation which takes a single tick.
#[derive(Clone, Copy, Debug)]
pub enum MicroOp {
    Idle,
    Fetch,
    Inc(Reg),
    Dec(Reg),
    SetBusD,
    SetBusT,
    GetBus,
    GetLo(Reg),
    GetHi(Reg),
    PutLo(Reg),
    PutHi(Reg),
    SetQ(bool),
    SetP(u8),
    SetX(u8),
    Mark,
    Alu(AluOp),
    Ret { ie: bool },
    Test { bit: Bit, inv: bool },
    Br,
    LbrLatch,
    Lbr,
    Ls,
}

/// Memory access pattern.
#[derive(Debug, Clone, Copy)]
pub enum Access {
    /// No memory access.
    None,
    /// Read from the memory address in the provided register.
    Read(Reg),
    /// Write to the memory address in the provided register.
    Write(Reg),
    /// Set the N pins, and read from the memory address in the provided register.
    Out(Reg, u8),
    /// Set the N pins, and write to the memory address in the provided register.
    Inp(Reg, u8),
}

const TICKS: usize = 16;

#[derive(Debug, Clone, Copy)]
#[repr(align(64))]
pub struct Cycle {
    pub access: Access,
    pub ticks: [MicroOp; TICKS],
}
impl Cycle {
    const fn empty() -> Self {
        Self {
            ticks: [MicroOp::Idle; 16],
            access: None,
        }
    }

    const fn set(&mut self, tick: u8, op: MicroOp) {
        assert!(tick < 16);
        let tick_op = &mut self.ticks[tick as usize];
        assert!(matches!(tick_op, MicroOp::Idle), "redefined");
        *tick_op = op;
    }
}

use Access::*;
use AluOp::*;
use Bit::*;
use MicroOp::*;
use Reg::*;

macro_rules! define_cycle {
    ($access:expr, {
        $( $tick:literal: $op:expr),* $(,)?
    }) => {{
        let mut c = Cycle::empty();
        c.access = $access;
        $( c.set($tick, $op); )*
        c
    }};
}

macro_rules! define_instr {
    ($table:ident, $opcode:expr, $access:expr, {
        $( $tick:literal: $op:expr),* $(,)?
    }) => {
        let cycle = &mut $table[$opcode as usize];
        cycle.access = $access;
        $( cycle.set($tick, $op); )*
    };
}

macro_rules! define_bxx_instr {
    ($table:ident, $opcode:expr, $bit:path, $inv:literal) => {
        define_instr!($table, $opcode, Read(P), {
            1: Test { bit: $bit, inv: $inv },
            3: Br,
        });
    };
    ($table:ident, $opcode:expr, $bit:path) => {
        define_bxx_instr!($table, $opcode, $bit, false)
    };
    ($table:ident, $opcode:expr, !$bit:path) => {
        define_bxx_instr!($table, $opcode, $bit, true)
    };
}

macro_rules! define_lbxx_instr {
    ($table:ident, $opcode:expr, $bit:path, $inv:literal) => {
        define_instr!($table, $opcode, Read(P), {
            1: Test { bit: $bit, inv: $inv},
            3: LbrLatch,
            11: Lbr, // second machine cycle, phase 3
        });
    };
    ($table:ident, $opcode:expr, $bit:path) => {
        define_lbxx_instr!($table, $opcode, $bit, false)
    };
    ($table:ident, $opcode:expr, !$bit:path) => {
        define_lbxx_instr!($table, $opcode, $bit, true)
    };
}

macro_rules! define_lsxx_instr {
    ($table:ident, $opcode:expr, $bit:path, $inv:literal) => {
        define_instr!($table, $opcode, None, {
            1: Test { bit: $bit, inv: $inv},
            3: Ls,
            11: Ls, // second machine cycle, phase 3
        });
    };
    ($table:ident, $opcode:expr, $bit:path) => {
        define_lsxx_instr!($table, $opcode, $bit, false)
    };
    ($table:ident, $opcode:expr, !$bit:path) => {
        define_lsxx_instr!($table, $opcode, $bit, true)
    };
}

pub const FETCH_CYCLE: Cycle = define_cycle!(Read(P), { 3: Fetch });
pub const DMA_IN_CYCLE: Cycle = define_cycle!(Write(R(0)), { 4: Inc(P) });
pub const DMA_OUT_CYCLE: Cycle = define_cycle!(Read(R(0)), { 4: Inc(P) });
pub const INSTR_CYCLE_TABLE: [Cycle; 256] = {
    let mut t = [Cycle::empty(); 256];

    // IDL
    define_instr!(t, 0x00, Read(R(0)), {});

    // LDN
    const_for!(n in 1..16 => {
        define_instr!(t, n, Read(R(n)), { 3: GetBus });
    });

    // INC / DEC
    const_for!(n in 0..16 => {
        define_instr!(t, 0x10 | n, None, { 4: Inc(R(n)) });
        define_instr!(t, 0x20 | n, None, { 4: Dec(R(n)) });
    });

    // Bxx
    define_bxx_instr!(t, 0x30, True);
    define_bxx_instr!(t, 0x31, Q);
    define_bxx_instr!(t, 0x32, DZ);
    define_bxx_instr!(t, 0x33, DF);
    define_bxx_instr!(t, 0x34, NEF1);
    define_bxx_instr!(t, 0x35, NEF2);
    define_bxx_instr!(t, 0x36, NEF3);
    define_bxx_instr!(t, 0x37, NEF4);
    define_bxx_instr!(t, 0x38, !True);
    define_bxx_instr!(t, 0x39, !Q);
    define_bxx_instr!(t, 0x3a, !DZ);
    define_bxx_instr!(t, 0x3b, !DF);
    define_bxx_instr!(t, 0x3c, !NEF1);
    define_bxx_instr!(t, 0x3d, !NEF2);
    define_bxx_instr!(t, 0x3e, !NEF3);
    define_bxx_instr!(t, 0x3f, !NEF4);

    // LDA, STR
    const_for!(n in 0..16 => {
        define_instr!(t, 0x40 | n, Read(R(n)), { 3: GetBus, 4: Inc(R(n)) });
        define_instr!(t, 0x50 | n, Write(R(n)), { 3: SetBusD });
    });

    // IRX
    define_instr!(t, 0x60, None, { 4: Inc(X) });

    // OUT
    const_for!(n in 1..8 => {
        define_instr!(t, 0x60 | n, Out(X, n), { 4: Inc(X) });
    });

    // INP
    const_for!(n in 9..16 => {
        define_instr!(t, 0x60 | n, Inp(X, n), { 3: GetBus });
    });

    // RET, DIS
    define_instr!(t, 0x70, Read(X), { 3: Ret{ ie: true } });
    define_instr!(t, 0x71, Read(X), { 3: Ret{ ie: false } });

    // LDXA
    define_instr!(t, 0x72, Read(X), { 3: GetBus, 4: Inc(X) });

    // STXD
    define_instr!(t, 0x73, Write(X), { 3: SetBusD, 4: Dec(X) });

    // ADC, SDB, SHRC, SMB
    define_instr!(t, 0x74, Read(X), { 3: Alu(Adc) });
    define_instr!(t, 0x75, Read(X), { 3: Alu(Sdb) });
    define_instr!(t, 0x76, None, { 3: Alu(Shrc) });
    define_instr!(t, 0x77, Read(X), { 3: Alu(Smb) });

    // SAV, MARK
    define_instr!(t, 0x78, Write(X), { 3: SetBusT });
    define_instr!(t, 0x79, Write(R(2)), { 3: Mark });

    // REQ, SEQ
    define_instr!(t, 0x7a, None, { 3: SetQ(false) });
    define_instr!(t, 0x7b, None, { 3: SetQ(true) });

    // ADCI, SDBI, SHLC, SMBI
    define_instr!(t, 0x7c, Read(P), { 3: Alu(Adc), 4: Inc(P) });
    define_instr!(t, 0x7d, Read(P), { 3: Alu(Sdb), 4: Inc(P) });
    define_instr!(t, 0x7e, None, { 3: Alu(Shlc) });
    define_instr!(t, 0x7f, Read(P), { 3: Alu(Smb), 4: Inc(P) });

    // GLO, GHI, PLO, PHI
    const_for!(n in 0..16 => {
        define_instr!(t, 0x80 | n, None, { 3: GetLo(R(n)) });
        define_instr!(t, 0x90 | n, None, { 3: GetHi(R(n)) });
        define_instr!(t, 0xa0 | n, None, { 3: PutLo(R(n)) });
        define_instr!(t, 0xb0 | n, None, { 3: PutHi(R(n)) });
    });

    // LBxx, LSxx
    define_lbxx_instr!(t, 0xc0, True);
    define_lbxx_instr!(t, 0xc1, Q);
    define_lbxx_instr!(t, 0xc2, DZ);
    define_lbxx_instr!(t, 0xc3, DF);

    define_lsxx_instr!(t, 0xc4, !True);
    define_lsxx_instr!(t, 0xc5, !Q);
    define_lsxx_instr!(t, 0xc6, !DZ);
    define_lsxx_instr!(t, 0xc7, !DF);

    define_lbxx_instr!(t, 0xc8, !True);
    define_lbxx_instr!(t, 0xc9, !Q);
    define_lbxx_instr!(t, 0xca, !DZ);
    define_lbxx_instr!(t, 0xcb, !DF);

    define_lsxx_instr!(t, 0xcc, IE);
    define_lsxx_instr!(t, 0xcd, Q);
    define_lsxx_instr!(t, 0xce, DZ);
    define_lsxx_instr!(t, 0xcf, DF);

    // SEP, SEX
    const_for!(n in 0..16 => {
        define_instr!(t, 0xd0 | n, None, { 3: SetP(n) });
        define_instr!(t, 0xe0 | n, None, { 3: SetX(n) });
    });

    // LDX
    define_instr!(t, 0xf0, Read(X), { 3: GetBus });

    // OR, AND, XOR, ADD, SD, SHR, SM
    define_instr!(t, 0xf1, Read(X), { 3: Alu(Or) });
    define_instr!(t, 0xf2, Read(X), { 3: Alu(And) });
    define_instr!(t, 0xf3, Read(X), { 3: Alu(Xor) });
    define_instr!(t, 0xf4, Read(X), { 3: Alu(Add) });
    define_instr!(t, 0xf5, Read(X), { 3: Alu(Sd) });
    define_instr!(t, 0xf6, None, { 3: Alu(Shr) });
    define_instr!(t, 0xf7, Read(X), { 3: Alu(Sm) });

    // LDI
    define_instr!(t, 0xf8, Read(P), { 3: GetBus, 4: Inc(P) });

    // ORI, ANI, XRI, ADI, SDI, SHL, SMI
    define_instr!(t, 0xf9, Read(P), { 3: Alu(Or), 4: Inc(P) });
    define_instr!(t, 0xfa, Read(P), { 3: Alu(And), 4: Inc(P) });
    define_instr!(t, 0xfb, Read(P), { 3: Alu(Xor), 4: Inc(P) });
    define_instr!(t, 0xfc, Read(P), { 3: Alu(Add), 4: Inc(P) });
    define_instr!(t, 0xfd, Read(P), { 3: Alu(Sd), 4: Inc(P) });
    define_instr!(t, 0xfe, None, { 3: Alu(Shl) });
    define_instr!(t, 0xff, Read(P), { 3: Alu(Sm), 4: Inc(P) });

    t
};

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;

    fn check_bus_access(name: &str, cycle: &Cycle) {
        for tick in 0..TICKS {
            let op = cycle.ticks[tick];
            let is_write = matches!(op, SetBusD | SetBusT | Mark);
            let is_read = matches!(
                op,
                Fetch
                    | GetBus
                    | Alu(Or | And | Xor | Add | Adc | Sd | Sdb | Sm | Smb)
                    | Br
                    | LbrLatch
                    | Lbr
                    | Ret { .. }
            );
            // Ops that access the bus must do so at the correct phase.
            if is_read || is_write {
                let phase = tick & 7;
                assert_eq!(phase, 3, "{name}: {op:?} at wrong phase");
            }
            // Ops that read from the bus must declare read/inp access.
            if is_read {
                assert_matches!(
                    cycle.access,
                    Read(_) | Inp(_, _),
                    "{name}: {op:?} requires Read|Inp access"
                );
            }
            // Ops that write to the bus must declare write access.
            if is_write {
                assert_matches!(
                    cycle.access,
                    Write(_),
                    "{name}: {op:?} requires Write|In access"
                );
            }
        }
    }

    #[test]
    fn test_cycle_timings() {
        check_bus_access("fetch", &FETCH_CYCLE);
        check_bus_access("dma-in", &DMA_IN_CYCLE);
        check_bus_access("dma-out", &DMA_OUT_CYCLE);
        for (opcode, instr) in INSTR_CYCLE_TABLE.iter().enumerate() {
            check_bus_access(&format!("instr {opcode:#04x}"), instr);
        }
    }
}
