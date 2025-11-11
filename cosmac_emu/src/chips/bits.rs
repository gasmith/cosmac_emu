pub fn set_1(pins: u64, bit: u8, val: bool) -> u64 {
    let mask = 1 << bit;
    let val = (val as u64) << bit;
    (pins & !mask) | val
}
#[inline(always)]
fn set_n(pins: u64, lsb: u8, mask: u64, val: u8) -> u64 {
    let mask = mask << lsb;
    let val = (val as u64) << lsb;
    (pins & !mask) | val
}
pub fn set_2(pins: u64, lsb: u8, val: u8) -> u64 {
    set_n(pins, lsb, 0x3, val)
}
pub fn set_3(pins: u64, lsb: u8, val: u8) -> u64 {
    set_n(pins, lsb, 0x7, val)
}
pub fn set_4(pins: u64, lsb: u8, val: u8) -> u64 {
    set_n(pins, lsb, 0xf, val)
}
pub fn set_8(pins: u64, lsb: u8, val: u8) -> u64 {
    set_n(pins, lsb, 0xff, val)
}

pub fn get_1(pins: u64, bit: u8) -> bool {
    (pins & (1 << bit)) > 0
}
pub fn get_2(pins: u64, lsb: u8) -> u8 {
    ((pins >> lsb) & 0x3) as u8
}
pub fn get_3(pins: u64, lsb: u8) -> u8 {
    ((pins >> lsb) & 0x7) as u8
}
pub fn get_4(pins: u64, lsb: u8) -> u8 {
    ((pins >> lsb) & 0xf) as u8
}
pub fn get_8(pins: u64, lsb: u8) -> u8 {
    ((pins >> lsb) & 0xff) as u8
}
