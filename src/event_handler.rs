//! Event handling for the maestro-tui application.
//!
//! Handles keyboard and mouse input events, dispatching them to the appropriate
//! handlers based on application state (dialog, control panel, or terminal mode).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::{handle_dialog_key, handle_key, App, Command, Dialog, Pane, TabKind};
use crate::worktree::{generate_merge_message, slugify_prompt, RemoveWarning, WorktreeManager};

/// Result of handling an event.
pub enum KeyAction {
    /// Continue to next event
    Continue,
    /// Quit the application
    Quit,
}

/// Load output from `bd ready` command.
pub fn load_bd_ready() -> Vec<String> {
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

/// Handle dialog key events.
pub fn process_dialog_key(
    app: &mut App,
    key: &KeyEvent,
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

/// Handle control panel key events.
pub fn process_control_panel_key(
    app: &mut App,
    key: &KeyEvent,
    wt_manager: &Option<WorktreeManager>,
) -> KeyAction {
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

/// Handle terminal tab key events.
pub fn process_terminal_key(
    app: &mut App,
    key: &KeyEvent,
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

/// Merge current worktree branch into main.
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

/// Show delete worktree dialog.
fn handle_delete_worktree(app: &mut App, wt_manager: &Option<WorktreeManager>) {
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

/// Handle mouse click events, returns true if should quit.
pub fn process_mouse_click(
    app: &mut App,
    mouse: &MouseEvent,
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
