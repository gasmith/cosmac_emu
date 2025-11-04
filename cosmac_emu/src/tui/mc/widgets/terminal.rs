use std::collections::VecDeque;

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use vt100::Parser;

/// Code Page 437 to Unicode mapping table.
/// Each index corresponds to the byte value 0x00–0xFF.
/// Source: IBM Code Page 437 / Unicode 15.1 mapping.
pub const CP437_EXTENDED_TO_UNICODE: [char; 128] = [
    '\u{00C7}', '\u{00FC}', '\u{00E9}', '\u{00E2}', '\u{00E4}', '\u{00E0}', '\u{00E5}', '\u{00E7}',
    '\u{00EA}', '\u{00EB}', '\u{00E8}', '\u{00EF}', '\u{00EE}', '\u{00EC}', '\u{00C4}', '\u{00C5}',
    '\u{00C9}', '\u{00E6}', '\u{00C6}', '\u{00F4}', '\u{00F6}', '\u{00F2}', '\u{00FB}', '\u{00F9}',
    '\u{00FF}', '\u{00D6}', '\u{00DC}', '\u{00A2}', '\u{00A3}', '\u{00A5}', '\u{20A7}', '\u{0192}',
    '\u{00E1}', '\u{00ED}', '\u{00F3}', '\u{00FA}', '\u{00F1}', '\u{00D1}', '\u{00AA}', '\u{00BA}',
    '\u{00BF}', '\u{2310}', '\u{00AC}', '\u{00BD}', '\u{00BC}', '\u{00A1}', '\u{00AB}', '\u{00BB}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}',
    '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255D}', '\u{255C}', '\u{255B}', '\u{2510}',
    '\u{2514}', '\u{2534}', '\u{252C}', '\u{251C}', '\u{2500}', '\u{253C}', '\u{255E}', '\u{255F}',
    '\u{255A}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256C}', '\u{2567}',
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256B}',
    '\u{256A}', '\u{2518}', '\u{250C}', '\u{2588}', '\u{2584}', '\u{258C}', '\u{2590}', '\u{2580}',
    '\u{03B1}', '\u{00DF}', '\u{0393}', '\u{03C0}', '\u{03A3}', '\u{03C3}', '\u{00B5}', '\u{03C4}',
    '\u{03A6}', '\u{0398}', '\u{03A9}', '\u{03B4}', '\u{221E}', '\u{03C6}', '\u{03B5}', '\u{2229}',
    '\u{2261}', '\u{00B1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00F7}', '\u{2248}',
    '\u{00B0}', '\u{2219}', '\u{00B7}', '\u{221A}', '\u{207F}', '\u{00B2}', '\u{25A0}', '\u{00A0}',
];

fn to_ratatui_color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
        vt100::Color::Default => Color::Reset,
    }
}

fn cp437_to_unicode(data: u8) -> char {
    debug_assert!(data >= 0x80);
    CP437_EXTENDED_TO_UNICODE[(data & 0x7f) as usize]
}

enum KeySym {
    One(u8),
    Multi(&'static [u8]),
}
impl TryFrom<KeyEvent> for KeySym {
    type Error = ();
    fn try_from(key: KeyEvent) -> Result<Self, Self::Error> {
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    let upper = c.to_ascii_uppercase();
                    let ctrl = (upper as u8) & 0x1F; // Ctrl+A → 0x01
                    Ok(KeySym::One(ctrl))
                } else {
                    Ok(KeySym::One(c as u8))
                }
            }
            KeyCode::Enter => Ok(KeySym::One(b'\r')),
            KeyCode::Tab => Ok(KeySym::One(b'\t')),
            KeyCode::Backspace => Ok(KeySym::One(0x08)),
            KeyCode::Left => Ok(KeySym::Multi(b"\x1b[D")),
            KeyCode::Right => Ok(KeySym::Multi(b"\x1b[C")),
            KeyCode::Up => Ok(KeySym::Multi(b"\x1b[A")),
            KeyCode::Down => Ok(KeySym::Multi(b"\x1b[B")),
            KeyCode::Home => Ok(KeySym::Multi(b"\x1b[H")),
            KeyCode::End => Ok(KeySym::Multi(b"\x1b[F")),
            KeyCode::Delete => Ok(KeySym::Multi(b"\x1b[3~")),
            _ => Err(()),
        }
    }
}

pub struct TerminalWidget {
    clean: bool,
    term: Parser,
    input_buffer: VecDeque<u8>,
}

fn new_term() -> Parser {
    Parser::new(25, 80, 0)
}

impl Default for TerminalWidget {
    fn default() -> Self {
        Self {
            clean: true,
            term: new_term(),
            input_buffer: VecDeque::default(),
        }
    }
}
impl TerminalWidget {
    pub const fn width() -> u16 {
        80
    }
    pub const fn height() -> u16 {
        25
    }

    pub fn as_text(&self) -> Text<'_> {
        let screen = self.term.screen();
        let mut lines = Vec::new();
        let (rows, cols) = screen.size();
        for row in 0..rows {
            let mut spans = Vec::new();
            for col in 0..cols {
                if let Some(cell) = screen.cell(row, col) {
                    assert!(!cell.is_wide());
                    let ch = if cell.has_contents() {
                        cell.contents()
                    } else {
                        " ".to_string()
                    };
                    let mut style = Style::default()
                        .fg(to_ratatui_color(cell.fgcolor()))
                        .bg(to_ratatui_color(cell.bgcolor()));
                    if cell.bold() {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if cell.inverse() {
                        style = style.add_modifier(Modifier::REVERSED);
                    }
                    spans.push(Span::styled(ch, style));
                }
            }
            lines.push(Line::from(spans));
        }
        Text::from(lines)
    }

    /// Resets the terminal.
    pub fn reset(&mut self) {
        self.input_buffer.truncate(0);
        if !self.clean {
            self.term = new_term();
            self.clean = true;
        }
    }

    /// Outputs a byte received from the UART.
    pub fn handle_output(&mut self, byte: u8) {
        self.clean = false;
        if byte < 0x80 {
            self.term.process(&[byte]);
        } else {
            let mut buf = [0u8; 4];
            let n = cp437_to_unicode(byte).encode_utf8(&mut buf).len();
            self.term.process(&buf[..n]);
        }
    }

    /// Handles an input event.
    ///
    /// Appends keysyms to the input buffer, which should be flushed using
    /// [`Self::pop_input_buffer`], when the device is waiting for a write.
    pub fn handle_input(&mut self, key: KeyEvent) {
        match KeySym::try_from(key) {
            Ok(KeySym::One(sym)) => self.input_buffer.push_back(sym),
            Ok(KeySym::Multi(syms)) => self.input_buffer.extend(syms),
            Err(()) => (),
        }
    }

    /// Pops the next keysym off the write buffer.
    pub fn pop_input_buffer(&mut self) -> Option<u8> {
        self.input_buffer.pop_front()
    }
}
