#[derive(thiserror::Error, Debug, Clone, Copy)]
pub enum UartRxError {
    #[error("framing error")]
    Framing,
    #[error("parity error")]
    Parity,
    #[error("overrun error")]
    Overrun,
}

pub trait Uart: std::fmt::Debug {
    fn reset(&mut self);
    fn tick(&mut self);

    fn set_rx_pin(&mut self, val: bool);
    fn get_tx_pin(&self) -> bool;

    fn rx(&mut self) -> Result<u8, UartRxError>;
    fn tx(&mut self, byte: u8);

    fn rx_hold_cycles(&self) -> u32;
    fn tx_hold_cycles(&self) -> u32;

    fn is_rx_ready(&self) -> bool;
    fn is_tx_ready(&self) -> bool;
    fn is_tx_idle(&self) -> bool;
}
