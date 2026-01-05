mod terminal;

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Paragraph},
    DefaultTerminal, Frame,
};

use crate::terminal::Terminal;

#[derive(Default, PartialEq)]
enum Pane {
    #[default]
    Left,
    Right,
}

#[derive(Default, PartialEq, Clone, Copy)]
enum Mode {
    #[default]
    Normal,
    Insert,
}

struct Tab {
    focused: Pane,
    term: Option<Terminal>,
    last_term_size: (u16, u16),
}

impl Tab {
    fn new() -> Self {
        Self {
            focused: Pane::Left,
            term: None,
            last_term_size: (0, 0),
        }
    }

    fn ensure_terminal(&mut self, area: Rect) {
        let inner = inner_area(area);
        let size = (inner.width, inner.height);

        if self.term.is_none() && size.0 > 0 && size.1 > 0 {
            self.term = Terminal::new(size.0, size.1).ok();
            self.last_term_size = size;
        } else if self.last_term_size != size && size.0 > 0 && size.1 > 0 {
            if let Some(ref mut term) = self.term {
                term.resize(size.0, size.1);
                self.last_term_size = size;
            }
        }
    }
}

struct App {
    tabs: Vec<Tab>,
    active_tab: usize,
    mode: Mode,
}

impl App {
    fn new() -> Self {
        Self {
            tabs: vec![Tab::new()],
            active_tab: 0,
            mode: Mode::Normal,
        }
    }

    fn current_tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    fn current_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }
}

fn inner_area(area: Rect) -> Rect {
    Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut app = App::new();
    let terminal = ratatui::init();
    let result = run(&mut app, terminal);
    ratatui::restore();
    result
}

fn run(app: &mut App, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
    loop {
        terminal.draw(|frame| render(app, frame))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.mode {
                    Mode::Insert => {
                        // Escape exits insert mode
                        if key.code == KeyCode::Esc {
                            app.mode = Mode::Normal;
                            continue;
                        }

                        // Forward all other input to terminal
                        let tab = app.current_tab_mut();
                        if tab.focused == Pane::Left {
                            if let Some(ref mut term) = tab.term {
                                let bytes = key_to_bytes(&key);
                                if !bytes.is_empty() {
                                    let _ = term.write(&bytes);
                                }
                            }
                        }
                    }
                    Mode::Normal => {
                        match key.code {
                            // 'q' quits in normal mode
                            KeyCode::Char('q') => break,
                            // 't' creates a new tab
                            KeyCode::Char('t') => {
                                app.tabs.push(Tab::new());
                                app.active_tab = app.tabs.len() - 1;
                            }
                            // Number keys 1-9 switch tabs
                            KeyCode::Char(c @ '1'..='9') => {
                                let tab_idx = (c as usize) - ('1' as usize);
                                if tab_idx < app.tabs.len() {
                                    app.active_tab = tab_idx;
                                }
                            }
                            // 'i' enters insert mode (only when terminal pane focused)
                            KeyCode::Char('i') if app.current_tab().focused == Pane::Left => {
                                app.mode = Mode::Insert;
                            }
                            // Navigation: h or Left arrow to left pane
                            KeyCode::Char('h') | KeyCode::Left => {
                                app.current_tab_mut().focused = Pane::Left;
                            }
                            // Navigation: l or Right arrow to right pane
                            KeyCode::Char('l') | KeyCode::Right => {
                                app.current_tab_mut().focused = Pane::Right;
                            }
                            // Tab key still works for pane switching
                            KeyCode::Tab => {
                                let tab = app.current_tab_mut();
                                tab.focused = match tab.focused {
                                    Pane::Left => Pane::Right,
                                    Pane::Right => Pane::Left,
                                };
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn key_to_bytes(key: &event::KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+letter -> ASCII 1-26
                let ctrl = (c.to_ascii_lowercase() as u8).wrapping_sub(b'a' - 1);
                vec![ctrl]
            } else {
                c.to_string().into_bytes()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![127],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n) => match n {
            1 => b"\x1bOP".to_vec(),
            2 => b"\x1bOQ".to_vec(),
            3 => b"\x1bOR".to_vec(),
            4 => b"\x1bOS".to_vec(),
            5 => b"\x1b[15~".to_vec(),
            6 => b"\x1b[17~".to_vec(),
            7 => b"\x1b[18~".to_vec(),
            8 => b"\x1b[19~".to_vec(),
            9 => b"\x1b[20~".to_vec(),
            10 => b"\x1b[21~".to_vec(),
            11 => b"\x1b[23~".to_vec(),
            12 => b"\x1b[24~".to_vec(),
            _ => vec![],
        },
        _ => vec![],
    }
}

fn render(app: &mut App, frame: &mut Frame) {
    // Split into tab bar, main area, and status bar
    let [tab_area, main_area, status_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
            .areas(frame.area());

    // Render tab bar
    let mut tab_spans = Vec::new();
    for (i, _) in app.tabs.iter().enumerate() {
        let tab_num = i + 1;
        let style = if i == app.active_tab {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        };
        tab_spans.push(ratatui::text::Span::styled(format!(" {} ", tab_num), style));
        tab_spans.push(ratatui::text::Span::raw(" "));
    }
    let tab_bar = ratatui::text::Line::from(tab_spans);
    frame.render_widget(Paragraph::new(tab_bar), tab_area);

    // Split main area into left and right panes
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(main_area);

    // Ensure terminal is created/resized for current tab
    app.current_tab_mut().ensure_terminal(left);

    let tab = app.current_tab();
    let left_block = Block::bordered()
        .title("Terminal")
        .border_style(border_style(tab.focused == Pane::Left));
    let right_block = Block::bordered()
        .title("Right")
        .border_style(border_style(tab.focused == Pane::Right));

    // Render left pane with terminal
    frame.render_widget(left_block.clone(), left);
    if let Some(ref term) = tab.term {
        frame.render_widget(term.widget(), inner_area(left));
    }

    let right_pane = Paragraph::new("Pane 2").block(right_block);
    frame.render_widget(right_pane, right);

    // Render status bar
    let mode_text = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
    };
    let mode_style = match app.mode {
        Mode::Normal => Style::default().fg(Color::Black).bg(Color::Cyan),
        Mode::Insert => Style::default().fg(Color::Black).bg(Color::Green),
    };
    let status_bar = Paragraph::new(format!(" {} ", mode_text)).style(mode_style);
    frame.render_widget(status_bar, status_area);
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}
