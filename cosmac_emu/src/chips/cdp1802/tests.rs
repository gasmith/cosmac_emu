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

    fn tick(&mut self) {
        self.cpu.tick(&mut self.pins);
        self.mem.tick(&mut self.pins, self.write_enable).ok();
    }

    fn tick_n(&mut self, n: usize) {
        for _ in 0..n {
            self.tick()
        }
    }

    fn is_waiting(&self) -> bool {
        self.cpu.is_waiting(self.pins)
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
    assert_eq!(sys.cpu.x, 0);
    assert_eq!(sys.cpu.p, 0);
    assert_eq!(sys.cpu.r[0], 0);
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
