mod app;
mod input;
mod pty;
mod terminal;
mod theme;
mod worktree;

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind, MouseEventKind, MouseButton, EnableMouseCapture, DisableMouseCapture};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, Clear, ClearType};
use crossterm::execute;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Paragraph},
    Frame, Terminal as RatatuiTerminal,
};

use crossterm::event::KeyCode;

use crate::app::{handle_key, handle_dialog_key, inner_area, App, Command, Dialog, Pane, Tab, TabKind};
use crate::theme::{active_tab_style, border_style, inactive_tab_style};
use crate::worktree::{generate_merge_message, slugify_prompt, WorktreeManager, WorktreeStatus};

/// Load output from `bd ready` command
fn load_bd_ready() -> Vec<String> {
    std::process::Command::new("bd")
        .arg("ready")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.lines().map(String::from).collect())
        .unwrap_or_default()
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut app = App::new();

    // Load bd ready output at startup
    app.execute(Command::ReloadBdReady(load_bd_ready()));

    // Tab 0 is always the control panel (already created by App::new())
    // Load existing worktrees as additional tabs (no stored prompt for existing worktrees)
    if let Ok(wt_manager) = WorktreeManager::new()
        && let Ok(worktrees) = wt_manager.list()
    {
        for wt in worktrees {
            let branch = wt.branch.unwrap_or_else(|| "detached".to_string());
            app.tabs.push(Tab::with_worktree(wt.path, branch, String::new()));
        }
    }

    // Setup terminal with mouse support
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, Clear(ClearType::All), EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = RatatuiTerminal::new(backend)?;

    let result = run(&mut app, &mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), Clear(ClearType::All), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

/// Result of handling a key event
enum KeyAction {
    /// Continue to next event
    Continue,
    /// Quit the application
    Quit,
}

/// Handle dialog key events
fn process_dialog_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    wt_manager: &Option<WorktreeManager>,
) {
    let Some(cmd) = handle_dialog_key(key) else {
        return;
    };

    if !matches!(cmd, Command::DialogConfirm) {
        app.execute(cmd);
        return;
    }

    // Handle confirmation based on dialog type
    match &app.dialog {
        Dialog::ConfirmDelete { branch, .. } => {
            let branch = branch.clone();
            let tab_idx = app.active_tab;
            if let Some(manager) = wt_manager {
                let _ = manager.remove(&branch, true);
                app.remove_tab(tab_idx);
            }
            app.dialog = Dialog::None;
        }
        Dialog::UncommittedChanges { .. } => {
            app.dialog = Dialog::None;
        }
        Dialog::None => {}
    }
}

/// Handle control panel key events
fn process_control_panel_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    wt_manager: &Option<WorktreeManager>,
) -> KeyAction {
    use crossterm::event::KeyModifiers;

    // Check for Ctrl/Alt shortcuts first
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        // Special: Ctrl+b reloads bd ready
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b') {
            app.execute(Command::ReloadBdReady(load_bd_ready()));
            return KeyAction::Continue;
        }

        if let Some(cmd) = handle_key(key) {
            if matches!(cmd, Command::Quit) {
                return KeyAction::Quit;
            }
            app.execute(cmd);
            return KeyAction::Continue;
        }
    }

    // Handle text input
    match key.code {
        KeyCode::Enter => {
            if let Some(prompt) = app.take_control_panel_input()
                && let Some(manager) = wt_manager
            {
                let branch = slugify_prompt(&prompt);
                if let Ok(wt) = manager.create(&branch, Some(&prompt)) {
                    app.add_worktree_tab(wt.path.clone(), branch, prompt);
                }
            }
        }
        KeyCode::Backspace => app.execute(Command::DeleteControlPanelChar),
        KeyCode::Char(c) => app.execute(Command::UpdateControlPanelInput(c)),
        _ => {}
    }

    KeyAction::Continue
}

/// Handle terminal tab key events
fn process_terminal_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    wt_manager: &Option<WorktreeManager>,
) -> KeyAction {
    let Some(cmd) = handle_key(key) else {
        return KeyAction::Continue;
    };

    match cmd {
        Command::Quit => KeyAction::Quit,
        Command::MergeBranch => {
            handle_merge_branch(app, wt_manager);
            KeyAction::Continue
        }
        Command::DeleteWorktree => {
            handle_delete_worktree(app, wt_manager);
            KeyAction::Continue
        }
        cmd => {
            app.execute(cmd);
            KeyAction::Continue
        }
    }
}

/// Merge current worktree branch into main
fn handle_merge_branch(app: &mut App, wt_manager: &Option<WorktreeManager>) {
    let Some(manager) = wt_manager else {
        return;
    };
    let Some(branch) = app.current_tab().branch().map(String::from) else {
        return;
    };

    // Generate commit message using Claude
    let commit_msg = manager
        .get_branch_diff(&branch)
        .ok()
        .and_then(|diff| generate_merge_message(&branch, "main", &diff).ok());

    let tab_idx = app.active_tab;
    if manager.merge(&branch, commit_msg.as_deref()).is_ok() {
        let _ = manager.remove(&branch, true);
        app.remove_tab(tab_idx);
    }
}

/// Show delete worktree dialog
fn handle_delete_worktree(app: &mut App, wt_manager: &Option<WorktreeManager>) {
    use crate::worktree::RemoveWarning;

    let Some(manager) = wt_manager else {
        return;
    };
    let Some(branch) = app.current_tab().branch().map(String::from) else {
        return;
    };

    let warnings = manager.remove(&branch, false).unwrap_or_default();
    let has_uncommitted = warnings.iter().any(|w| matches!(w, RemoveWarning::UncommittedChanges));
    let has_unmerged = warnings.iter().any(|w| matches!(w, RemoveWarning::NotMerged { .. }));

    if has_uncommitted {
        app.dialog = Dialog::UncommittedChanges { branch };
    } else {
        app.dialog = Dialog::ConfirmDelete { branch, unmerged: has_unmerged };
    }
}

/// Handle mouse click events, returns true if should quit
fn process_mouse_click(
    app: &mut App,
    mouse: &crossterm::event::MouseEvent,
    tab_area: Rect,
    quit_button_x: u16,
    main_area: Rect,
) -> bool {
    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return false;
    }

    // Tab bar clicks
    if mouse.row == tab_area.y {
        if mouse.column >= quit_button_x {
            return true; // Quit
        }

        // Tab selection
        if mouse.column >= tab_area.x && mouse.column < tab_area.x + tab_area.width {
            let mut x = 0u16;
            for (i, tab) in app.tabs.iter().enumerate() {
                let label = match &tab.kind {
                    TabKind::ControlPanel { .. } => "0 Control".to_string(),
                    TabKind::Worktree { branch, .. } => format!("{} {}", i, branch),
                };
                let tab_width = (label.len() + 2 + 1) as u16;
                if mouse.column >= x && mouse.column < x + tab_width {
                    app.execute(Command::SwitchTab(i));
                    break;
                }
                x += tab_width;
            }
        }
        return false;
    }

    // Pane focus clicks (only for terminal tabs)
    if !app.current_tab().is_control_panel()
        && mouse.row >= main_area.y
        && mouse.row < main_area.y + main_area.height
    {
        let mid_x = main_area.x + main_area.width / 2;
        let pane = if mouse.column < mid_x { Pane::Left } else { Pane::Right };
        app.execute(Command::FocusPane(pane));
    }

    false
}

fn run(app: &mut App, terminal: &mut RatatuiTerminal<CrosstermBackend<std::io::Stdout>>) -> color_eyre::Result<()> {
    let wt_manager = WorktreeManager::new().ok();
    let mut tab_area = Rect::default();
    let mut quit_button_x = 0u16;
    let mut main_area = Rect::default();

    loop {
        terminal.draw(|frame| {
            (tab_area, quit_button_x, main_area) = render(app, frame);
        })?;

        if !event::poll(Duration::from_millis(16))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                // Dialog takes priority
                if !matches!(app.dialog, Dialog::None) {
                    process_dialog_key(app, &key, &wt_manager);
                    continue;
                }

                let action = if app.current_tab().is_control_panel() {
                    process_control_panel_key(app, &key, &wt_manager)
                } else {
                    process_terminal_key(app, &key, &wt_manager)
                };

                if matches!(action, KeyAction::Quit) {
                    return Ok(());
                }
            }
            Event::Mouse(mouse) => {
                if process_mouse_click(app, &mouse, tab_area, quit_button_x, main_area) {
                    return Ok(());
                }
            }
            _ => {}
        }
    }
}

/// Returns (tab_area, quit_button_x, main_area) for click detection
fn render(app: &mut App, frame: &mut Frame) -> (Rect, u16, Rect) {
    // Split into tab bar, main area, and status bar
    let [tab_area, main_area, status_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
            .areas(frame.area());

    // Render status bar with keyboard shortcuts
    render_status_bar(frame, status_area);

    // Render tab bar with quit button on the right
    let mut tab_spans = Vec::new();
    for (i, tab) in app.tabs.iter().enumerate() {
        let label = match &tab.kind {
            TabKind::ControlPanel { .. } => "0 Control".to_string(),
            TabKind::Worktree { branch, .. } => format!("{} {}", i, branch),
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

    // Render quit button [X] on the right side
    let quit_text = "[X]";
    let quit_x = tab_area.width.saturating_sub(quit_text.len() as u16);
    let quit_area = Rect::new(tab_area.x + quit_x, tab_area.y, quit_text.len() as u16, 1);
    let quit_style = ratatui::style::Style::default().fg(theme::RED);
    frame.render_widget(Paragraph::new(quit_text).style(quit_style), quit_area);

    // Check if current tab is control panel
    if app.current_tab().is_control_panel() {
        render_control_panel(app, frame, main_area);
    } else {
        render_terminal_tab(app, frame, main_area);
    }

    // Render dialog on top if shown
    if !matches!(app.dialog, Dialog::None) {
        render_dialog(&app.dialog, frame, frame.area());
    }

    (tab_area, quit_x, main_area)
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

    // Get worktree status information
    let worktree_statuses: Vec<WorktreeStatus> = WorktreeManager::new()
        .and_then(|m| m.list_with_status())
        .unwrap_or_default();

    // List worktrees with status
    lines.push(Line::from(Span::styled(
        "  Worktrees:",
        Style::default().fg(Color::Cyan),
    )));

    for status in &worktree_statuses {
        let branch = status.worktree.branch.as_deref().unwrap_or("detached");

        // Build status indicators
        let mut indicators = Vec::new();

        // Dirty indicator
        if status.is_dirty {
            indicators.push(Span::styled(" ●", Style::default().fg(theme::RED)));
        }

        // Ahead/behind indicators
        if status.ahead > 0 {
            indicators.push(Span::styled(
                format!(" ↑{}", status.ahead),
                Style::default().fg(theme::GREEN),
            ));
        }
        if status.behind > 0 {
            indicators.push(Span::styled(
                format!(" ↓{}", status.behind),
                Style::default().fg(theme::ORANGE),
            ));
        }

        // Build the line
        let mut spans = vec![Span::raw(format!("    {}", branch))];
        spans.extend(indicators);
        lines.push(Line::from(spans));
    }

    // Legend for worktrees
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("●", Style::default().fg(theme::RED)),
        Span::raw(" dirty  "),
        Span::styled("↑", Style::default().fg(theme::GREEN)),
        Span::raw(" ahead  "),
        Span::styled("↓", Style::default().fg(theme::ORANGE)),
        Span::raw(" behind"),
    ]));

    // Add bd ready section
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Ready Issues ", Style::default().fg(Color::Cyan)),
        Span::styled("[Ctrl+b to reload]", Style::default().fg(Color::DarkGray)),
    ]));

    let bd_ready_output = app.get_bd_ready_output();
    if bd_ready_output.is_empty() {
        lines.push(Line::from(Span::styled(
            "    (no bd ready output)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        // Skip the header line if present (starts with emoji or "Ready work")
        for line in bd_ready_output.iter() {
            let trimmed = line.trim();
            // Skip empty lines and the header line
            if trimmed.is_empty() || trimmed.starts_with("📋") {
                continue;
            }
            // Display issue lines with some styling
            let styled_line = if trimmed.starts_with("[P0]") || trimmed.contains("[P0]") {
                // Critical priority - red
                Span::styled(format!("    {}", trimmed), Style::default().fg(theme::RED))
            } else if trimmed.starts_with("[P1]") || trimmed.contains("[P1]") {
                // High priority - orange
                Span::styled(format!("    {}", trimmed), Style::default().fg(theme::ORANGE))
            } else {
                // Normal priority
                Span::raw(format!("    {}", trimmed))
            };
            lines.push(Line::from(styled_line));
        }
    }

    let content = Paragraph::new(lines).block(block);
    frame.render_widget(content, content_area);

    // Render input field
    let input_text = match &app.current_tab().kind {
        TabKind::ControlPanel { input, .. } => input.as_str(),
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

    // Ensure terminals exist and are properly sized
    let tab = app.current_tab_mut();
    tab.ensure_left_terminal(left);
    tab.ensure_right_terminal(right);

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

fn render_status_bar(frame: &mut Frame, area: Rect) {
    use ratatui::style::Style;
    use ratatui::text::{Line, Span};

    // Zellij-style: <key> action  <key> action ...
    let key_style = Style::default().fg(theme::BG).bg(theme::GREEN);
    let action_style = Style::default().fg(theme::PAPER);

    let shortcuts = vec![
        ("Alt+0-9", "Tabs"),
        ("Ctrl+h", "Left"),
        ("Ctrl+l", "Right"),
        ("Ctrl+m", "Merge"),
        ("Ctrl+r", "Remove"),
        ("Ctrl+x", "Quit"),
    ];

    let mut spans = Vec::new();
    for (key, action) in shortcuts {
        spans.push(Span::styled(format!(" {} ", key), key_style));
        spans.push(Span::styled(format!(" {} ", action), action_style));
        spans.push(Span::raw(" "));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_dialog(dialog: &Dialog, frame: &mut Frame, area: Rect) {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Clear;

    // Calculate dialog size and position (centered)
    let dialog_width = 60u16.min(area.width.saturating_sub(4));
    let dialog_height = 9u16;
    let dialog_x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
    let dialog_y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

    // Clear the area behind the dialog
    frame.render_widget(Clear, dialog_area);

    let (title, lines) = match dialog {
        Dialog::ConfirmDelete { branch, unmerged } => {
            let mut content = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Delete worktree "),
                    Span::styled(branch, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw("?"),
                ]),
            ];
            if *unmerged {
                content.push(Line::from(""));
                content.push(Line::from(Span::styled(
                    "  ⚠ WARNING: Branch has unmerged commits!",
                    Style::default().fg(Color::Yellow),
                )));
            }
            content.push(Line::from(""));
            content.push(Line::from(vec![
                Span::raw("  Press "),
                Span::styled("Y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" to confirm, "),
                Span::styled("N", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(" to cancel"),
            ]));
            ("Delete Worktree", content)
        }
        Dialog::UncommittedChanges { branch } => {
            let content = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Cannot delete "),
                    Span::styled(branch, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  ✗ Worktree has uncommitted changes!",
                    Style::default().fg(Color::Red),
                )),
                Line::from(""),
                Line::from("  Please commit or stash your changes first."),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Press "),
                    Span::styled("any key", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to close"),
                ]),
            ];
            ("Cannot Delete", content)
        }
        Dialog::None => return,
    };

    let block = Block::bordered()
        .title(title)
        .border_style(Style::default().fg(theme::RED));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, dialog_area);
}
