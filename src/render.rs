//! Rendering functions for the maestro-tui application.
//!
//! Consolidates all UI rendering logic including tab bars, terminal panes,
//! control panel, status bar, and dialogs.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use ratatui::style::Color;

use crate::app::{App, ContentFocus, ControlPanelPane, Dialog, Pane, TabKind, inner_area};
use crate::theme::{self, Theme, ALL_THEMES};

/// Render the entire application UI.
/// Returns (tab_area, quit_button_x, main_area) for click detection.
pub fn render(app: &mut App, frame: &mut Frame) -> (Rect, u16, Rect) {
    // Copy theme to avoid borrow issues
    let theme = app.theme;

    // Split into tab bar, main area, and status bar
    let [tab_area, main_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    // Render status bar with keyboard shortcuts
    render_status_bar(frame, status_area, &theme);

    // Use cached worktree statuses for tab display (refreshed periodically, not every frame)
    let worktree_statuses = app.worktree_status_map();

    // Render tab bar with quit button on the right
    let mut tab_spans = Vec::new();
    for (i, tab) in app.tabs.iter().enumerate() {
        let is_active = i == app.active_tab;

        match &tab.kind {
            TabKind::ControlPanel { .. } => {
                // Control tab uses distinct purple color
                let (fg, bg) = if is_active {
                    theme.active_control_tab_colors()
                } else {
                    theme.inactive_control_tab_colors()
                };
                let style = Style::default().fg(fg).bg(bg);
                tab_spans.push(Span::styled(" 1 Control ", style));
            }
            TabKind::Worktree { path, branch, .. } => {
                // Worktree tabs use accent color
                let (fg, bg) = if is_active {
                    theme.active_tab_colors()
                } else {
                    theme.inactive_tab_colors()
                };
                let style = Style::default().fg(fg).bg(bg);
                // Add tab index and branch name (display index = internal index + 1)
                tab_spans.push(Span::styled(format!(" {} {} ", i + 1, branch), style));

                // Add dirty indicator if available (ahead/behind shown in control pane)
                if let Some(status) = worktree_statuses.get(path)
                    && status.is_dirty
                {
                    tab_spans.push(Span::styled("●", Style::default().fg(theme::RED)));
                }
            }
        }
        tab_spans.push(Span::raw(" "));
    }
    let tab_bar = Line::from(tab_spans);
    frame.render_widget(Paragraph::new(tab_bar), tab_area);

    // Render quit button [X] on the right side
    let quit_text = "[X]";
    let quit_x = tab_area.width.saturating_sub(quit_text.len() as u16);
    let quit_area = Rect::new(tab_area.x + quit_x, tab_area.y, quit_text.len() as u16, 1);
    let quit_style = Style::default().fg(theme.accents.red);
    frame.render_widget(Paragraph::new(quit_text).style(quit_style), quit_area);

    // Check if current tab is control panel
    if app.current_tab().is_control_panel() {
        render_control_panel(app, frame, main_area);
    } else {
        render_terminal_tab(app, frame, main_area);
    }

    // Render dialog on top if shown
    if !matches!(app.dialog, Dialog::None) {
        render_dialog(&app.dialog, &theme, frame, frame.area());
    }

    (tab_area, quit_x, main_area)
}

/// Render the control panel tab with worktree management interface.
fn render_control_panel(app: &mut App, frame: &mut Frame, area: Rect) {
    let theme = app.theme;

    // Split into left (content) and right (Claude terminal) panes
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

    // Get the focused pane
    let focused_pane = app.current_tab().control_panel_pane();

    // Render left pane (content + input)
    render_control_panel_content(
        app,
        frame,
        left_area,
        focused_pane == ControlPanelPane::Content,
        &theme,
    );

    // Ensure Claude terminal exists and render right pane
    let tab = app.current_tab_mut();
    tab.ensure_claude_terminal(right_area);

    let tab = app.current_tab();
    let right_block = Block::bordered()
        .title("Claude")
        .border_style(Style::default().fg(theme.border_color(focused_pane == ControlPanelPane::Claude)));

    frame.render_widget(right_block.clone(), right_area);
    if let Some(ref term) = tab.claude_terminal {
        frame.render_widget(term.widget(), inner_area(right_area));
    }
}

/// Render the content portion of the control panel (left side).
/// Layout: Input (top) -> Worktrees (middle) -> Ready Issues (bottom)
fn render_control_panel_content(app: &App, frame: &mut Frame, area: Rect, is_focused: bool, theme: &Theme) {
    // Split into three sections: Input (top), Worktrees (middle), Issues (bottom)
    let [input_area, worktrees_area, issues_area] = Layout::vertical([
        Constraint::Length(3),  // Input field
        Constraint::Min(8),     // Worktrees pane (flexible, minimum 8 lines)
        Constraint::Length(10), // Ready Issues (fixed height)
    ])
    .areas(area);

    // Get content focus state for sub-pane highlighting
    let content_focus = app.current_tab().content_focus();
    let worktrees_focused = is_focused && content_focus == ContentFocus::Worktrees;
    let input_focused = is_focused && content_focus == ContentFocus::Input;

    // Render input field (top)
    render_input_pane(app, frame, input_area, input_focused, theme);

    // Render worktrees pane (middle)
    render_worktrees_pane(app, frame, worktrees_area, worktrees_focused, theme);

    // Render ready issues pane (bottom)
    render_issues_pane(app, frame, issues_area, theme);
}

/// Render the input field for creating new worktree tasks.
fn render_input_pane(app: &App, frame: &mut Frame, area: Rect, is_focused: bool, theme: &Theme) {
    let input_text = match &app.current_tab().kind {
        TabKind::ControlPanel { input, .. } => input.as_str(),
        _ => "",
    };

    let input_block = Block::bordered()
        .title("New Task (creates worktree + tab)")
        .border_style(Style::default().fg(theme.border_color(is_focused)));

    // Show cursor indicator only when input is focused
    let input_display = if is_focused {
        format!("{}_", input_text)
    } else {
        input_text.to_string()
    };
    let input_widget = Paragraph::new(input_display).block(input_block);
    frame.render_widget(input_widget, area);
}

/// Render the worktrees selection pane.
fn render_worktrees_pane(app: &App, frame: &mut Frame, area: Rect, is_focused: bool, theme: &Theme) {
    let block = Block::bordered()
        .title("Worktrees (j/k to navigate, Enter for actions)")
        .border_style(Style::default().fg(theme.border_color(is_focused)));

    // Use cached worktree status information (refreshed periodically, not every frame)
    let worktree_statuses = app.worktree_statuses();

    // Get selected worktree index
    let selected_idx = app.current_tab().selected_worktree();

    let mut lines = Vec::new();

    for (idx, status) in worktree_statuses.iter().enumerate() {
        let branch = status.worktree.branch.as_deref().unwrap_or("detached");
        let is_selected = idx == selected_idx && is_focused;

        // Build status indicators
        let mut indicators = Vec::new();

        // Dirty indicator
        if status.is_dirty {
            indicators.push(Span::styled(" ●", Style::default().fg(theme.accents.red)));
        }

        // Ahead/behind indicators
        if status.ahead > 0 {
            indicators.push(Span::styled(
                format!(" ↑{}", status.ahead),
                Style::default().fg(theme.accents.green),
            ));
        }
        if status.behind > 0 {
            indicators.push(Span::styled(
                format!(" ↓{}", status.behind),
                Style::default().fg(theme.accents.orange),
            ));
        }

        // Build the line with selection indicator
        let prefix = if is_selected { " > " } else { "   " };
        let branch_style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let mut spans = vec![Span::raw(prefix), Span::styled(branch, branch_style)];
        spans.extend(indicators);
        lines.push(Line::from(spans));
    }

    // Legend for worktrees (at the bottom of the pane)
    if !worktree_statuses.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled("●", Style::default().fg(theme.accents.red)),
            Span::raw(" dirty  "),
            Span::styled("↑", Style::default().fg(theme.accents.green)),
            Span::raw(" ahead  "),
            Span::styled("↓", Style::default().fg(theme.accents.orange)),
            Span::raw(" behind"),
        ]));
    }

    let content = Paragraph::new(lines).block(block);
    frame.render_widget(content, area);
}

/// Render the ready issues pane.
fn render_issues_pane(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let block = Block::bordered()
        .title("Ready Issues [Alt+b to reload]")
        .border_style(Style::default().fg(theme.border_color(false)));

    let bd_ready_output = app.get_bd_ready_output();

    let mut lines = Vec::new();

    if bd_ready_output.is_empty() {
        lines.push(Line::from(Span::styled(
            " (no ready issues)",
            Style::default().fg(theme.tx_muted),
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
                Span::styled(format!(" {}", trimmed), Style::default().fg(theme.accents.red))
            } else if trimmed.starts_with("[P1]") || trimmed.contains("[P1]") {
                // High priority - orange
                Span::styled(format!(" {}", trimmed), Style::default().fg(theme.accents.orange))
            } else {
                // Normal priority
                Span::raw(format!(" {}", trimmed))
            };
            lines.push(Line::from(styled_line));
        }
    }

    let content = Paragraph::new(lines).block(block);
    frame.render_widget(content, area);
}

/// Render a terminal tab with diff viewer on left and Claude terminal on right.
fn render_terminal_tab(app: &mut App, frame: &mut Frame, main_area: Rect) {
    // Split main area into left and right panes
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(main_area);

    // Ensure right terminal (Claude) exists and is properly sized
    let tab = app.current_tab_mut();
    tab.ensure_right_terminal(right);

    let theme = &app.theme;
    let tab = app.current_tab();
    let left_block = Block::bordered()
        .title("Diff")
        .border_style(Style::default().fg(theme.border_color(tab.focused == Pane::Left)));
    let right_block = Block::bordered()
        .title("Claude")
        .border_style(Style::default().fg(theme.border_color(tab.focused == Pane::Right)));

    // Render left pane with diff viewer
    frame.render_widget(left_block.clone(), left);
    if let Some(ref dv) = tab.diff_viewer {
        let inner = inner_area(left);
        frame.render_widget(dv.widget(inner.height), inner);
    }

    // Render right pane with claude terminal
    frame.render_widget(right_block.clone(), right);
    if let Some(term) = tab.pair.get(Pane::Right) {
        frame.render_widget(term.widget(), inner_area(right));
    }
}

/// Render the status bar with keyboard shortcuts.
fn render_status_bar(frame: &mut Frame, area: Rect, theme: &Theme) {
    // Zellij-style: <key> action  <key> action ...
    let (key_fg, key_bg) = theme.status_key_colors();
    let key_style = Style::default().fg(key_fg).bg(key_bg);
    let action_style = Style::default().fg(theme.status_action_color());

    let shortcuts = vec![
        ("Alt+0-9", "Tabs"),
        ("Alt+h/l", "Focus"),
        ("Alt+u/d", "Scroll"),
        ("Alt+t", "Theme"),
        ("Alt+q", "Quit"),
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
fn render_dialog(dialog: &Dialog, theme: &Theme, frame: &mut Frame, area: Rect) {
    // Calculate dialog size and position (centered)
    let dialog_height = match dialog {
        Dialog::ThemePicker { .. } => (ALL_THEMES.len() + 6) as u16,
        _ => 9u16,
    };
    let dialog_width = 60u16.min(area.width.saturating_sub(4));
    let dialog_x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
    let dialog_y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

    // Clear the area behind the dialog
    frame.render_widget(Clear, dialog_area);

    let (title, lines, border_color) = match dialog {
        Dialog::WorktreeAction { branch } => {
            let content = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Worktree: "),
                    Span::styled(
                        branch,
                        Style::default()
                            .fg(theme.accents.cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from("  Choose an action:"),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Press "),
                    Span::styled(
                        "M",
                        Style::default()
                            .fg(theme.accents.green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" to merge, "),
                    Span::styled(
                        "R",
                        Style::default().fg(theme.accents.red).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" to remove, "),
                    Span::styled(
                        "Esc",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" to cancel"),
                ]),
            ];
            ("Worktree Action", content, theme.accent)
        }
        Dialog::ThemePicker { selected } => {
            let mut content = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Select a theme:",
                    Style::default().fg(theme.highlight),
                )),
                Line::from(""),
            ];

            for (i, t) in ALL_THEMES.iter().enumerate() {
                let indicator = if i == *selected { "▸ " } else { "  " };
                let style = if i == *selected {
                    Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.tx)
                };
                content.push(Line::from(Span::styled(
                    format!("  {}  {}", indicator, t.name),
                    style,
                )));
            }

            content.push(Line::from(""));
            content.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("j/k", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" navigate  "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" select  "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" cancel"),
            ]));

            ("Theme", content, theme.accent)
        }
        Dialog::ConfirmDelete { branch, unmerged } => {
            let mut content = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Delete worktree "),
                    Span::styled(branch, Style::default().fg(theme.accents.cyan).add_modifier(Modifier::BOLD)),
                    Span::raw("?"),
                ]),
            ];
            if *unmerged {
                content.push(Line::from(""));
                content.push(Line::from(Span::styled(
                    "  ⚠ WARNING: Branch has unmerged commits!",
                    Style::default().fg(theme.accents.yellow),
                )));
            }
            content.push(Line::from(""));
            content.push(Line::from(vec![
                Span::raw("  Press "),
                Span::styled("Y", Style::default().fg(theme.accents.green).add_modifier(Modifier::BOLD)),
                Span::raw(" to confirm, "),
                Span::styled("N", Style::default().fg(theme.accents.red).add_modifier(Modifier::BOLD)),
                Span::raw(" to cancel"),
            ]));
            ("Delete Worktree", content, theme.accents.red)
        }
        Dialog::UncommittedChanges { branch } => {
            let content = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Cannot delete "),
                    Span::styled(branch, Style::default().fg(theme.accents.cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  ✗ Worktree has uncommitted changes!",
                    Style::default().fg(theme.accents.red),
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
            ("Cannot Delete", content, theme.accents.red)
        }
        Dialog::None => return,
    };

    let block = Block::bordered()
        .title(title)
        .border_style(Style::default().fg(border_color));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, dialog_area);
}
