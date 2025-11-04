use ratatui::text::{Line, Text};

use crate::{chips::cdp1802::Memory, instr::InstrSchema as _};

#[derive(Default, Clone, Copy)]
pub struct ListingWidget {}

impl ListingWidget {
    pub const fn height() -> u16 {
        13
    }
    pub const fn width() -> u16 {
        30
    }
    pub fn as_text(mem: &Memory, mut pc: u16) -> Text<'_> {
        let mut lines = Vec::new();
        for i in 0..Self::height() {
            let instr = mem.get_instr_at(pc);
            let (listing, size) = instr.map_or(("??".into(), 1), |i| (i.listing(), i.size()));
            let sigil = if i == 0 { ">" } else { " " };
            lines.push(Line::from(format!(" {sigil}{pc:04x} {listing}")));
            match pc.overflowing_add(size as u16) {
                (next, false) => pc = next,
                (_, true) => break,
            }
        }
        Text::from(lines)
    }
}
