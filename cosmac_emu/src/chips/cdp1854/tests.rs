use super::{Cdp1854, Control, Parity, Pins, Status};

/// Loads a word into the tx holding register.
fn write_data(chip: &mut Cdp1854, pins: &mut u64, data: u8) {
    Pins::TBus0.set8(pins, data);
    Pins::Cs1.set3(pins, 0b101);
    Pins::Rsel.set(pins, false);
    Pins::RdWr.set(pins, false);
    chip.tick_tpb(pins);
    Pins::Cs1.set3(pins, 0b111);
}

/// Reads a word from the rx holding register.
fn read_data(chip: &mut Cdp1854, pins: &mut u64) -> u8 {
    Pins::Cs1.set3(pins, 0b101);
    Pins::Rsel.set(pins, false);
    Pins::RdWr.set(pins, true);
    chip.tick_tpb(pins);
    Pins::Cs1.set3(pins, 0b111);
    Pins::RBus0.get8(*pins)
}

/// Writes a control word to the device.
fn write_control(chip: &mut Cdp1854, pins: &mut u64, control: Control) {
    Pins::TBus0.set8(pins, control.into());
    Pins::Cs1.set3(pins, 0b101);
    Pins::Rsel.set(pins, true);
    Pins::RdWr.set(pins, false);
    chip.tick_tpb(pins);
    Pins::Cs1.set3(pins, 0b111);
}

/// Reads the status register
fn read_status(chip: &mut Cdp1854, pins: &mut u64) -> Status {
    Pins::Cs1.set3(pins, 0b101);
    Pins::Rsel.set(pins, true);
    Pins::RdWr.set(pins, true);
    chip.tick_tpb(pins);
    Pins::Cs1.set3(pins, 0b111);
    Pins::RBus0.get8(*pins).into()
}

/// Resets the device, loads the control word, and brings /CTS low.
///
/// The control word should not have TR set.
fn reset(chip: &mut Cdp1854, pins: &mut u64, control: Control) {
    Pins::Clear.set(pins, false);
    chip.tick_tpb(pins);
    Pins::Clear.set(pins, true);
    write_control(chip, pins, control);
    Pins::Cts.set(pins, false);
}

#[test]
fn test_control_roundtrip() {
    let ctrl = Control::new(8, None, 1).with_ie(true);
    let reg = u8::from(ctrl);
    assert_eq!(ctrl, Control::from(reg));

    let ctrl = Control::new(5, Some(Parity::Odd), 2)
        .with_tr(true)
        .with_tx_break(true);
    let reg = u8::from(ctrl);
    assert_eq!(ctrl, Control::from(reg));
}

#[test]
fn test_tx_8n1() {
    let mut pins = Pins::mask_all();
    let mut chip = Cdp1854::default();
    let ctrl = Control::new(8, None, 1).with_ie(true);
    reset(&mut chip, &mut pins, ctrl);
    assert!(!Pins::Thre.get(pins));
    assert!(Pins::Sdo.get(pins));
    assert!(Pins::Int.get(pins));
    assert!(Pins::Rts.get(pins));

    // Read status.
    let status = read_status(&mut chip, &mut pins);
    assert!(status.thre);
    assert!(status.tsre);
    assert!(Pins::Int.get(pins));

    // Load word.
    write_data(&mut chip, &mut pins, 0x53);
    assert!(Pins::Thre.get(pins));
    assert!(Pins::Sdo.get(pins));
    assert!(Pins::Int.get(pins));

    // Read status.
    let status = read_status(&mut chip, &mut pins);
    assert!(!status.thre);
    assert!(status.tsre);

    // After the first tick, SDO is high, and /THRE is high
    chip.tick_tclock(&mut pins);
    assert!(Pins::Thre.get(pins));
    assert!(Pins::Sdo.get(pins));
    assert!(Pins::Int.get(pins));

    // Read status.
    let status = read_status(&mut chip, &mut pins);
    assert!(!status.thre);
    assert!(!status.tsre);

    // Start bit
    for t in 0..16 {
        chip.tick_tclock(&mut pins);
        assert!(!Pins::Thre.get(pins), "start tick {t}");
        assert!(!Pins::Sdo.get(pins), "start tick {t}");
        assert!(Pins::Int.get(pins), "start tick {t}");
    }

    // Read status.
    let status = read_status(&mut chip, &mut pins);
    assert!(status.thre);
    assert!(!status.tsre);

    // Expected data bits.
    let expect = [1, 1, 0, 0, 1, 0, 1, 0];
    for (n, bit) in expect.into_iter().enumerate() {
        for t in 0..16 {
            chip.tick_tclock(&mut pins);
            assert!(!Pins::Thre.get(pins), "bit{n} tick {t}");
            assert_eq!(Pins::Sdo.get(pins) as u8, bit, "bit{n} tick {t}");
            assert!(Pins::Int.get(pins), "bit{n} tick {t}");
        }
    }

    // Stop bit, and interrupt on final tick.
    for t in 0..16 {
        chip.tick_tclock(&mut pins);
        assert!(!Pins::Thre.get(pins), "stop tick {t}");
        assert!(Pins::Sdo.get(pins), "stop tick {t}");
        assert_eq!(Pins::Int.get(pins), t != 15, "stop tick {t}");
    }

    // Reading status register clears interrupt.
    let status = read_status(&mut chip, &mut pins);
    assert!(status.thre);
    assert!(status.tsre);
    assert!(Pins::Int.get(pins));
}

#[test]
fn test_tx_5o2() {
    let mut pins = Pins::mask_all();
    let mut chip = Cdp1854::default();
    let ctrl = Control::new(5, Some(Parity::Odd), 2).with_ie(true);
    reset(&mut chip, &mut pins, ctrl);

    // Load word.
    write_data(&mut chip, &mut pins, 0x6c);

    // First tick to load TSRE
    chip.tick_tclock(&mut pins);

    // Start bit
    for t in 0..16 {
        chip.tick_tclock(&mut pins);
        assert!(!Pins::Sdo.get(pins), "start tick {t}");
        assert!(Pins::Int.get(pins), "start tick {t}");
    }

    // Expected data bits.
    let expect = [0, 0, 1, 1, 0];
    for (n, bit) in expect.into_iter().enumerate() {
        for t in 0..16 {
            chip.tick_tclock(&mut pins);
            assert_eq!(Pins::Sdo.get(pins) as u8, bit, "bit{n} tick {t}");
            assert!(Pins::Int.get(pins), "bit{n} tick {t}");
        }
    }

    // Parity bit.
    for t in 0..16 {
        chip.tick_tclock(&mut pins);
        assert!(!Pins::Sdo.get(pins), "parity tick {t}");
        assert!(Pins::Int.get(pins), "parity tick {t}");
    }

    // 1.5 stop bits, and interrupt on final tick.
    for t in 0..24 {
        chip.tick_tclock(&mut pins);
        assert!(Pins::Sdo.get(pins), "stop tick {t}");
        assert_eq!(Pins::Int.get(pins), t != 23, "stop tick {t}");
    }

    // Reading status register clears interrupt.
    let status = read_status(&mut chip, &mut pins);
    assert!(status.thre);
    assert!(status.tsre);
    assert!(Pins::Int.get(pins));
}

#[test]
fn test_tx_6e2_with_buffering() {
    let mut pins = Pins::mask_all();
    let mut chip = Cdp1854::default();
    let ctrl = Control::new(6, Some(Parity::Even), 2).with_ie(true);
    reset(&mut chip, &mut pins, ctrl);

    // Set TR for interrupts on THRE.
    write_control(&mut chip, &mut pins, ctrl.with_tr(true));
    assert!(!Pins::Rts.get(pins));
    assert!(Pins::Int.get(pins));

    // Load word with odd parity.
    write_data(&mut chip, &mut pins, 0x8);

    // After the first tick, SDO is high, and /THRE is high
    chip.tick_tclock(&mut pins);
    assert!(Pins::Thre.get(pins));
    assert!(Pins::Int.get(pins));

    // Start bit. Expect interrupt for THRE.
    for t in 0..16 {
        chip.tick_tclock(&mut pins);
        assert!(!Pins::Sdo.get(pins), "start tick {t}");
        assert!(!Pins::Thre.get(pins), "start tick {t}");
        assert!(!Pins::Int.get(pins), "start tick {t}");
    }

    // Load next word with odd parity.
    write_data(&mut chip, &mut pins, 0xf);
    assert!(Pins::Thre.get(pins));
    assert!(Pins::Int.get(pins));

    // Expected data bits for 0x8.
    let expect = [0, 0, 0, 1, 0, 0];
    for (n, bit) in expect.into_iter().enumerate() {
        for t in 0..16 {
            chip.tick_tclock(&mut pins);
            assert_eq!(Pins::Sdo.get(pins) as u8, bit, "bit{n} tick {t}");
            assert!(Pins::Int.get(pins), "bit{n} tick {t}");
        }
    }

    // Parity bit for 0x8.
    for t in 0..16 {
        chip.tick_tclock(&mut pins);
        assert!(!Pins::Sdo.get(pins), "parity tick {t}");
        assert!(Pins::Int.get(pins), "parity tick {t}");
    }

    // 2 stop bits.
    for t in 0..32 {
        chip.tick_tclock(&mut pins);
        assert!(Pins::Sdo.get(pins), "stop tick {t}");
        assert!(Pins::Int.get(pins), "stop tick {t}");
    }

    // Update control to clear TR.
    write_control(&mut chip, &mut pins, ctrl);

    // Start bit. No more interrupts for THRE.
    for t in 0..16 {
        chip.tick_tclock(&mut pins);
        assert!(!Pins::Sdo.get(pins), "start tick {t}");
        assert!(!Pins::Thre.get(pins), "start tick {t}");
        assert!(Pins::Int.get(pins), "start tick {t}");
    }

    // Expected data bits for 0xf.
    let expect = [1, 1, 1, 1, 0, 0];
    for (n, bit) in expect.into_iter().enumerate() {
        for t in 0..16 {
            chip.tick_tclock(&mut pins);
            assert_eq!(Pins::Sdo.get(pins) as u8, bit, "bit{n} tick {t}");
            assert!(Pins::Int.get(pins), "bit{n} tick {t}");
        }
    }

    // Parity bit for 0xf.
    for t in 0..16 {
        chip.tick_tclock(&mut pins);
        assert!(Pins::Sdo.get(pins), "parity tick {t}");
        assert!(Pins::Int.get(pins), "parity tick {t}");
    }

    // 2 stop bits, interrupt on final tick.
    for t in 0..32 {
        chip.tick_tclock(&mut pins);
        assert!(Pins::Sdo.get(pins), "stop tick {t}");
        assert_eq!(Pins::Int.get(pins), t != 31, "stop tick {t}");
    }
}

fn roundtrip(chip: &mut Cdp1854, pins: &mut u64, ticks: u64, value: u8) {
    write_data(chip, pins, value);
    for _ in 0..=ticks {
        chip.tick_tclock(pins);
        let bit = Pins::Sdo.get(*pins);
        Pins::Sdi.set(pins, bit);
        chip.tick_rclock(pins);
    }
    assert!(!Pins::Da.get(*pins));
    assert_eq!(read_data(chip, pins), value);
}

#[test]
fn test_roundtrip_8e1() {
    let mut pins = Pins::mask_all();
    let mut chip = Cdp1854::default();
    let ctrl = Control::new(8, Some(Parity::Even), 1).with_ie(true);
    reset(&mut chip, &mut pins, ctrl);
    for val in 0..=255 {
        roundtrip(&mut chip, &mut pins, ctrl.tx_ticks(), dbg!(val));
    }
}

#[test]
fn test_roundtrip_5n1() {
    let mut pins = Pins::mask_all();
    let mut chip = Cdp1854::default();
    let ctrl = Control::new(5, None, 1).with_ie(true);
    reset(&mut chip, &mut pins, ctrl);
    for val in 0..=0x1f {
        roundtrip(&mut chip, &mut pins, ctrl.tx_ticks(), dbg!(val));
    }
}

#[test]
fn test_pv_tx() {
    let (tx, rx) = flume::bounded(1);
    let mut pins = Pins::mask_all();
    let mut chip = Cdp1854::default().with_pv_tx(tx);
    let ctrl = Control::new(7, None, 1).with_ie(true);
    reset(&mut chip, &mut pins, ctrl);

    // Read status to clear interrupts.
    let status = read_status(&mut chip, &mut pins);
    assert!(status.thre);
    assert!(status.tsre);
    assert!(Pins::Int.get(pins));

    // Load a first word.
    write_data(&mut chip, &mut pins, 0x11);

    // First tick loads the shift register, second tick performs paravirt send. This causes /CTS to
    // go high, inhibiting future sends.
    chip.tick_tclock(&mut pins);
    assert!(rx.is_empty());
    assert!(!Pins::Cts.get(pins));
    chip.tick_tclock(&mut pins);
    assert!(!rx.is_empty());
    assert!(Pins::Cts.get(pins));

    // /CTS going high raises an interrupt. Read status to clear it.
    chip.tick_tpb(&mut pins);
    assert!(!Pins::Int.get(pins));
    let status = read_status(&mut chip, &mut pins);
    assert!(status.thre);
    assert!(!status.tsre);
    assert!(Pins::Int.get(pins));

    // Load a second word. The most significant bit will be masked since this is 7n1.
    assert!(!Pins::Thre.get(pins));
    write_data(&mut chip, &mut pins, 0xff);

    // Wait the remaining tx cycles for the first word.
    for _ in 0..ctrl.tx_ticks() {
        chip.tick_tclock(&mut pins);
    }

    // The holding register stays full, because /CTS is blocking sends.
    for _ in 0..10 {
        chip.tick_tclock(&mut pins);
        assert!(Pins::Thre.get(pins));
    }

    // Drain the queue, and /CTS will drop low.
    assert_eq!(rx.try_recv().unwrap(), 0x11);
    chip.tick_tpb(&mut pins);
    assert!(!Pins::Cts.get(pins));

    // Finish sending the second word.
    for _ in 0..=ctrl.tx_ticks() {
        chip.tick_tclock(&mut pins);
    }
    assert_eq!(rx.try_recv().unwrap(), 0x7f);

    let status = read_status(&mut chip, &mut pins);
    assert!(status.thre);
    assert!(status.tsre);
}

#[test]
fn test_pv_rx() {
    let (tx, rx) = flume::bounded(1);
    let mut pins = Pins::mask_all();
    let mut chip = Cdp1854::default().with_pv_rx(rx);
    let ctrl = Control::new(7, None, 1).with_ie(true);
    reset(&mut chip, &mut pins, ctrl);

    // Read status to clear interrupts.
    let status = read_status(&mut chip, &mut pins);
    assert!(!status.da);
    assert!(Pins::Int.get(pins));

    // Inject a word.
    tx.try_send(0x11).unwrap();

    // Wait 2 ticks for WaitSdiHigh and WaitSdiLow, then a full receive cycle.
    for i in 0..(2 + ctrl.rx_ticks()) {
        assert!(Pins::Int.get(pins));
        chip.tick_rclock(&mut pins);

        // Inject a second word. The most significant bit will be masked since this is 7n1.
        if i == 2 {
            tx.try_send(0xff).unwrap();
        }
    }

    // First word received.
    assert!(!Pins::Int.get(pins));

    // Read status to validate data available.
    let status = read_status(&mut chip, &mut pins);
    assert!(status.da);
    assert_eq!(read_data(&mut chip, &mut pins), 0x11);
    assert!(Pins::Int.get(pins));

    // Wait 1 tick for WaitSdiLow, then a full receive cycle.
    for _ in 0..=ctrl.rx_ticks() {
        assert!(Pins::Int.get(pins));
        chip.tick_rclock(&mut pins);
    }

    // Second word received.
    assert!(!Pins::Int.get(pins));

    // Read status to validate data available.
    let status = read_status(&mut chip, &mut pins);
    assert!(status.da);
    assert_eq!(read_data(&mut chip, &mut pins), 0x7f);
    assert!(Pins::Int.get(pins));
}
