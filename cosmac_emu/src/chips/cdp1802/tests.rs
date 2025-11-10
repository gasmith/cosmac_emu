use assert_matches::assert_matches;

use super::{Cdp1802, Cdp1802Pins, Memory, State};

struct TestSystem {
    pins: Cdp1802Pins,
    cpu: Cdp1802,
    mem: Memory,
    write_enable: bool,
}
impl TestSystem {
    fn new() -> Self {
        Self {
            pins: Cdp1802Pins(Cdp1802Pins::mask_all()),
            cpu: Cdp1802::default(),
            mem: Memory::default(),
            write_enable: true,
        }
    }

    fn new_with_program(data: impl Into<Vec<u8>>) -> Self {
        let mem = Memory::builder().with_image(0, data).build().unwrap();
        Self {
            pins: Cdp1802Pins(Cdp1802Pins::mask_all()),
            cpu: Cdp1802::default(),
            mem,
            write_enable: true,
        }
    }

    fn tick(&mut self) {
        self.cpu.tick(&mut self.pins);
        self.mem.tick(&mut self.pins, self.write_enable).ok();
    }

    fn tick_n(&mut self, n: usize) {
        for _ in 0..n {
            self.tick();
            println!("{} {}", self.cpu, self.pins);
        }
    }

    fn is_waiting(&self) -> bool {
        self.cpu.is_waiting(self.pins)
    }

    fn reset(&mut self) {
        println!("reset");
        self.pins.set_clear(false);
        self.tick();
        self.pins.set_clear(true);
        self.tick_n(9);
    }
}

#[test]
fn test_init() {
    let mut sys = TestSystem::new();
    sys.tick();
    assert!(!sys.is_waiting());
    assert_matches!(sys.cpu.state, State::Init(1));
    sys.tick_n(8);
    assert_matches!(sys.cpu.state, State::Fetch(0));
    sys.tick_n(8);
    assert_matches!(sys.cpu.state, State::Execute(0));
    sys.tick_n(7);
    assert_matches!(sys.cpu.state, State::Execute(7));
    assert!(sys.is_waiting());
    sys.tick();
    assert_matches!(sys.cpu.state, State::Execute(0));
}

#[test]
fn test_reset_to_run() {
    let mut sys = TestSystem::new();
    sys.cpu = rand::random();
    sys.pins.set_clear(false);
    sys.tick();
    assert_matches!(sys.cpu.state, State::Init(0));
    assert_eq!(sys.cpu.i, 0);
    assert_eq!(sys.cpu.n, 0);
    assert!(sys.cpu.ie);
    assert!(!sys.pins.get_q());
    assert_eq!(sys.pins.get_bus(), 0x00);

    // Stay in reset until clear goes high.
    sys.pins.set_bus(0xff);
    sys.tick();
    assert_matches!(sys.cpu.state, State::Init(0));
    assert_eq!(sys.pins.get_bus(), 0x00);

    // Init takes 9 cycles, and interrupts are disabled coming out of init.
    sys.pins.set_clear(true);
    sys.pins.set_intr(false);
    sys.pins.set_bus(0xff);
    sys.tick();
    assert_matches!(sys.cpu.state, State::Init(1));
    assert_eq!(sys.cpu.x, 0);
    assert_eq!(sys.cpu.p, 0);
    assert_eq!(sys.cpu.r[0], 0);
    assert_eq!(sys.pins.get_bus(), 0x00);
    sys.tick_n(7);
    assert_matches!(sys.cpu.state, State::Init(8));
    sys.tick();
    assert_matches!(sys.cpu.state, State::Fetch(0));
}

#[test]
fn test_reset_to_load() {
    let mut sys = TestSystem::new();
    sys.cpu = rand::random();
    sys.pins.set_clear(false);
    sys.tick();
    assert_matches!(sys.cpu.state, State::Init(0));

    // Stay in reset until wait goes low.
    sys.tick();
    assert_matches!(sys.cpu.state, State::Init(0));

    // Init takes 9 cycles, and interrupts are disabled coming out of init.
    sys.pins.set_wait(false);
    sys.tick();
    assert_matches!(sys.cpu.state, State::Init(1));
    sys.tick_n(8);
    assert_matches!(sys.cpu.state, State::Execute(0));
    assert_matches!(sys.cpu.r[0], 0);
    sys.tick_n(7);

    // At this point the device is waiting for inputs to change.
    assert_matches!(sys.cpu.state, State::Execute(7));
    assert!(sys.is_waiting());

    // Idle transition back to S1.
    sys.tick();
    assert_matches!(sys.cpu.state, State::Execute(0));
    assert_matches!(sys.cpu.r[0], 0);
}

#[test]
fn test_n() {
    let mut sys = TestSystem::new();
    sys.pins.set_clear(false);
    sys.tick();

    // n lines are low at all times except for i/o instructions
    assert_eq!(sys.pins.get_n(), 0);
}

#[test]
fn test_ef_sampling() {
    // ef flags should be sampled at the beginning of s1.
}

#[test]
fn test_intr_dma_sampling() {
    // interrupt and dma pins are sampled between tpb and tpa.
}

#[test]
fn test_intr() {
    // x and p are stored in t after executing the current instruction.
    // x is set to 2
    // p is set to 1
    // ie is set to false
    // one s3 machine cycle
}

#[test]
fn test_dma_in() {
    // finish executing current instruction
    // r[0] points to memory location
    // data is loaded into memory
}

#[test]
fn test_dma_out() {
    // finish executing current instruction
    // r[0] points to memory location
    // data is read from memory
}

#[test]
fn test_dma_priority() {
    // dma in has priority over dma out
}

fn expect_lbr(instr: u8, cb: impl FnOnce(&mut TestSystem)) {
    let mut sys = TestSystem::new_with_program([instr, 0x55, 0xaa]);
    sys.reset();
    cb(&mut sys);
    sys.tick_n(24);
    assert_matches!(sys.cpu.state, State::Fetch(0));
    assert_eq!(sys.cpu.rp(), 0x55aa);
}

fn expect_lskp(instr: u8, cb: impl FnOnce(&mut TestSystem)) {
    let mut sys = TestSystem::new_with_program([instr, 0xff, 0xff]);
    sys.reset();
    cb(&mut sys);
    sys.tick_n(24);
    assert_matches!(sys.cpu.state, State::Fetch(0));
    assert_eq!(sys.cpu.rp(), 3);
}

fn expect_lcnt(instr: u8, cb: impl FnOnce(&mut TestSystem)) {
    let mut sys = TestSystem::new_with_program([instr, 0xff, 0xff]);
    sys.reset();
    cb(&mut sys);
    sys.tick_n(24);
    assert_matches!(sys.cpu.state, State::Fetch(0));
    assert_eq!(sys.cpu.rp(), 1);
}

#[test]
fn test_lbr() {
    expect_lbr(0xc0, |_| ());
}

#[test]
fn test_lbr_at_page_boundary() {
    // Validate that the lbxx implementation stores the high byte of the target address in a
    // temporary register (b), and not r[p]. This is done by placing the high byte of the target
    // address at the edge of a page boundary. The chip must do a full 16-bit increment of r[p] to
    // successfully fetch the low byte on the subsequent page.
    let mut prog = vec![0xc0, 0x01, 0xfe];
    prog.extend([0; 256 + 251]);
    prog.extend([0xc0, 0x55, 0xaa]);
    assert_eq!(prog[0x1ff], 0x55);

    let mut sys = TestSystem::new_with_program(prog);
    sys.reset();

    // first branch
    sys.tick_n(24);
    assert_matches!(sys.cpu.state, State::Fetch(0));
    assert_eq!(sys.cpu.rp(), 0x01fe);

    // second branch
    sys.tick_n(24);
    assert_matches!(sys.cpu.state, State::Fetch(0));
    assert_eq!(sys.cpu.rp(), 0x55aa);
}

#[test]
fn test_lbq() {
    expect_lbr(0xc1, |sys| sys.cpu.out.set_q(true));
    expect_lskp(0xc1, |sys| sys.cpu.out.set_q(false));
}

#[test]
fn test_lbz() {
    expect_lbr(0xc2, |sys| sys.cpu.d = 0);
    expect_lskp(0xc2, |sys| sys.cpu.d = 1);
}

#[test]
fn test_lbdf() {
    expect_lbr(0xc3, |sys| sys.cpu.df = true);
    expect_lskp(0xc3, |sys| sys.cpu.df = false);
}

#[test]
fn test_nop() {
    expect_lcnt(0xc4, |_| ());
}

#[test]
fn test_lsnq() {
    expect_lskp(0xc5, |sys| sys.cpu.out.set_q(false));
    expect_lcnt(0xc5, |sys| sys.cpu.out.set_q(true));
}

#[test]
fn test_lsnz() {
    expect_lskp(0xc6, |sys| sys.cpu.d = 1);
    expect_lcnt(0xc6, |sys| sys.cpu.d = 0);
}

#[test]
fn test_lsnf() {
    expect_lskp(0xc7, |sys| sys.cpu.df = false);
    expect_lcnt(0xc7, |sys| sys.cpu.df = true);
}

#[test]
fn test_lskp() {
    expect_lskp(0xc8, |_| ());
}

#[test]
fn test_lbnq() {
    expect_lbr(0xc9, |sys| sys.cpu.out.set_q(false));
    expect_lskp(0xc9, |sys| sys.cpu.out.set_q(true));
}

#[test]
fn test_lbnz() {
    expect_lbr(0xca, |sys| sys.cpu.d = 1);
    expect_lskp(0xca, |sys| sys.cpu.d = 0);
}

#[test]
fn test_lbnf() {
    expect_lbr(0xcb, |sys| sys.cpu.df = false);
    expect_lskp(0xcb, |sys| sys.cpu.df = true);
}

#[test]
fn test_lsie() {
    expect_lskp(0xcc, |sys| sys.cpu.ie = true);
    expect_lcnt(0xcc, |sys| sys.cpu.ie = false);
}

#[test]
fn test_lsq() {
    expect_lskp(0xcd, |sys| sys.cpu.out.set_q(true));
    expect_lcnt(0xcd, |sys| sys.cpu.out.set_q(false));
}

#[test]
fn test_lsz() {
    expect_lskp(0xce, |sys| sys.cpu.d = 0);
    expect_lcnt(0xce, |sys| sys.cpu.d = 1);
}

#[test]
fn test_lsdf() {
    expect_lskp(0xcf, |sys| sys.cpu.df = true);
    expect_lcnt(0xcf, |sys| sys.cpu.df = false);
}
