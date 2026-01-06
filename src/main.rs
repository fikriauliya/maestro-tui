mod app;
mod input;
mod pty;
mod terminal;
mod ui;
mod worktree;

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph},
    DefaultTerminal, Frame,
};

use crate::app::{handle_key_insert, handle_key_normal, inner_area, App, Command, Mode, Pane};
use crate::terminal::Terminal;
use crate::ui::{active_tab_style, border_style, inactive_tab_style, mode_style};

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

                let cmd = match app.mode {
                    Mode::Insert => handle_key_insert(&key),
                    Mode::Normal => handle_key_normal(&key),
                };

                if let Some(cmd) = cmd {
                    if matches!(cmd, Command::Quit) {
                        break;
                    }
                    app.execute(cmd);
                }
            }
        }
    }
    Ok(())
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
            active_tab_style()
        } else {
            inactive_tab_style()
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

    // Handle terminal creation and resize for current tab
    let tab = app.current_tab_mut();

    if tab.needs_left_terminal(left) {
        let inner = inner_area(left);
        if let Ok(term) = Terminal::new(inner.width, inner.height) {
            tab.set_left_terminal(term, left);
        }
    } else if let Some((cols, rows)) = tab.needs_left_resize(left) {
        if let Some(ref mut term) = tab.left_term {
            term.resize(cols, rows);
            tab.update_left_size(left);
        }
    }

    if tab.needs_right_terminal(right) {
        let inner = inner_area(right);
        if let Ok(term) = Terminal::with_command(inner.width, inner.height, "claude", &[]) {
            tab.set_right_terminal(term, right);
        }
    } else if let Some((cols, rows)) = tab.needs_right_resize(right) {
        if let Some(ref mut term) = tab.right_term {
            term.resize(cols, rows);
            tab.update_right_size(right);
        }
    }

    let tab = app.current_tab();
    let left_block = Block::bordered()
        .title("Terminal")
        .border_style(border_style(tab.focused == Pane::Left));
    let right_block = Block::bordered()
        .title("Claude")
        .border_style(border_style(tab.focused == Pane::Right));

    // Render left pane with shell terminal
    frame.render_widget(left_block.clone(), left);
    if let Some(ref term) = tab.left_term {
        frame.render_widget(term.widget(), inner_area(left));
    }

    // Render right pane with claude terminal
    frame.render_widget(right_block.clone(), right);
    if let Some(ref term) = tab.right_term {
        frame.render_widget(term.widget(), inner_area(right));
    }

    // Render status bar
    let mode_text = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
    };
    let status_bar = Paragraph::new(format!(" {} ", mode_text)).style(mode_style(app.mode == Mode::Insert));
    frame.render_widget(status_bar, status_area);
}
