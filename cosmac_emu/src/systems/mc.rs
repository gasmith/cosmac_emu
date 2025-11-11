//! Lee Hart's 1802 Membership Card, but with a UART

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use crate::{
    chips::cdp1802::{Cdp1802, Cdp1802Pins, Memory},
    instr::InstrSchema as _,
    time::TimeTracker,
    uart::{Uart, UartRxError},
};

/// The length of the execution opcode history, which we use for heuristics.
///
/// See [`MembershipCard::is_cpu_waiting_for_uart`] for more details about the value of this
/// constant.
const OPCODE_HISTORY_LEN: usize = 104;

#[derive(Debug, Clone, Copy)]
pub enum Status {
    UartRead,
    UartWrite,
    Tick,
}

pub struct Builder {
    memory: Memory,
    front: FrontPanel,
    invert_ef: bool,
    invert_q: bool,
    clk_freq: u32,
    speed: Option<f64>,
    uart: Option<Box<dyn Uart>>,
}
impl Default for Builder {
    fn default() -> Self {
        Self {
            memory: Memory::default(),
            front: FrontPanel::default(),
            invert_ef: false,
            invert_q: false,
            clk_freq: 4_000_000,
            speed: None,
            uart: None,
        }
    }
}
impl Builder {
    pub fn with_memory(self, memory: Memory) -> Self {
        Self { memory, ..self }
    }

    pub fn with_invert_ef(self, invert_ef: bool) -> Self {
        Self { invert_ef, ..self }
    }

    pub fn with_invert_q(self, invert_q: bool) -> Self {
        Self { invert_q, ..self }
    }

    pub fn with_clock_freq(self, clk_freq: u32) -> Self {
        Self { clk_freq, ..self }
    }

    pub fn with_speed(self, speed: Option<f64>) -> Self {
        assert!(speed.is_none_or(|s| s > 0.));
        Self { speed, ..self }
    }

    pub fn with_uart(self, uart: impl Into<Option<Box<dyn Uart>>>) -> Self {
        let uart = uart.into();
        Self { uart, ..self }
    }

    pub fn build(self) -> MembershipCard {
        let mut cpu = Cdp1802::default();
        let mut cpu_pins = Cdp1802Pins::default();
        if self.invert_ef {
            cpu_pins.set_ef(0);
        }
        cpu.reset(&mut cpu_pins);

        let clock_freq_f64 = self.clk_freq as f64;
        let tick_duration = Duration::from_secs_f64(clock_freq_f64.recip());
        let time_tracker = self.speed.map(TimeTracker::new);

        MembershipCard {
            now: Duration::default(),
            tick_duration,
            time_tracker,
            cpu_pins,
            cpu,
            memory: self.memory,
            front_panel: self.front,
            last_front_panel: self.front,
            uart: self.uart,
            invert_ef: self.invert_ef,
            invert_q: self.invert_q,
            last_pc: 0,
            opcode_history: VecDeque::with_capacity(OPCODE_HISTORY_LEN),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrontPanel {
    pub out_buffer: u8,
    pub inp_buffer: u8,
    pub inp: bool,   // true = pressed
    pub clear: bool, // true = down
    pub wait: bool,  // true = down
    pub read: bool,  // true = down, false = write
}

#[derive(Debug)]
pub struct MembershipCard {
    now: Duration,
    tick_duration: Duration,
    time_tracker: Option<TimeTracker>,
    cpu_pins: Cdp1802Pins,
    cpu: Cdp1802,
    memory: Memory,
    front_panel: FrontPanel,
    last_front_panel: FrontPanel,
    uart: Option<Box<dyn Uart>>,
    invert_ef: bool,
    invert_q: bool,
    last_pc: u16,
    opcode_history: VecDeque<u8>,
}
impl Default for MembershipCard {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl MembershipCard {
    /// Returns a builder for the membership card.
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Returns the value of R[P] the last time the system entered S0.
    pub fn last_pc(&self) -> u16 {
        self.last_pc
    }

    /// Returns a reference to the CPU.
    pub fn cpu(&self) -> &Cdp1802 {
        &self.cpu
    }

    /// Returns a reference to memory.
    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    /// Returns a reference to the front panel.
    pub fn front_panel(&self) -> &FrontPanel {
        &self.front_panel
    }

    /// Returns a mutable reference to the front panel.
    pub fn front_panel_mut(&mut self) -> &mut FrontPanel {
        &mut self.front_panel
    }

    /// Ticks the clock.
    pub fn tick(&mut self) {
        let start = Instant::now();

        // Propagate signals from front panel.
        let load = self.front_panel.clear && self.front_panel.wait;
        self.cpu_pins.set_clear(!self.front_panel.clear);
        self.cpu_pins.set_wait(!self.front_panel.wait);
        self.cpu_pins
            .set_ef4(self.invert_ef ^ !self.front_panel.inp);
        let write_enable = !self.front_panel.read;

        // In the load state, when the inp button is released, set /DmaIn.
        if load && self.last_front_panel.inp && !self.front_panel.inp {
            self.cpu_pins.set_dma_in(false);
        }
        self.last_front_panel = self.front_panel;

        // Tick cpu.
        self.cpu.tick(&mut self.cpu_pins);

        // Update PC.
        if self.cpu.is_fetch_tick0() {
            self.last_pc = self.cpu.rp();
        } else if load {
            // In load mode, use R[P]-1 to show the previously-written instruction. Decode
            // the instruction at last_pc to determine its size to determine whether it has
            // been completely written.
            let rp = self.cpu.rp().saturating_sub(1);
            if rp > self.last_pc && rp < self.last_pc + 3 {
                let instr = self.memory.get_instr_at(self.last_pc);
                let size = instr.map_or(1, |i| i.size() as u16);
                if rp >= self.last_pc + size {
                    self.last_pc = rp;
                }
            } else if rp != self.last_pc {
                self.last_pc = rp;
            }
        }

        // Update opcode history.
        if self.front_panel.clear {
            self.opcode_history.truncate(0);
        } else if let Some(opcode) = self.cpu.get_exec_opcode() {
            self.push_opcode_history(opcode);
        }

        // Reset /DmaIn in S2.
        if !self.cpu_pins.get_dma_in() && self.cpu_pins.get_sc1() {
            self.cpu_pins.set_dma_in(true);
        }

        // Propagate input buffer on /MWR when N2 is set or we're in load mode.
        let n2_or_load = self.cpu_pins.get_n2() || load;
        if !self.cpu_pins.get_mwr() && n2_or_load {
            self.cpu_pins.set_bus(self.front_panel.inp_buffer);
        }

        // Tick memory.
        let result = self.memory.tick(&mut self.cpu_pins, write_enable);
        if let Err(err) = result {
            log::warn!("memory: {err}")
        }

        // Latch output on data strobe when either N2 is set or we're in load mode.
        if !self.cpu_pins.get_mrd() && self.cpu_pins.get_tpb() && n2_or_load {
            self.front_panel.out_buffer = self.cpu_pins.get_bus();
        }

        if let Some(uart) = &mut self.uart {
            // Tick or reset the UART as necessary.
            if self.front_panel.clear {
                uart.reset();
            } else if !self.front_panel.wait {
                // Propagate Q to UART rx pin.
                uart.set_rx_pin(self.invert_q ^ !self.cpu_pins.get_q());
                uart.tick();
            }
            // Propagate UART tx pin to EF3.
            self.cpu_pins.set_ef3(self.invert_ef ^ uart.get_tx_pin());
        }

        // Update clock, and sleep if we're too far ahead of schedule.
        if let Some(tt) = &mut self.time_tracker {
            tt.tick(start.elapsed(), self.tick_duration);
        }
        self.now += self.tick_duration;
    }

    /// Records the current opcode, if the CPU has just entered S1.
    fn push_opcode_history(&mut self, opcode: u8) {
        while self.opcode_history.len() + 1 >= OPCODE_HISTORY_LEN {
            self.opcode_history.pop_front();
        }
        self.opcode_history.push_back(opcode);
    }

    /// Reads one byte from the UART.
    pub fn uart_read(&mut self) -> Result<u8, UartRxError> {
        let (result, hold_cycles) = self
            .uart
            .as_mut()
            .map(|uart| (uart.rx(), uart.rx_hold_cycles()))
            .expect("no uart");
        for _ in 0..hold_cycles {
            self.tick();
        }
        result
    }

    /// Writes one byte to the UART.
    pub fn uart_write(&mut self, byte: u8) {
        let hold_cycles = self
            .uart
            .as_mut()
            .map(|uart| {
                uart.tx(byte);
                uart.tx_hold_cycles()
            })
            .expect("no uart");
        for _ in 0..hold_cycles {
            self.tick();
        }
    }

    /// Polls the memebrership card for status.
    pub fn poll(&self) -> Option<Status> {
        if self.is_uart_rx_ready() {
            Some(Status::UartRead)
        } else if self.is_cpu_waiting_for_uart() {
            Some(Status::UartWrite)
        } else if !self.cpu.is_waiting(self.cpu_pins) || self.is_front_panel_updated() {
            Some(Status::Tick)
        } else {
            None
        }
    }

    /// Returns true if there is rx data available on the UART.
    ///
    /// This function returns true if all of the following conditions are met:
    ///
    ///  - The CPU is running.
    ///  - The system has a UART configured.
    ///  - The UART has data ready.
    ///
    fn is_uart_rx_ready(&self) -> bool {
        !self.front_panel.clear
            && !self.front_panel.wait
            && self.uart.as_ref().is_some_and(|uart| uart.is_rx_ready())
    }

    /// Returns true if the CPU seems to be waiting for data from the UART.
    ///
    /// This function returns true if all of the following conditions are met:
    ///
    ///  - The CPU is running.
    ///  - The system has a UART configured.
    ///  - The UART's transmit side is idle.
    ///  - The CPU has just entered S1 and is branching on EF3.
    ///  - The CPU executed the same branch opcode recently.
    ///
    /// This is an approximation, and it's not perfect. Just because the program is probing the EF
    /// lines, that doesn't necessarily mean it's in a tight loop waiting for serial data. It
    /// doesn't necessarily have to be a tight loop either. Some programs poll for rx data
    /// periodically, using spare cycles to do other work.
    ///
    /// Given the UART baud rate and clock frequency, we can put an upper bound on the number of
    /// instructions that the device can execute without missing a start symbol. If, for example,
    /// we presume the clock is 4MHz, and the baud rate is 2400. A standard S0/S1 cycle takes 16
    /// clock cycles. 4_000_000 / (2_400 * 16) = 104 instructions. This seems like a reasonable
    /// bound; most ROMs are written for compatibility with slower system clocks, and higher baud
    /// rates.
    fn is_cpu_waiting_for_uart(&self) -> bool {
        if self.front_panel.clear
            || self.front_panel.wait
            || !self.uart.as_ref().is_some_and(|uart| uart.is_tx_idle())
        {
            return false;
        }
        match self.cpu.get_exec_opcode() {
            Some(opcode @ (0x36 | 0x3e)) => {
                self.opcode_history
                    .iter()
                    .filter(|oc| **oc == opcode)
                    .count()
                    > 1
            }
            _ => false,
        }
    }

    /// Returns true if there have been changes to the front panel since the last tick.
    fn is_front_panel_updated(&self) -> bool {
        self.front_panel != self.last_front_panel
    }
}
