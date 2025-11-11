use crate::uart::{Parity, UartMode};

use super::{Ay51013, Ay51013Pins};

macro_rules! tick_n {
    ($chip:expr, $pins:expr, $n:expr) => {
        for _ in 0..$n {
            $chip.tick($pins);
        }
    };
}

#[test]
fn test_tx_8n1() {
    let mut pins = Ay51013Pins::default();
    let mut chip = Ay51013::default();
    chip.configure(&mut pins, UartMode::new(8, None, 1));

    pins.set_db(0x53);
    pins.set_ds(false);
    chip.tick(&mut pins);

    // Release data strobe and validate start bit.
    pins.set_ds(true);
    chip.tick(&mut pins);
    pins.set_db(0xff);
    assert!(!pins.get_tbmt());
    assert!(!pins.get_so());
    tick_n!(chip, &mut pins, 16);

    // Expected data bits.
    let expect = [1, 1, 0, 0, 1, 0, 1, 0];
    for (ii, bit) in expect.into_iter().enumerate() {
        assert!(pins.get_tbmt());
        assert_eq!(pins.get_so() as u8, bit, "invalid bit {ii}");
        assert!(!pins.get_eoc());
        tick_n!(chip, &mut pins, 16);
    }

    // Stop bit
    assert!(pins.get_so());
    assert!(!pins.get_eoc());
    tick_n!(chip, &mut pins, 16);

    // End of character
    assert!(pins.get_so());
    assert!(pins.get_eoc());
}

#[test]
fn test_tx_5o2() {
    let mut pins = Ay51013Pins::default();
    let mut chip = Ay51013::default();
    chip.configure(&mut pins, UartMode::new(5, Parity::Odd, 2));

    pins.set_db(0x6c);
    pins.set_ds(false);
    chip.tick(&mut pins);

    // Release data strobe and validate start bit.
    pins.set_ds(true);
    chip.tick(&mut pins);
    pins.set_db(0xff);
    assert!(!pins.get_tbmt());
    assert!(!pins.get_so());
    tick_n!(chip, &mut pins, 16);

    // Expected data bits.
    let expect = [0, 0, 1, 1, 0];
    for (ii, bit) in expect.into_iter().enumerate() {
        assert!(pins.get_tbmt());
        assert_eq!(pins.get_so() as u8, bit, "invalid bit {ii}");
        assert!(!pins.get_eoc());
        tick_n!(chip, &mut pins, 16);
    }

    // Parity bit
    assert!(!pins.get_so());
    tick_n!(chip, &mut pins, 16);

    // Stop bit
    for _ in 0..2 {
        assert!(pins.get_so());
        assert!(!pins.get_eoc());
        tick_n!(chip, &mut pins, 16);
    }

    // End of character
    assert!(pins.get_so());
    assert!(pins.get_eoc());
}

#[test]
fn test_tx_5e1_with_buffering() {
    let mut pins = Ay51013Pins::default();
    let mut chip = Ay51013::default();
    chip.configure(&mut pins, UartMode::new(5, Parity::Even, 1));

    // Load character with odd parity.
    pins.set_db(0x8);
    pins.set_ds(false);
    chip.tick(&mut pins);

    // Release data strobe and validate start bit.
    pins.set_ds(true);
    chip.tick(&mut pins);
    pins.set_db(0xff);
    assert!(!pins.get_tbmt());
    assert!(!pins.get_so());

    // Wait for buffer to empty
    tick_n!(chip, &mut pins, 6);
    assert!(pins.get_tbmt());

    // Load next character with even parity
    pins.set_db(0xf);
    pins.set_ds(false);
    chip.tick(&mut pins);
    pins.set_ds(true);
    chip.tick(&mut pins);
    pins.set_db(0xff);

    // Remaining 8 ticks in the start bit.
    tick_n!(chip, &mut pins, 8);

    // Expected data bits for 0x8.
    let expect = [0, 0, 0, 1, 0];
    for (ii, bit) in expect.into_iter().enumerate() {
        assert_eq!(pins.get_so() as u8, bit, "invalid bit {ii}");
        assert!(!pins.get_eoc());
        tick_n!(chip, &mut pins, 16);
    }

    // Parity bit for 0x8.
    assert!(!pins.get_so());
    tick_n!(chip, &mut pins, 16);

    // Stop bit for 0x8
    assert!(pins.get_so());
    tick_n!(chip, &mut pins, 16);

    // EOC for 0x8, buffer still full, start bit for 0xf
    assert!(pins.get_eoc());
    assert!(!pins.get_tbmt());
    assert!(!pins.get_so());
    chip.tick(&mut pins);

    // EOC is reset in the next cycle.
    assert!(!pins.get_eoc());
    tick_n!(chip, &mut pins, 15);

    // Buffer no longer full.
    assert!(pins.get_tbmt());

    // Expected data bits for 0xf.
    let expect = [1, 1, 1, 1, 0];
    for (ii, bit) in expect.into_iter().enumerate() {
        assert_eq!(pins.get_so() as u8, bit, "invalid bit {ii}");
        tick_n!(chip, &mut pins, 16);
    }

    // Parity bit for 0xf.
    assert!(pins.get_so());
    tick_n!(chip, &mut pins, 16);

    // Stop bit for 0xf
    assert!(pins.get_so());
    tick_n!(chip, &mut pins, 16);

    // EOC for 0xf
    assert!(pins.get_eoc());
}

fn roundtrip(chip: &mut Ay51013, pins: &mut Ay51013Pins, cycle_count: u8, value: u8) {
    // Strobe in value
    pins.set_db(value);
    pins.set_ds(false);
    chip.tick(pins);
    pins.set_ds(true);

    // Wait for transmit
    for _ in 0..=cycle_count {
        let so = pins.get_so();
        pins.set_si(so);
        chip.tick(pins);
    }

    // Validate
    assert!(pins.get_eoc());
    assert!(pins.get_dav());
    assert!(!pins.get_pe());
    assert!(!pins.get_fe());
    assert!(!pins.get_or());
    assert_eq!(pins.get_rd(), value);
}

fn reset_dav(chip: &mut Ay51013, pins: &mut Ay51013Pins) {
    pins.set_rdav(false);
    chip.tick(pins);
    pins.set_rdav(true);
}

#[test]
fn test_roundtrip_8e1() {
    let mut pins = Ay51013Pins::default();
    let mut chip = Ay51013::default();
    chip.configure(&mut pins, UartMode::new(8, Parity::Even, 1));
    for val in 0..=255 {
        roundtrip(&mut chip, &mut pins, 176, dbg!(val));
        reset_dav(&mut chip, &mut pins);
    }
}

#[test]
fn test_rx_false_start() {
    let mut pins = Ay51013Pins::default();
    let mut chip = Ay51013::default();
    chip.configure(&mut pins, UartMode::new(5, None, 1));

    // Start not held long enough.
    pins.set_si(false);
    tick_n!(chip, &mut pins, 7);
    pins.set_si(true);
    tick_n!(chip, &mut pins, 3);

    // Start
    pins.set_si(false);
    tick_n!(chip, &mut pins, 16);

    // Data & stop
    pins.set_si(true);
    tick_n!(chip, &mut pins, (5 + 1) * 16);

    // Success.
    assert!(pins.get_dav());
    assert!(!pins.get_pe());
    assert!(!pins.get_fe());
    assert!(!pins.get_or());
    assert_eq!(pins.get_rd(), 0x1f);
}

#[test]
fn test_rx_framing_error() {
    let mut pins = Ay51013Pins::default();
    let mut chip = Ay51013::default();
    chip.configure(&mut pins, UartMode::new(5, None, 1));

    // Start, data, stop
    pins.set_si(false);
    tick_n!(chip, &mut pins, (1 + 5 + 1) * 16);

    // Validate that the framing error pin is set.
    assert!(pins.get_dav());
    assert!(!pins.get_pe());
    assert!(pins.get_fe());
    assert!(!pins.get_or());
    assert_eq!(pins.get_rd(), 0x00);
}

#[test]
fn test_rx_parity_error() {
    let mut pins = Ay51013Pins::default();
    let mut chip = Ay51013::default();
    chip.configure(&mut pins, UartMode::new(5, Parity::Even, 1));

    // Start, data, parity
    pins.set_si(false);
    tick_n!(chip, &mut pins, (1 + 5 + 1) * 16);

    // Stop
    pins.set_si(true);
    tick_n!(chip, &mut pins, 16);

    // Validate that the parity error pin is set.
    assert!(pins.get_dav());
    assert!(pins.get_pe());
    assert!(!pins.get_fe());
    assert!(!pins.get_or());
    assert_eq!(pins.get_rd(), 0x00);
}

#[test]
fn test_rx_overrun() {
    let mut pins = Ay51013Pins::default();
    let mut chip = Ay51013::default();
    chip.configure(&mut pins, UartMode::new(8, None, 1));

    // Do not set RDAV.
    roundtrip(&mut chip, &mut pins, 160, 0xaa);

    // Strobe in value
    pins.set_db(0x99);
    pins.set_ds(false);
    chip.tick(&mut pins);
    pins.set_ds(true);

    // Wait for transmit
    for _ in 0..=160 {
        let so = pins.get_so();
        pins.set_si(so);
        chip.tick(&mut pins);
    }

    // Validate that the overrun pin is set.
    assert!(pins.get_dav());
    assert!(!pins.get_pe());
    assert!(!pins.get_fe());
    assert!(pins.get_or());
    assert_eq!(pins.get_rd(), 0x99);

    // Reset DAV & roundtrip again.
    reset_dav(&mut chip, &mut pins);
    roundtrip(&mut chip, &mut pins, 160, 0x42);
}
