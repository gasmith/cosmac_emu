use crate::chips::bits;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum Pin {
    // out
    Rd1,
    Rd2,
    Rd3,
    Rd4,
    Rd5,
    Rd6,
    Rd7,
    Rd8,
    Dav,
    Pe,
    Fe,
    Or,
    Tbmt,
    Eoc,
    So,

    // in
    Si,
    Db1,
    Db2,
    Db3,
    Db4,
    Db5,
    Db6,
    Db7,
    Db8,
    Tsb,
    Eps,
    Np,
    Nb1,
    Nb2,
    Cs,
    Ds,
    Rde,
    Swe,
    Rdav,
    Xr,
}

#[derive(Clone, Copy)]
pub struct Ay51013Pins(pub u64);
impl Default for Ay51013Pins {
    fn default() -> Self {
        Self(u64::MAX & Self::mask_all())
    }
}
impl std::fmt::Debug for Ay51013Pins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Ay51013Pins")
            .field(&format!("{:034b}", self.0))
            .finish()
    }
}
impl Ay51013Pins {
    pub const fn mask_all() -> u64 {
        (1 << ((Pin::Xr as u8) + 1)) - 1
    }

    /// Read buffer.
    pub fn get_rd(self) -> u8 {
        bits::get_8(self.0, Pin::Rd1 as u8)
    }
    pub fn set_rd(&mut self, val: u8) {
        self.0 = bits::set_8(self.0, Pin::Rd1 as u8, val);
    }

    /// Data available.
    pub fn get_dav(self) -> bool {
        bits::get_1(self.0, Pin::Dav as u8)
    }
    pub fn set_dav(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Dav as u8, val);
    }

    /// Parity error.
    pub fn get_pe(self) -> bool {
        bits::get_1(self.0, Pin::Pe as u8)
    }
    pub fn set_pe(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Pe as u8, val);
    }

    /// Framing error.
    pub fn get_fe(self) -> bool {
        bits::get_1(self.0, Pin::Fe as u8)
    }
    pub fn set_fe(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Fe as u8, val);
    }

    /// Overrun.
    pub fn get_or(self) -> bool {
        bits::get_1(self.0, Pin::Or as u8)
    }
    pub fn set_or(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Or as u8, val);
    }

    /// Transmit buffer empty.
    pub fn get_tbmt(self) -> bool {
        bits::get_1(self.0, Pin::Tbmt as u8)
    }
    pub fn set_tbmt(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Tbmt as u8, val);
    }

    /// Transmit end of character.
    #[cfg(test)]
    pub fn get_eoc(self) -> bool {
        bits::get_1(self.0, Pin::Eoc as u8)
    }
    pub fn set_eoc(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Eoc as u8, val);
    }

    /// Transmit output.
    pub fn get_so(self) -> bool {
        bits::get_1(self.0, Pin::So as u8)
    }
    pub fn set_so(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::So as u8, val);
    }

    /// Receive input.
    pub fn get_si(self) -> bool {
        bits::get_1(self.0, Pin::Si as u8)
    }
    pub fn set_si(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Si as u8, val);
    }

    pub fn get_db(self) -> u8 {
        bits::get_8(self.0, Pin::Db1 as u8)
    }
    pub fn set_db(&mut self, val: u8) {
        self.0 = bits::set_8(self.0, Pin::Db1 as u8, val);
    }

    /// Number of stop bits per character.
    pub fn get_tsb(self) -> bool {
        bits::get_1(self.0, Pin::Tsb as u8)
    }
    pub fn set_tsb(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Tsb as u8, val);
    }

    /// Parity select.
    pub fn get_eps(self) -> bool {
        bits::get_1(self.0, Pin::Eps as u8)
    }
    pub fn set_eps(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Eps as u8, val);
    }

    /// No parity.
    pub fn get_np(self) -> bool {
        bits::get_1(self.0, Pin::Np as u8)
    }
    pub fn set_np(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Np as u8, val);
    }

    /// Number of bits per character.
    pub fn get_nb(self) -> u8 {
        bits::get_2(self.0, Pin::Nb1 as u8)
    }
    pub fn set_nb(&mut self, val: u8) {
        self.0 = bits::set_2(self.0, Pin::Nb1 as u8, val);
    }

    /// Configuration strobe.
    pub fn get_cs(self) -> bool {
        bits::get_1(self.0, Pin::Cs as u8)
    }
    pub fn set_cs(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Cs as u8, val);
    }

    /// Data strobe (negated).
    pub fn get_ds(self) -> bool {
        bits::get_1(self.0, Pin::Ds as u8)
    }
    pub fn set_ds(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Ds as u8, val);
    }

    /// Read data bits enable (negated).
    ///
    /// When set to 0, the RD1..RD8 lines are active.
    pub fn get_rde(self) -> bool {
        bits::get_1(self.0, Pin::Rde as u8)
    }
    pub fn set_rde(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Rde as u8, val);
    }

    /// Status word enable (negated).
    ///
    /// When set to 0, puts PE, FE, OR, DAV, TBMT on the output lines.
    pub fn get_swe(self) -> bool {
        bits::get_1(self.0, Pin::Swe as u8)
    }
    pub fn set_swe(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Swe as u8, val);
    }

    /// Reset DAV (negated).
    ///
    /// When set to 0, resets the DAV line.
    pub fn get_rdav(self) -> bool {
        bits::get_1(self.0, Pin::Rdav as u8)
    }
    pub fn set_rdav(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Rdav as u8, val);
    }

    /// External reset.
    pub fn get_xr(self) -> bool {
        bits::get_1(self.0, Pin::Xr as u8)
    }
    pub fn set_xr(&mut self, val: bool) {
        self.0 = bits::set_1(self.0, Pin::Xr as u8, val);
    }
}
