//! Event handling for the maestro-tui application.
//!
//! Handles keyboard and mouse input events, dispatching them to the appropriate
//! handlers based on application state (dialog, control panel, or terminal mode).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::{
    App, Command, ControlPanelPane, Dialog, Pane, TabKind, handle_dialog_key, handle_key,
};
use crate::input::key_to_bytes;
use crate::worktree::{RemoveWarning, WorktreeManager, generate_branch_name, generate_rebase_prompt};

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
pub fn process_dialog_key(app: &mut App, key: &KeyEvent, wt_manager: &Option<WorktreeManager>) {
    let Some(cmd) = handle_dialog_key(key, &app.dialog) else {
        return;
    };

    match cmd {
        Command::DialogCancel => {
            app.dialog = Dialog::None;
        }
        Command::DialogMerge => {
            // Handle merge action from WorktreeAction dialog
            if let Dialog::WorktreeAction { ref branch } = app.dialog {
                let branch = branch.clone();
                app.dialog = Dialog::None;
                trigger_merge_worktree(app, wt_manager, &branch);
            }
        }
        Command::DialogRemove => {
            // Handle remove action from WorktreeAction dialog
            if let Dialog::WorktreeAction { ref branch } = app.dialog {
                let branch = branch.clone();
                app.dialog = Dialog::None;
                trigger_delete_worktree(app, wt_manager, &branch);
            }
        }
        Command::DialogConfirm => {
            // Handle confirmation based on dialog type
            match &app.dialog {
                Dialog::ConfirmDelete { branch, .. } => {
                    let branch = branch.clone();
                    if let Some(manager) = wt_manager {
                        let _ = manager.remove(&branch, true);
                        // Find and remove the tab for this branch (if any)
                        if let Some(tab_idx) = app.tabs.iter().position(
                            |tab| matches!(&tab.kind, TabKind::Worktree { branch: b, .. } if b == &branch),
                        ) {
                            app.remove_tab(tab_idx);
                        }
                    }
                    app.dialog = Dialog::None;
                }
                Dialog::UncommittedChanges { .. } => {
                    app.dialog = Dialog::None;
                }
                Dialog::ThemePicker { .. } | Dialog::WorktreeAction { .. } | Dialog::None => {}
            }
        }
        _ => {}
    }
}

/// Handle control panel key events.
pub fn process_control_panel_key(
    app: &mut App,
    key: &KeyEvent,
    wt_manager: &Option<WorktreeManager>,
) -> KeyAction {
    // Check for Alt shortcuts first
    if key.modifiers.contains(KeyModifiers::ALT) {
        // Special: Alt+b reloads bd ready
        if key.code == KeyCode::Char('b') {
            app.execute(Command::ReloadBdReady(load_bd_ready()));
            return KeyAction::Continue;
        }

        // Alt+h focuses content pane, Alt+l focuses Claude pane
        if key.code == KeyCode::Char('h') {
            app.current_tab_mut()
                .set_control_panel_pane(ControlPanelPane::Content);
            return KeyAction::Continue;
        }
        if key.code == KeyCode::Char('l') {
            app.current_tab_mut()
                .set_control_panel_pane(ControlPanelPane::Claude);
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

    // Get the focused pane
    let focused_pane = app.current_tab().control_panel_pane();

    // Route input based on focused pane
    match focused_pane {
        ControlPanelPane::Content => {
            // Handle worktree navigation with up/down and j/k
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    app.current_tab_mut().select_prev_worktree();
                    return KeyAction::Continue;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let worktree_count = wt_manager
                        .as_ref()
                        .and_then(|m| m.list_with_status().ok())
                        .map(|v| v.len())
                        .unwrap_or(0);
                    app.current_tab_mut().select_next_worktree(worktree_count);
                    return KeyAction::Continue;
                }
                _ => {}
            }

            // Handle text input for worktree creation or worktree action
            match key.code {
                KeyCode::Enter => {
                    // Check if there's input text - if so, create new worktree
                    if let Some(prompt) = app.take_control_panel_input()
                        && let Some(manager) = wt_manager
                    {
                        let branch = generate_branch_name(&prompt);
                        if let Ok(wt) = manager.create(&branch, Some(&prompt)) {
                            app.add_worktree_tab(wt.path.clone(), branch, prompt);
                        }
                    } else {
                        // No input text - show action dialog for selected worktree
                        if let Some(branch) = get_selected_worktree_branch(app, wt_manager) {
                            // Don't show action dialog for main/master
                            if branch != "main" && branch != "master" {
                                app.dialog = Dialog::WorktreeAction { branch };
                            }
                        }
                    }
                }
                KeyCode::Backspace => app.execute(Command::DeleteControlPanelChar),
                KeyCode::Char(c) => app.execute(Command::UpdateControlPanelInput(c)),
                _ => {}
            }
        }
        ControlPanelPane::Claude => {
            // Pass input to Claude terminal
            let bytes = key_to_bytes(key);
            if !bytes.is_empty()
                && let Some(ref mut term) = app.current_tab_mut().claude_terminal
            {
                let _ = term.write(&bytes);
            }
        }
    }

    KeyAction::Continue
}

/// Handle terminal tab key events.
/// Note: Merge/remove operations are now handled in the control panel only.
pub fn process_terminal_key(
    app: &mut App,
    key: &KeyEvent,
    _wt_manager: &Option<WorktreeManager>,
) -> KeyAction {
    let Some(cmd) = handle_key(key) else {
        return KeyAction::Continue;
    };

    match cmd {
        Command::Quit => KeyAction::Quit,
        cmd => {
            app.execute(cmd);
            KeyAction::Continue
        }
    }
}

/// Get the branch name of the selected worktree in control panel.
fn get_selected_worktree_branch(app: &App, wt_manager: &Option<WorktreeManager>) -> Option<String> {
    let manager = wt_manager.as_ref()?;
    let worktrees = manager.list_with_status().ok()?;
    let selected_idx = app.current_tab().selected_worktree();
    worktrees
        .get(selected_idx)
        .and_then(|s| s.worktree.branch.clone())
}

/// Find the tab index for a given branch name.
#[allow(dead_code)]
fn find_tab_for_branch(app: &App, branch: &str) -> Option<usize> {
    app.tabs
        .iter()
        .position(|tab| matches!(&tab.kind, TabKind::Worktree { branch: b, .. } if b == branch))
}

/// Trigger rebase workflow for a specific worktree via Claude Code pane.
/// Sends a prompt to the Claude Code terminal in the control panel.
fn trigger_merge_worktree(app: &mut App, wt_manager: &Option<WorktreeManager>, branch: &str) {
    let Some(manager) = wt_manager else {
        return;
    };

    // Don't rebase main branch
    if branch == "main" || branch == "master" {
        return;
    }

    // Get the worktree path
    let Ok(worktree_path) = manager.switch(branch) else {
        return;
    };

    // Generate the rebase prompt
    let prompt = generate_rebase_prompt(branch, &worktree_path);

    // Send prompt to Claude Code terminal in control panel
    if let Some(ref mut term) = app.tabs[0].claude_terminal {
        // Send the prompt followed by Enter
        let _ = term.write(prompt.as_bytes());
        let _ = term.write(b"\r");
    }

    // Focus the Claude pane so user can see the progress
    app.tabs[0].set_control_panel_pane(ControlPanelPane::Claude);
}

/// Show delete worktree dialog for a specific branch.
fn trigger_delete_worktree(app: &mut App, wt_manager: &Option<WorktreeManager>, branch: &str) {
    let Some(manager) = wt_manager else {
        return;
    };

    // Don't delete main branch
    if branch == "main" || branch == "master" {
        return;
    }

    let warnings = manager.remove(branch, false).unwrap_or_default();
    let has_uncommitted = warnings
        .iter()
        .any(|w| matches!(w, RemoveWarning::UncommittedChanges));
    let has_unmerged = warnings
        .iter()
        .any(|w| matches!(w, RemoveWarning::NotMerged { .. }));

    if has_uncommitted {
        app.dialog = Dialog::UncommittedChanges {
            branch: branch.to_string(),
        };
    } else {
        app.dialog = Dialog::ConfirmDelete {
            branch: branch.to_string(),
            unmerged: has_unmerged,
        };
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

    // Pane focus clicks
    if mouse.row >= main_area.y && mouse.row < main_area.y + main_area.height {
        let mid_x = main_area.x + main_area.width / 2;
        if app.current_tab().is_control_panel() {
            // Control panel: left is content, right is Claude
            let pane = if mouse.column < mid_x {
                ControlPanelPane::Content
            } else {
                ControlPanelPane::Claude
            };
            app.current_tab_mut().set_control_panel_pane(pane);
        } else {
            // Terminal tab: left is diff, right is Claude
            let pane = if mouse.column < mid_x {
                Pane::Left
            } else {
                Pane::Right
            };
            app.execute(Command::FocusPane(pane));
        }
    }

    false
}
