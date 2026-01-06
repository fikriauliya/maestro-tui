//! Rendering functions for the maestro-tui application.
//!
//! Consolidates all UI rendering logic including tab bars, terminal panes,
//! control panel, status bar, and dialogs.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

use crate::app::{inner_area, App, Dialog, Pane, TabKind};
use crate::theme::{self, active_tab_style, border_style, inactive_tab_style};
use crate::worktree::{WorktreeManager, WorktreeStatus};

/// Render the entire application UI.
/// Returns (tab_area, quit_button_x, main_area) for click detection.
pub fn render(app: &mut App, frame: &mut Frame) -> (Rect, u16, Rect) {
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
        tab_spans.push(Span::styled(format!(" {} ", label), style));
        tab_spans.push(Span::raw(" "));
    }
    let tab_bar = Line::from(tab_spans);
    frame.render_widget(Paragraph::new(tab_bar), tab_area);

    // Render quit button [X] on the right side
    let quit_text = "[X]";
    let quit_x = tab_area.width.saturating_sub(quit_text.len() as u16);
    let quit_area = Rect::new(tab_area.x + quit_x, tab_area.y, quit_text.len() as u16, 1);
    let quit_style = Style::default().fg(theme::RED);
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

/// Render the control panel tab with worktree management interface.
fn render_control_panel(app: &App, frame: &mut Frame, area: Rect) {
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

/// Render a terminal tab with left and right panes.
fn render_terminal_tab(app: &mut App, frame: &mut Frame, main_area: Rect) {
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
    if let Some(term) = tab.pair.get(Pane::Left) {
        frame.render_widget(term.widget(), inner_area(left));
    }

    // Render right pane with claude terminal
    frame.render_widget(right_block.clone(), right);
    if let Some(term) = tab.pair.get(Pane::Right) {
        frame.render_widget(term.widget(), inner_area(right));
    }
}

/// Render the status bar with keyboard shortcuts.
fn render_status_bar(frame: &mut Frame, area: Rect) {
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

/// Render a modal dialog.
fn render_dialog(dialog: &Dialog, frame: &mut Frame, area: Rect) {
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
