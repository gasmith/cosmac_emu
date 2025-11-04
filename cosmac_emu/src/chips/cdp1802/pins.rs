use crate::chips::bits;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Pin {
    // in/out
    Bus0,
    Bus1,
    Bus2,
    Bus3,
    Bus4,
    Bus5,
    Bus6,
    Bus7,

    // out
    Ma0,
    Ma1,
    Ma2,
    Ma3,
    Ma4,
    Ma5,
    Ma6,
    Ma7,
    N0,
    N1,
    N2,
    Q,
    Mrd, // negated
    Mwr, // negated
    Tpb,
    Tpa,
    Sc0,
    Sc1,

    // in (negated)
    DmaIn,
    DmaOut,
    Intr,
    Ef1,
    Ef2,
    Ef3,
    Ef4,
    Clear,
    Wait,
}

#[derive(Clone, Copy)]
pub struct Cdp1802Pins(pub u64);
impl Default for Cdp1802Pins {
    fn default() -> Self {
        Self(u64::MAX & Self::mask_all())
    }
}
impl std::fmt::Debug for Cdp1802Pins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Cdp1802Pins")
            .field(&format!("{:034b}", self.0))
            .finish()
    }
}
impl std::fmt::Display for Cdp1802Pins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bus={bus:02x} ", bus = self.get_bus(),)?;
        write!(
            f,
            "ma={ma:02x} n={n} q={q} mrw={mrw:02b} tp={tp:02b} sc={sc} ",
            ma = self.get_ma(),
            n = self.get_n(),
            q = self.get_q() as u8,
            mrw = bits::get_2(self.0, Pin::Mrd as u8),
            tp = bits::get_2(self.0, Pin::Tpa as u8),
            sc = self.get_sc(),
        )?;
        write!(
            f,
            "dmio={dma:02b} intr={intr} ef={ef:04b} clr={clr} wait={wait}",
            dma = bits::get_2(self.0, Pin::DmaIn as u8),
            intr = self.get_intr() as u8,
            ef = self.get_ef(),
            clr = self.get_clear() as u8,
            wait = self.get_wait() as u8,
        )
    }
}
impl Cdp1802Pins {
    pub const fn mask_bus() -> u64 {
        (1 << ((Pin::Bus7 as u8) + 1)) - 1
    }

    pub const fn mask_bus_out() -> u64 {
        (1 << ((Pin::Sc1 as u8) + 1)) - 1
    }

    pub const fn mask_all() -> u64 {
        (1 << ((Pin::Wait as u8) + 1)) - 1
    }

    pub const fn mask_out() -> u64 {
        !Self::mask_bus() & Self::mask_bus_out()
    }

    pub const fn mask_in() -> u64 {
        !Self::mask_out() & Self::mask_all()
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn set_masked(&mut self, other: Cdp1802Pins, mask: u64) {
        self.0 = (self.0 & !mask) | (other.0 & mask);
    }

    pub fn get_bus(self) -> u8 {
        bits::get_8(self.0, Pin::Bus0 as u8)
    }
    pub fn get_ma(self) -> u8 {
        bits::get_8(self.0, Pin::Ma0 as u8)
    }
    pub fn get_n(self) -> u8 {
        bits::get_3(self.0, Pin::N0 as u8)
    }
    pub fn get_n2(self) -> bool {
        bits::get_1(self.0, Pin::N2 as u8)
    }
    pub fn get_q(self) -> bool {
        bits::get_1(self.0, Pin::Q as u8)
    }
    pub fn get_ef(self) -> u8 {
        bits::get_4(self.0, Pin::Ef1 as u8)
    }
    pub fn get_ef1(self) -> bool {
        bits::get_1(self.0, Pin::Ef1 as u8)
    }
    pub fn get_ef2(self) -> bool {
        bits::get_1(self.0, Pin::Ef2 as u8)
    }
    pub fn get_ef3(self) -> bool {
        bits::get_1(self.0, Pin::Ef3 as u8)
    }
    pub fn get_ef4(self) -> bool {
        bits::get_1(self.0, Pin::Ef4 as u8)
    }
    pub fn get_mrd(self) -> bool {
        bits::get_1(self.0, Pin::Mrd as u8)
    }
    pub fn get_mwr(self) -> bool {
        bits::get_1(self.0, Pin::Mwr as u8)
    }
    pub fn get_tpa(self) -> bool {
        bits::get_1(self.0, Pin::Tpa as u8)
    }
    pub fn get_tpb(self) -> bool {
        bits::get_1(self.0, Pin::Tpb as u8)
    }
    pub fn get_dma_in(self) -> bool {
        bits::get_1(self.0, Pin::DmaIn as u8)
    }
    pub fn get_dma_out(self) -> bool {
        bits::get_1(self.0, Pin::DmaOut as u8)
    }
    pub fn get_intr(self) -> bool {
        bits::get_1(self.0, Pin::Intr as u8)
    }
    pub fn get_sc(self) -> u8 {
        bits::get_2(self.0, Pin::Sc0 as u8)
    }
    pub fn get_sc1(self) -> bool {
        bits::get_1(self.0, Pin::Sc1 as u8)
    }
    pub fn get_clear(self) -> bool {
        bits::get_1(self.0, Pin::Clear as u8)
    }
    pub fn get_wait(self) -> bool {
        bits::get_1(self.0, Pin::Wait as u8)
    }

    pub fn set_bus(&mut self, val: u8) {
        self.0 = bits::set_8(self.0, Pin::Bus0 as u8, val);
    }
    pub fn set_ma(&mut self, val: u8) {
        self.0 = bits::set_8(self.0, Pin::Ma0 as u8, val);
    }
    pub fn set_n(&mut self, val: u8) {
        self.0 = bits::set_3(self.0, Pin::N0 as u8, val);
    }
    pub fn set_q(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Q as u8, val);
    }
    pub fn set_mrd(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Mrd as u8, val);
    }
    pub fn set_mwr(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Mwr as u8, val);
    }
    pub fn set_tpa(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Tpa as u8, val);
    }
    pub fn set_tpb(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Tpb as u8, val);
    }
    pub fn set_sc(&mut self, val: u8) {
        self.0 = bits::set_2(self.0, Pin::Sc0 as u8, val);
    }
    pub fn set_dma_in(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::DmaIn as u8, val);
    }
    pub fn set_dma_out(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::DmaOut as u8, val);
    }
    pub fn set_intr(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Intr as u8, val);
    }
    pub fn set_ef1(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Ef1 as u8, val);
    }
    pub fn set_ef2(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Ef2 as u8, val);
    }
    pub fn set_ef3(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Ef3 as u8, val);
    }
    pub fn set_ef4(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Ef4 as u8, val);
    }
    pub fn set_clear(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Clear as u8, val);
    }
    pub fn set_wait(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Wait as u8, val);
    }
}
