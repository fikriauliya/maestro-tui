mod app;
mod input;
mod pty;
mod terminal;
mod ui;
mod worktree;

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind, MouseEventKind, MouseButton, EnableMouseCapture, DisableMouseCapture};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Paragraph},
    Frame, Terminal as RatatuiTerminal,
};

use crossterm::event::KeyCode;

use crate::app::{handle_key, inner_area, App, Command, Pane, Tab, TabKind};
use crate::terminal::Terminal;
use crate::ui::{active_tab_style, border_style, inactive_tab_style};
use crate::worktree::{slugify_prompt, WorktreeManager};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut app = App::new();

    // Tab 0 is always the control panel (already created by App::new())
    // Load existing worktrees as additional tabs
    if let Ok(wt_manager) = WorktreeManager::new() {
        if let Ok(worktrees) = wt_manager.list() {
            for wt in worktrees {
                let branch = wt.branch.unwrap_or_else(|| "detached".to_string());
                app.tabs.push(Tab::with_worktree(wt.path, branch));
            }
        }
    }

    // Setup terminal with mouse support
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = RatatuiTerminal::new(backend)?;

    let result = run(&mut app, &mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

fn run(app: &mut App, terminal: &mut RatatuiTerminal<CrosstermBackend<std::io::Stdout>>) -> color_eyre::Result<()> {
    // Store worktree manager for creating new worktrees
    let wt_manager = WorktreeManager::new().ok();
    // Track tab area for click detection
    let mut tab_area = Rect::default();

    loop {
        terminal.draw(|frame| {
            tab_area = render(app, frame);
        })?;

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Handle input differently based on tab type
                    let is_control_panel = app.current_tab().is_control_panel();

                    if is_control_panel {
                        // Control panel: text input handling
                        match key.code {
                            KeyCode::Enter => {
                                // Submit prompt and create worktree tab
                                if let Some(prompt) = app.take_control_panel_input() {
                                    if let Some(ref manager) = wt_manager {
                                        let branch = slugify_prompt(&prompt);
                                        if let Ok(wt) = manager.create(&branch, Some(&prompt)) {
                                            app.add_worktree_tab(wt.path.clone(), branch);
                                        }
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                app.execute(Command::DeleteControlPanelChar);
                            }
                            KeyCode::Char(c) => {
                                // Check for Ctrl+key shortcuts
                                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                    if let Some(cmd) = handle_key(&key) {
                                        app.execute(cmd);
                                    }
                                } else {
                                    app.execute(Command::UpdateControlPanelInput(c));
                                }
                            }
                            _ => {
                                // Pass Ctrl shortcuts through
                                if let Some(cmd) = handle_key(&key) {
                                    app.execute(cmd);
                                }
                            }
                        }
                    } else {
                        // Terminal tab: passthrough with Ctrl shortcuts
                        if let Some(cmd) = handle_key(&key) {
                            app.execute(cmd);
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    // Handle mouse clicks on tab bar
                    if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                        if mouse.row == tab_area.y && mouse.column >= tab_area.x && mouse.column < tab_area.x + tab_area.width {
                            // Calculate which tab was clicked
                            let mut x = 0u16;
                            for (i, _tab) in app.tabs.iter().enumerate() {
                                // Each tab takes 3 chars (" N ") plus 1 space separator
                                let tab_width = 4;
                                if mouse.column >= x && mouse.column < x + tab_width {
                                    app.execute(Command::SwitchTab(i));
                                    break;
                                }
                                x += tab_width;
                            }
                        }
                    }
                }
                Event::Resize(_, _) => {
                    // Terminal resize is handled automatically by ratatui
                }
                _ => {}
            }
        }
    }
}

fn render(app: &mut App, frame: &mut Frame) -> Rect {
    // Split into tab bar and main area (no status bar needed without modes)
    let [tab_area, main_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)])
            .areas(frame.area());

    // Render tab bar
    let mut tab_spans = Vec::new();
    for (i, tab) in app.tabs.iter().enumerate() {
        let label = match &tab.kind {
            TabKind::ControlPanel { .. } => "0".to_string(),
            TabKind::Worktree { .. } => format!("{}", i),
        };
        let style = if i == app.active_tab {
            active_tab_style()
        } else {
            inactive_tab_style()
        };
        tab_spans.push(ratatui::text::Span::styled(format!(" {} ", label), style));
        tab_spans.push(ratatui::text::Span::raw(" "));
    }
    let tab_bar = ratatui::text::Line::from(tab_spans);
    frame.render_widget(Paragraph::new(tab_bar), tab_area);

    // Check if current tab is control panel
    if app.current_tab().is_control_panel() {
        render_control_panel(app, frame, main_area);
    } else {
        render_terminal_tab(app, frame, main_area);
    }

    tab_area
}

fn render_control_panel(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    use ratatui::text::{Line, Span};
    use ratatui::style::{Color, Style};

    // Split into content area and input area
    let [content_area, input_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(3)])
            .areas(area);

    // Render control panel content
    let block = Block::bordered()
        .title("Control Panel")
        .border_style(border_style(true));

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Enter a prompt to create a new worktree tab:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
    ];

    // List existing worktree tabs
    lines.push(Line::from(Span::styled(
        "  Worktrees:",
        Style::default().fg(Color::Cyan),
    )));
    for (i, tab) in app.tabs.iter().enumerate() {
        if let TabKind::Worktree { branch, .. } = &tab.kind {
            lines.push(Line::from(format!("    [{}] {}", i, branch)));
        }
    }

    let content = Paragraph::new(lines).block(block);
    frame.render_widget(content, content_area);

    // Render input field
    let input_text = match &app.current_tab().kind {
        TabKind::ControlPanel { input } => input.as_str(),
        _ => "",
    };

    let input_block = Block::bordered()
        .title("Prompt")
        .border_style(border_style(true)); // Always focused on control panel

    // Show cursor indicator
    let input_display = format!("{}_", input_text);
    let input_widget = Paragraph::new(input_display).block(input_block);
    frame.render_widget(input_widget, input_area);
}

fn render_terminal_tab(app: &mut App, frame: &mut Frame, main_area: ratatui::layout::Rect) {
    // Split main area into left and right panes
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(main_area);

    // Handle terminal creation and resize for current tab
    let tab = app.current_tab_mut();

    if tab.needs_left_terminal(left) {
        let inner = inner_area(left);
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let term_result = if let Some(cwd) = tab.worktree_path() {
            Terminal::with_command_in_dir(inner.width, inner.height, &shell, &[], cwd)
        } else {
            Terminal::new(inner.width, inner.height)
        };
        if let Ok(term) = term_result {
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
        let term_result = if let Some(cwd) = tab.worktree_path() {
            Terminal::with_command_in_dir(inner.width, inner.height, "claude", &[], cwd)
        } else {
            Terminal::with_command(inner.width, inner.height, "claude", &[])
        };
        if let Ok(term) = term_result {
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
}
