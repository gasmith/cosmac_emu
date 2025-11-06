use std::{
    collections::VecDeque,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use chrono::Local;
use color_eyre::Result;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEvent},
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Text},
    widgets::{Block, Borders, Widget},
};

use crate::systems::mc::{MembershipCard, Status};

mod widgets;
use widgets::{FrontPanelWidget, ListingWidget, RegisterWidget, TerminalWidget};

#[derive(Default)]
struct LogBuffer(Mutex<VecDeque<String>>);
static LOG_BUFFER: LazyLock<LogBuffer> = LazyLock::new(LogBuffer::default);
impl log::Log for LogBuffer {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let mut records = self.0.lock().unwrap();
            let now = Local::now().naive_local();
            let msg = format!("{now} {} {}\n", record.level(), record.args());
            if records.len() >= 1000 {
                records.pop_front();
            }
            records.push_back(msg);
        }
    }

    fn flush(&self) {}
}
impl Widget for &'static LogBuffer {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let records = self.0.lock().unwrap();
        let lines: Vec<_> = records
            .iter()
            .rev()
            .map(|s| Line::from(s.as_str()))
            .take(area.height as usize)
            .collect();
        Text::from(lines).render(area, buf)
    }
}

/// Redraw at no greater than 30fps. Handle events at no less than 30Hz.
const UI_FREQ: Duration = Duration::from_millis(1000 / 30);

#[derive(Default, Clone, Copy)]
enum Focus {
    FrontPanel,
    #[default]
    Terminal,
}
impl Focus {
    fn next(self) -> Self {
        match self {
            Focus::FrontPanel => Focus::Terminal,
            Focus::Terminal => Focus::FrontPanel,
        }
    }
}

pub struct MembershipCardTui {
    mc: MembershipCard,
    focus: Focus,
    front_panel: FrontPanelWidget,
    terminal: TerminalWidget,
    ui_draw_at: Instant,
    ui_poll_at: Instant,
}

#[derive(Debug, Clone, Copy)]
enum McPollStatus {
    Active,
    Idle,
}

#[derive(Debug, Clone, Copy)]
enum UiPollStatus {
    Timeout,
    Handled,
    Noop,
    Exit,
}

impl MembershipCardTui {
    pub fn new(mc: MembershipCard) -> Self {
        Self {
            mc,
            focus: Default::default(),
            front_panel: Default::default(),
            terminal: Default::default(),
            ui_draw_at: Instant::now(),
            ui_poll_at: Instant::now(),
        }
    }

    pub fn draw(&self, f: &mut Frame) {
        let area = f.area();

        // Top area (terminal, front panel, registers) and bottom (log buffer)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(TerminalWidget::height() + 2),
                Constraint::Fill(1),
            ])
            .split(area);

        // Split top area into left (terminal) and right (front panel, registers)
        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(TerminalWidget::width() + 2),
                Constraint::Length(
                    FrontPanelWidget::width()
                        .max(RegisterWidget::width())
                        .max(ListingWidget::width())
                        + 2,
                ),
            ])
            .split(chunks[0]);

        // Front panel, registers
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(FrontPanelWidget::height() + 2),
                Constraint::Length(RegisterWidget::height() + 2),
                Constraint::Length(ListingWidget::height() + 2),
            ])
            .split(top_chunks[1]);

        // Terminal area
        let term_chunk = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(TerminalWidget::width() + 2)].as_ref())
            .split(top_chunks[0])[0];

        self.render_block(
            f,
            "Front Panel",
            right_chunks[0],
            FrontPanelWidget::width(),
            FrontPanelWidget::height(),
            self.front_panel.as_text(self.mc.front_panel()),
        );
        self.render_block(
            f,
            "Registers",
            right_chunks[1],
            RegisterWidget::width(),
            RegisterWidget::height(),
            RegisterWidget::as_text(self.mc.cpu()),
        );
        self.render_block(
            f,
            "Listing",
            right_chunks[2],
            ListingWidget::width(),
            ListingWidget::height(),
            ListingWidget::as_text(self.mc.memory(), self.mc.last_pc()),
        );
        self.render_block(
            f,
            "Terminal",
            term_chunk,
            TerminalWidget::width(),
            TerminalWidget::height(),
            self.terminal.as_text(),
        );
        f.render_widget(&*LOG_BUFFER, chunks[1]);
    }

    fn render_block(
        &self,
        f: &mut Frame,
        title: &str,
        rect: Rect,
        width: u16,
        height: u16,
        w: impl Widget,
    ) {
        let block = Block::default().borders(Borders::ALL).title(title);
        let inner = Rect {
            x: rect.x + 1,
            y: rect.y + 1,
            width,
            height,
        };
        f.render_widget(block, rect);
        f.render_widget(w, inner);
    }

    pub fn run(mut self) -> Result<()> {
        let mut terminal = ratatui::init();
        log::set_logger(&*LOG_BUFFER).unwrap();
        log::set_max_level(log::LevelFilter::Info);
        let result = self.run_loop(&mut terminal);
        ratatui::restore();
        result
    }

    fn run_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut need_redraw = true;
        let mut need_input = false;
        loop {
            // Poll the device. If we did work, redraw. Otherwise, if no redraw is pending, block
            // on UI events indefinitely.
            match self.poll_mc()? {
                McPollStatus::Active => need_redraw = true,
                McPollStatus::Idle => need_input = true,
            }

            // Redraw the UI periodically, when requested.
            if need_redraw && self.ui_draw_at <= Instant::now() {
                need_redraw = false;
                self.ui_draw_at = Instant::now() + UI_FREQ;
                self.front_panel
                    .set_focus(matches!(self.focus, Focus::FrontPanel));
                terminal.draw(|f| self.draw(f))?;
            }

            // Poll the UI, if input is needed, and periodically.
            if need_input || self.ui_poll_at <= Instant::now() {
                // If we're waiting for input, and there's no redraw pending, wait indefinitely.
                let duration = if need_input && !need_redraw {
                    Duration::from_secs(60)
                } else {
                    Duration::default()
                };
                self.ui_poll_at = Instant::now() + UI_FREQ;
                match self.poll_ui(duration)? {
                    UiPollStatus::Timeout => (),
                    UiPollStatus::Handled => {
                        need_input = false;
                        need_redraw = true;
                    }
                    UiPollStatus::Noop => need_input = false,
                    UiPollStatus::Exit => return Ok(()),
                }
            }
        }
    }

    fn poll_mc(&mut self) -> Result<McPollStatus> {
        let status = match self.mc.poll() {
            Some(Status::UartRead) => {
                match self.mc.uart_read() {
                    Ok(byte) => self.terminal.handle_output(byte),
                    Err(err) => log::warn!("uart read: {err}"),
                }
                McPollStatus::Active
            }
            Some(Status::UartWrite) => {
                // If there's pending data in the buffer, send it. Otherwise, fall back to the
                // TUI poll loop to wait for user input.
                if let Some(byte) = self.terminal.pop_input_buffer() {
                    self.mc.uart_write(byte);
                    McPollStatus::Active
                } else {
                    McPollStatus::Idle
                }
            }
            Some(Status::Tick) => {
                self.mc.tick();
                McPollStatus::Active
            }
            None => McPollStatus::Idle,
        };
        Ok(status)
    }

    fn poll_ui(&mut self, duration: Duration) -> Result<UiPollStatus> {
        if !event::poll(duration)? {
            return Ok(UiPollStatus::Timeout);
        }
        let event = event::read()?;
        let status = match event {
            Event::Key(key) if matches!(key.code, KeyCode::Esc) => UiPollStatus::Exit,
            Event::Key(key) if matches!(key.code, KeyCode::Tab) => {
                self.focus = self.focus.next();
                UiPollStatus::Handled
            }
            Event::Key(key) => {
                self.handle_key(key);
                UiPollStatus::Handled
            }
            _ => UiPollStatus::Noop,
        };
        Ok(status)
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match self.focus {
            Focus::FrontPanel => {
                self.front_panel
                    .handle_input(self.mc.front_panel_mut(), key);
                let fp = self.mc.front_panel();
                if fp.clear {
                    self.terminal.reset();
                }
            }
            Focus::Terminal => self.terminal.handle_input(key),
        }
    }
}
