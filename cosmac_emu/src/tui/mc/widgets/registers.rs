use std::fmt::Write as _;

use ratatui::text::{Line, Text};

use crate::chips::cdp1802::Cdp1802;

#[derive(Default, Clone, Copy)]
pub struct RegisterWidget {}

impl RegisterWidget {
    pub const fn height() -> u16 {
        5
    }
    pub const fn width() -> u16 {
        29
    }
    pub fn as_text(chip: &Cdp1802) -> Text<'_> {
        let mut lines = Vec::new();

        let instr = chip.i << 4 | chip.n;
        lines.push(Line::from(format!(
            " d={d:02x}.{df} p={p:x} x={x:x} t={t:04x} in={instr:02x}",
            d = chip.d,
            df = u8::from(chip.df),
            p = chip.p,
            x = chip.x,
            t = chip.t,
        )));

        let mut row = String::new();
        for (n, r) in chip.r.iter().enumerate() {
            write!(row, " {n:x}={r:04x}").unwrap();
            if n % 4 == 3 {
                lines.push(Line::from(row));
                row = String::new();
            }
        }

        Text::from(lines)
    }
}
