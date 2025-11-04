//! A ratatui widget for the membership card front panel.
//!
//! ```text
//!  output ● ○ ○ ● ● ○ ● ○ 9A
//!   input ● ● ○ ○ ○ ● ○ ○ C4
//! in ○  wait ○  clr ○  read ○
//! ```

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    style::{Color, Modifier, Stylize},
    text::{Line, Span, Text},
};

use crate::systems::mc::FrontPanel;

pub struct FrontPanelWidget {
    focus: bool,
    selected_row: u8, // 0=output, 1=input, 2=control
    selected_col: u8,
}

impl Default for FrontPanelWidget {
    fn default() -> Self {
        Self {
            focus: false,
            selected_row: 1,
            selected_col: 0,
        }
    }
}
impl FrontPanelWidget {
    pub const fn height() -> u16 {
        3
    }
    pub const fn width() -> u16 {
        29
    }

    fn is_selected(&self, row: u8, col: u8) -> bool {
        self.focus && self.selected_row == row && self.selected_col == col
    }

    pub fn set_focus(&mut self, focus: bool) {
        self.focus = focus;
    }

    pub fn handle_input(&mut self, fp: &mut FrontPanel, key: KeyEvent) {
        match key.code {
            KeyCode::Left => {
                if self.selected_col > 0 {
                    self.selected_col -= 1;
                }
            }
            KeyCode::Right => {
                let max_col = match self.selected_row {
                    1 => 7,
                    2 => 3,
                    _ => unreachable!(),
                };
                if self.selected_col < max_col {
                    self.selected_col += 1;
                }
            }
            KeyCode::Up => {
                if self.selected_row > 1 {
                    self.selected_row -= 1;
                    self.selected_col = 0;
                }
            }
            KeyCode::Down => {
                if self.selected_row < 2 {
                    self.selected_row += 1;
                    self.selected_col = 0;
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter => match self.selected_row {
                1 => fp.inp_buffer ^= 1 << (7 - self.selected_col),
                2 => {
                    let v = match self.selected_col {
                        0 => &mut fp.inp,
                        1 => &mut fp.wait,
                        2 => &mut fp.clear,
                        3 => &mut fp.read,
                        _ => unreachable!(),
                    };
                    *v = !*v;
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn as_text(&self, fp: &FrontPanel) -> Text<'_> {
        let mut lines = Vec::new();

        // Row 1: Output
        {
            let value = fp.out_buffer;
            let bits: Vec<_> = (0..8)
                .flat_map(|i| {
                    [
                        Span::from(get_bit_symbol(value, 7 - i)).fg(Color::Red),
                        Span::from(" "),
                    ]
                })
                .collect();
            let mut spans = vec![Span::from("  output ")];
            spans.extend(bits);
            spans.push(Span::raw(format!("{value:02X}")));
            lines.push(Line::from(spans));
        }

        // Row 2: Input
        {
            let value = fp.inp_buffer;
            let bits: Vec<_> = (0..8)
                .flat_map(|i| {
                    let bit = get_bit_symbol(value, 7 - i);
                    let span = if self.is_selected(1, i) {
                        Span::styled(bit, Modifier::REVERSED)
                    } else {
                        Span::from(bit)
                    };
                    [span, Span::from(" ")]
                })
                .collect();
            let mut spans = vec![Span::from("   input ")];
            spans.extend(bits);
            spans.push(Span::raw(format!("{value:02X}")));
            lines.push(Line::from(spans));
        }

        // Row 3: Controls
        {
            let mut spans: Vec<_> = [
                (0, "in", fp.inp),
                (1, "wait", fp.wait),
                (2, "clr", fp.clear),
                (3, "read", fp.read),
            ]
            .into_iter()
            .flat_map(|(i, label, value)| {
                let symbol = get_symbol(value);
                let text = format!("{label} {symbol}");
                let span = if self.is_selected(2, i) {
                    Span::styled(text, Modifier::REVERSED)
                } else {
                    Span::from(text)
                };
                [Span::from("  "), span]
            })
            .skip(1)
            .collect();
            spans.insert(0, Span::from(" "));
            lines.push(Line::from(spans));
        }

        Text::from(lines)
    }
}

fn get_bit_symbol(value: u8, n: u8) -> &'static str {
    get_symbol((value & (1 << n)) > 0)
}

fn get_symbol(value: bool) -> &'static str {
    if value { "●" } else { "○" }
}
