use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::diff_viewer::DiffViewer;
use crate::input::key_to_bytes;
use crate::terminal::Terminal;
use crate::terminal_pair::TerminalPair;

#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub enum Pane {
    #[default]
    Left,
    Right,
}

/// Dialog state for confirmations and warnings
#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    /// No dialog shown
    None,
    /// Confirm delete worktree (branch name, has unmerged commits)
    ConfirmDelete { branch: String, unmerged: bool },
    /// Cannot delete: has uncommitted changes
    UncommittedChanges { branch: String },
}

/// Pane focus for control panel (content vs Claude terminal)
#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub enum ControlPanelPane {
    /// Content area (worktrees, bd ready, input)
    #[default]
    Content,
    /// Claude terminal pane
    Claude,
}

/// The kind of tab - control panel or worktree terminal
#[derive(Debug, Clone, PartialEq)]
pub enum TabKind {
    /// Control panel with text input for creating new worktrees
    ControlPanel {
        input: String,
        /// Output from `bd ready` command
        bd_ready_output: Vec<String>,
        /// Which pane is focused in the control panel
        focused_pane: ControlPanelPane,
        /// Currently selected worktree index (for merge/remove operations)
        selected_worktree: usize,
    },
    /// Worktree tab with dual terminal panes
    Worktree {
        path: PathBuf,
        branch: String,
        prompt: String,
    },
}

/// Commands that can be executed by the application
#[derive(Debug, PartialEq)]
pub enum Command {
    Quit,
    SwitchTab(usize),
    FocusPane(Pane),
    #[allow(dead_code)] // Used in tests
    TogglePane,
    WriteToTerminal(Vec<u8>),
    /// Update the control panel input text
    UpdateControlPanelInput(char),
    /// Delete last character from control panel input
    DeleteControlPanelChar,
    /// Scroll up (show older content) - half page
    ScrollUp,
    /// Scroll down (show newer content) - half page
    ScrollDown,
    /// Confirm dialog action (yes)
    DialogConfirm,
    /// Cancel dialog action (no)
    DialogCancel,
    /// Reload bd ready output for control panel
    ReloadBdReady(Vec<String>),
    /// Select previous worktree in control panel list
    #[allow(dead_code)]
    SelectPrevWorktree,
    /// Select next worktree in control panel list
    #[allow(dead_code)]
    SelectNextWorktree,
}

pub struct Tab {
    pub kind: TabKind,
    pub focused: Pane,
    pub pair: TerminalPair,
    /// Diff viewer for worktree tabs (replaces left terminal)
    pub diff_viewer: Option<DiffViewer>,
    /// Claude terminal for control panel tab
    pub claude_terminal: Option<Terminal>,
    /// Last size of claude terminal area (for resize detection)
    claude_terminal_size: Option<(u16, u16)>,
}

impl Tab {
    pub fn new() -> Self {
        Self {
            kind: TabKind::ControlPanel {
                input: String::new(),
                bd_ready_output: Vec::new(),
                focused_pane: ControlPanelPane::Content,
                selected_worktree: 0,
            },
            focused: Pane::Left,
            pair: TerminalPair::new(),
            diff_viewer: None,
            claude_terminal: None,
            claude_terminal_size: None,
        }
    }

    #[allow(dead_code)] // Used in tests
    pub fn control_panel() -> Self {
        Self {
            kind: TabKind::ControlPanel {
                input: String::new(),
                bd_ready_output: Vec::new(),
                focused_pane: ControlPanelPane::Content,
                selected_worktree: 0,
            },
            focused: Pane::Left,
            pair: TerminalPair::new(),
            diff_viewer: None,
            claude_terminal: None,
            claude_terminal_size: None,
        }
    }

    pub fn with_worktree(path: PathBuf, branch: String, prompt: String) -> Self {
        // Create diff viewer for the worktree
        let diff_viewer = Some(DiffViewer::new(path.clone()));
        Self {
            kind: TabKind::Worktree {
                path,
                branch,
                prompt,
            },
            focused: Pane::Right,
            pair: TerminalPair::new(),
            diff_viewer,
            claude_terminal: None,
            claude_terminal_size: None,
        }
    }

    /// Check if this is a control panel tab
    pub fn is_control_panel(&self) -> bool {
        matches!(self.kind, TabKind::ControlPanel { .. })
    }

    /// Get worktree path if this is a worktree tab
    pub fn worktree_path(&self) -> Option<&PathBuf> {
        match &self.kind {
            TabKind::Worktree { path, .. } => Some(path),
            _ => None,
        }
    }

    /// Get branch name if this is a worktree tab
    #[allow(dead_code)]
    pub fn branch(&self) -> Option<&str> {
        match &self.kind {
            TabKind::Worktree { branch, .. } => Some(branch),
            _ => None,
        }
    }

    /// Get prompt if this is a worktree tab
    pub fn worktree_prompt(&self) -> Option<&str> {
        match &self.kind {
            TabKind::Worktree { prompt, .. } => Some(prompt),
            _ => None,
        }
    }

    /// Scroll the focused pane by the given number of lines (positive = up, negative = down)
    /// For left pane (diff viewer), scrolls the diff. For right pane, scrolls the terminal.
    pub fn scroll_focused(&mut self, lines: i32) {
        match self.focused {
            Pane::Left => {
                // Scroll diff viewer
                if let Some(ref mut dv) = self.diff_viewer {
                    // Convert: positive lines = scroll up (show older), negative = scroll down (show newer)
                    // For diff viewer: positive delta scrolls down in the view
                    dv.scroll(lines as i16);
                }
            }
            Pane::Right => {
                // Scroll terminal
                self.pair.scroll(self.focused, lines);
            }
        }
    }

    /// Refresh the diff viewer if present. Returns true if the diff changed.
    pub fn refresh_diff_viewer(&mut self) -> bool {
        if let Some(ref mut dv) = self.diff_viewer {
            dv.refresh()
        } else {
            false
        }
    }

    /// Ensure left terminal exists and is properly sized.
    /// Creates a shell terminal if needed, or resizes if dimensions changed.
    /// Note: Currently unused as left pane shows diff viewer, but kept for potential toggle feature.
    #[allow(dead_code)]
    pub fn ensure_left_terminal(&mut self, area: Rect) {
        if self.pair.needs_left(area) {
            let inner = inner_area(area);
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            let term_result = if let Some(cwd) = self.worktree_path() {
                Terminal::with_command_in_dir(inner.width, inner.height, &shell, &[], cwd)
            } else {
                Terminal::new(inner.width, inner.height)
            };
            if let Ok(term) = term_result {
                self.pair.set_left(term, area);
            }
        } else if let Some((cols, rows)) = self.pair.needs_left_resize(area)
            && let Some(term) = self.pair.get_mut(Pane::Left)
        {
            term.resize(cols, rows);
            self.pair.update_left_size(area);
        }
    }

    /// Ensure right terminal exists and is properly sized.
    /// Creates a Claude terminal if needed, or resizes if dimensions changed.
    pub fn ensure_right_terminal(&mut self, area: Rect) {
        if self.pair.needs_right(area) {
            let inner = inner_area(area);
            let prompt = self.worktree_prompt().unwrap_or("");
            let args: Vec<&str> = if !prompt.is_empty() {
                vec![prompt]
            } else {
                vec![]
            };
            let term_result = if let Some(cwd) = self.worktree_path() {
                Terminal::with_command_in_dir(inner.width, inner.height, "claude", &args, cwd)
            } else {
                Terminal::with_command(inner.width, inner.height, "claude", &args)
            };
            if let Ok(term) = term_result {
                self.pair.set_right(term, area);
            }
        } else if let Some((cols, rows)) = self.pair.needs_right_resize(area)
            && let Some(term) = self.pair.get_mut(Pane::Right)
        {
            term.resize(cols, rows);
            self.pair.update_right_size(area);
        }
    }

    /// Ensure Claude terminal exists for control panel and is properly sized.
    /// Creates a Claude terminal if needed, or resizes if dimensions changed.
    pub fn ensure_claude_terminal(&mut self, area: Rect) {
        let inner = inner_area(area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let needs_create = self.claude_terminal.is_none()
            || self.claude_terminal_size != Some((area.width, area.height));

        if needs_create && self.claude_terminal.is_none() {
            // Create new Claude terminal
            if let Ok(term) = Terminal::with_command(inner.width, inner.height, "claude", &[]) {
                self.claude_terminal = Some(term);
                self.claude_terminal_size = Some((area.width, area.height));
            }
        } else if let Some((old_w, old_h)) = self.claude_terminal_size
            && (old_w != area.width || old_h != area.height)
            && let Some(ref mut term) = self.claude_terminal
        {
            // Resize existing terminal
            term.resize(inner.width, inner.height);
            self.claude_terminal_size = Some((area.width, area.height));
        }
    }

    /// Get the control panel focused pane
    pub fn control_panel_pane(&self) -> ControlPanelPane {
        match &self.kind {
            TabKind::ControlPanel { focused_pane, .. } => *focused_pane,
            _ => ControlPanelPane::Content,
        }
    }

    /// Set the control panel focused pane
    pub fn set_control_panel_pane(&mut self, pane: ControlPanelPane) {
        if let TabKind::ControlPanel {
            ref mut focused_pane,
            ..
        } = self.kind
        {
            *focused_pane = pane;
        }
    }

    /// Toggle control panel pane focus
    #[allow(dead_code)]
    pub fn toggle_control_panel_pane(&mut self) {
        if let TabKind::ControlPanel {
            ref mut focused_pane,
            ..
        } = self.kind
        {
            *focused_pane = match focused_pane {
                ControlPanelPane::Content => ControlPanelPane::Claude,
                ControlPanelPane::Claude => ControlPanelPane::Content,
            };
        }
    }

    /// Get the selected worktree index (for control panel)
    pub fn selected_worktree(&self) -> usize {
        match &self.kind {
            TabKind::ControlPanel {
                selected_worktree, ..
            } => *selected_worktree,
            _ => 0,
        }
    }

    /// Select previous worktree in the list
    pub fn select_prev_worktree(&mut self) {
        if let TabKind::ControlPanel {
            ref mut selected_worktree,
            ..
        } = self.kind
        {
            *selected_worktree = selected_worktree.saturating_sub(1);
        }
    }

    /// Select next worktree in the list (needs max count)
    pub fn select_next_worktree(&mut self, max_count: usize) {
        if let TabKind::ControlPanel {
            ref mut selected_worktree,
            ..
        } = self.kind
            && max_count > 0
            && *selected_worktree < max_count - 1
        {
            *selected_worktree += 1;
        }
    }
}

impl Default for Tab {
    fn default() -> Self {
        Self::new()
    }
}

pub struct App {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub dialog: Dialog,
}

impl App {
    pub fn new() -> Self {
        Self {
            tabs: vec![Tab::new()],
            active_tab: 0,
            dialog: Dialog::None,
        }
    }

    pub fn current_tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    pub fn current_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }

    /// Execute a command, mutating app state as needed
    pub fn execute(&mut self, cmd: Command) {
        match cmd {
            Command::Quit => {
                // Handled in main event loop
            }
            Command::SwitchTab(idx) => {
                if idx < self.tabs.len() {
                    self.active_tab = idx;
                }
            }
            Command::FocusPane(pane) => {
                self.current_tab_mut().focused = pane;
            }
            Command::TogglePane => {
                let tab = self.current_tab_mut();
                tab.focused = match tab.focused {
                    Pane::Left => Pane::Right,
                    Pane::Right => Pane::Left,
                };
            }
            Command::WriteToTerminal(bytes) => {
                let tab = self.current_tab_mut();
                if let Some(term) = tab.pair.get_mut(tab.focused) {
                    let _ = term.write(&bytes);
                }
            }
            Command::UpdateControlPanelInput(c) => {
                if let TabKind::ControlPanel { ref mut input, .. } = self.current_tab_mut().kind {
                    input.push(c);
                }
            }
            Command::DeleteControlPanelChar => {
                if let TabKind::ControlPanel { ref mut input, .. } = self.current_tab_mut().kind {
                    input.pop();
                }
            }
            Command::ReloadBdReady(output) => {
                if let TabKind::ControlPanel {
                    ref mut bd_ready_output,
                    ..
                } = self.current_tab_mut().kind
                {
                    *bd_ready_output = output;
                }
            }
            Command::ScrollUp => {
                // Scroll up half a page (show older content)
                self.current_tab_mut().scroll_focused(15);
            }
            Command::ScrollDown => {
                // Scroll down half a page (show newer content)
                self.current_tab_mut().scroll_focused(-15);
            }
            Command::DialogConfirm => {
                // Handled in main event loop
            }
            Command::DialogCancel => {
                self.dialog = Dialog::None;
            }
            Command::SelectPrevWorktree | Command::SelectNextWorktree => {
                // Handled directly in event handler (needs worktree count)
            }
        }
    }

    /// Get the control panel input if current tab is control panel
    #[allow(dead_code)] // Used in tests
    pub fn get_control_panel_input(&self) -> Option<&str> {
        match &self.current_tab().kind {
            TabKind::ControlPanel { input, .. } => Some(input),
            _ => None,
        }
    }

    /// Take the control panel input (clears it) - used when submitting
    pub fn take_control_panel_input(&mut self) -> Option<String> {
        match &mut self.current_tab_mut().kind {
            TabKind::ControlPanel { input, .. } => {
                let prompt = std::mem::take(input);
                if prompt.is_empty() {
                    None
                } else {
                    Some(prompt)
                }
            }
            _ => None,
        }
    }

    /// Get the bd ready output from the control panel tab (tab 0)
    pub fn get_bd_ready_output(&self) -> &[String] {
        match &self.tabs[0].kind {
            TabKind::ControlPanel {
                bd_ready_output, ..
            } => bd_ready_output,
            _ => &[],
        }
    }

    /// Add a new worktree tab and switch to it, returns the new tab index
    pub fn add_worktree_tab(&mut self, path: PathBuf, branch: String, prompt: String) -> usize {
        self.tabs.push(Tab::with_worktree(path, branch, prompt));
        let new_idx = self.tabs.len() - 1;
        self.active_tab = new_idx;
        new_idx
    }

    /// Remove a tab by index, returns true if removed
    /// Cannot remove tab 0 (control panel) or if only one tab remains
    pub fn remove_tab(&mut self, idx: usize) -> bool {
        if idx == 0 || idx >= self.tabs.len() || self.tabs.len() <= 1 {
            return false;
        }
        self.tabs.remove(idx);
        // Adjust active tab if needed
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if self.active_tab > idx {
            self.active_tab -= 1;
        }
        true
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate inner area (inside border) from outer area
pub fn inner_area(area: Rect) -> Rect {
    Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

/// Handle a key event for terminal tabs (passthrough model with Alt shortcuts)
pub fn handle_key(key: &KeyEvent) -> Option<Command> {
    // Check for Alt+key shortcuts (all app shortcuts use Alt)
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            // Alt+0 switches to tab 0 (control panel)
            KeyCode::Char('0') => return Some(Command::SwitchTab(0)),
            // Alt+1-9 switches to tabs 1-9
            KeyCode::Char(c @ '1'..='9') => {
                let tab_idx = (c as usize) - ('0' as usize);
                return Some(Command::SwitchTab(tab_idx));
            }
            // Alt+h focuses left pane
            KeyCode::Char('h') => return Some(Command::FocusPane(Pane::Left)),
            // Alt+l focuses right pane
            KeyCode::Char('l') => return Some(Command::FocusPane(Pane::Right)),
            // Alt+u scrolls up (vim-style half page up)
            KeyCode::Char('u') => return Some(Command::ScrollUp),
            // Alt+d scrolls down (vim-style half page down)
            KeyCode::Char('d') => return Some(Command::ScrollDown),
            // Alt+q quits the application
            KeyCode::Char('q') => return Some(Command::Quit),
            _ => {}
        }
        return None;
    }

    // Ctrl keys pass through to terminal (Ctrl+C, Ctrl+Z, etc.)
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char(c) if c.is_ascii_alphabetic() => {
                let bytes = key_to_bytes(key);
                if !bytes.is_empty() {
                    return Some(Command::WriteToTerminal(bytes));
                }
            }
            _ => {}
        }
        return None;
    }

    // Non-Ctrl keys pass through to terminal
    let bytes = key_to_bytes(key);
    if !bytes.is_empty() {
        Some(Command::WriteToTerminal(bytes))
    } else {
        None
    }
}

/// Handle a key event when a dialog is shown
/// Returns Some(command) if the key was handled, None otherwise
pub fn handle_dialog_key(key: &KeyEvent) -> Option<Command> {
    match key.code {
        // Y or Enter confirms
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(Command::DialogConfirm),
        // N or Escape cancels
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(Command::DialogCancel),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn make_alt_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::ALT)
    }

    // inner_area tests
    #[test]
    fn test_inner_area_normal() {
        let area = Rect::new(0, 0, 80, 24);
        let inner = inner_area(area);
        assert_eq!(inner.x, 1);
        assert_eq!(inner.y, 1);
        assert_eq!(inner.width, 78);
        assert_eq!(inner.height, 22);
    }

    #[test]
    fn test_inner_area_small() {
        let area = Rect::new(5, 5, 4, 4);
        let inner = inner_area(area);
        assert_eq!(inner.x, 6);
        assert_eq!(inner.y, 6);
        assert_eq!(inner.width, 2);
        assert_eq!(inner.height, 2);
    }

    #[test]
    fn test_inner_area_too_small() {
        let area = Rect::new(0, 0, 1, 1);
        let inner = inner_area(area);
        assert_eq!(inner.width, 0);
        assert_eq!(inner.height, 0);
    }

    #[test]
    fn test_inner_area_zero() {
        let area = Rect::new(0, 0, 0, 0);
        let inner = inner_area(area);
        assert_eq!(inner.width, 0);
        assert_eq!(inner.height, 0);
    }

    // handle_key tests (all shortcuts use Alt)
    #[test]
    fn test_alt_switch_tab() {
        assert_eq!(
            handle_key(&make_alt_key(KeyCode::Char('0'))),
            Some(Command::SwitchTab(0))
        );
        assert_eq!(
            handle_key(&make_alt_key(KeyCode::Char('1'))),
            Some(Command::SwitchTab(1))
        );
        assert_eq!(
            handle_key(&make_alt_key(KeyCode::Char('5'))),
            Some(Command::SwitchTab(5))
        );
        assert_eq!(
            handle_key(&make_alt_key(KeyCode::Char('9'))),
            Some(Command::SwitchTab(9))
        );
    }

    #[test]
    fn test_alt_focus_pane() {
        assert_eq!(
            handle_key(&make_alt_key(KeyCode::Char('h'))),
            Some(Command::FocusPane(Pane::Left))
        );
        assert_eq!(
            handle_key(&make_alt_key(KeyCode::Char('l'))),
            Some(Command::FocusPane(Pane::Right))
        );
    }

    #[test]
    fn test_alt_quit() {
        assert_eq!(
            handle_key(&make_alt_key(KeyCode::Char('q'))),
            Some(Command::Quit)
        );
    }

    #[test]
    fn test_dialog_key_confirm() {
        assert_eq!(
            handle_dialog_key(&make_key(KeyCode::Char('y'))),
            Some(Command::DialogConfirm)
        );
        assert_eq!(
            handle_dialog_key(&make_key(KeyCode::Char('Y'))),
            Some(Command::DialogConfirm)
        );
        assert_eq!(
            handle_dialog_key(&make_key(KeyCode::Enter)),
            Some(Command::DialogConfirm)
        );
    }

    #[test]
    fn test_dialog_key_cancel() {
        assert_eq!(
            handle_dialog_key(&make_key(KeyCode::Char('n'))),
            Some(Command::DialogCancel)
        );
        assert_eq!(
            handle_dialog_key(&make_key(KeyCode::Char('N'))),
            Some(Command::DialogCancel)
        );
        assert_eq!(
            handle_dialog_key(&make_key(KeyCode::Esc)),
            Some(Command::DialogCancel)
        );
    }

    // handle_key tests (passthrough)
    #[test]
    fn test_passthrough_char() {
        assert_eq!(
            handle_key(&make_key(KeyCode::Char('a'))),
            Some(Command::WriteToTerminal(b"a".to_vec()))
        );
    }

    #[test]
    fn test_passthrough_enter() {
        assert_eq!(
            handle_key(&make_key(KeyCode::Enter)),
            Some(Command::WriteToTerminal(b"\r".to_vec()))
        );
    }

    #[test]
    fn test_passthrough_escape() {
        assert_eq!(
            handle_key(&make_key(KeyCode::Esc)),
            Some(Command::WriteToTerminal(b"\x1b".to_vec()))
        );
    }

    // App state tests
    #[test]
    fn test_app_new() {
        let app = App::new();
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active_tab, 0);
    }

    #[test]
    fn test_app_add_worktree_tab() {
        let mut app = App::new();
        let idx = app.add_worktree_tab(
            PathBuf::from("/tmp/test"),
            "test-branch".to_string(),
            "test prompt".to_string(),
        );
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(idx, 1);
        assert_eq!(app.active_tab, 1);
    }

    #[test]
    fn test_app_remove_tab() {
        let mut app = App::new();
        app.add_worktree_tab(
            PathBuf::from("/tmp/test1"),
            "branch1".to_string(),
            String::new(),
        );
        app.add_worktree_tab(
            PathBuf::from("/tmp/test2"),
            "branch2".to_string(),
            String::new(),
        );
        assert_eq!(app.tabs.len(), 3);
        assert_eq!(app.active_tab, 2);

        // Remove current tab (tab 2)
        assert!(app.remove_tab(2));
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.active_tab, 1); // Should adjust to last tab

        // Cannot remove tab 0 (control panel)
        assert!(!app.remove_tab(0));
        assert_eq!(app.tabs.len(), 2);

        // Remove tab 1
        assert!(app.remove_tab(1));
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active_tab, 0);

        // Cannot remove last tab
        assert!(!app.remove_tab(0));
    }

    #[test]
    fn test_app_switch_tab() {
        let mut app = App::new();
        app.add_worktree_tab(
            PathBuf::from("/tmp/test1"),
            "branch1".to_string(),
            String::new(),
        );
        app.add_worktree_tab(
            PathBuf::from("/tmp/test2"),
            "branch2".to_string(),
            String::new(),
        );
        assert_eq!(app.active_tab, 2);

        app.execute(Command::SwitchTab(0));
        assert_eq!(app.active_tab, 0);

        // Out of bounds - no change
        app.execute(Command::SwitchTab(10));
        assert_eq!(app.active_tab, 0);
    }

    #[test]
    fn test_app_focus_pane() {
        let mut app = App::new();
        assert_eq!(app.current_tab().focused, Pane::Left);

        app.execute(Command::FocusPane(Pane::Right));
        assert_eq!(app.current_tab().focused, Pane::Right);

        app.execute(Command::FocusPane(Pane::Left));
        assert_eq!(app.current_tab().focused, Pane::Left);
    }

    #[test]
    fn test_app_toggle_pane() {
        let mut app = App::new();
        assert_eq!(app.current_tab().focused, Pane::Left);

        app.execute(Command::TogglePane);
        assert_eq!(app.current_tab().focused, Pane::Right);

        app.execute(Command::TogglePane);
        assert_eq!(app.current_tab().focused, Pane::Left);
    }

    // Control panel tests
    #[test]
    fn test_control_panel_input() {
        let mut app = App::new();
        assert!(app.current_tab().is_control_panel());

        app.execute(Command::UpdateControlPanelInput('h'));
        app.execute(Command::UpdateControlPanelInput('i'));
        assert_eq!(app.get_control_panel_input(), Some("hi"));

        app.execute(Command::DeleteControlPanelChar);
        assert_eq!(app.get_control_panel_input(), Some("h"));
    }

    #[test]
    fn test_take_control_panel_input() {
        let mut app = App::new();
        app.execute(Command::UpdateControlPanelInput('t'));
        app.execute(Command::UpdateControlPanelInput('e'));
        app.execute(Command::UpdateControlPanelInput('s'));
        app.execute(Command::UpdateControlPanelInput('t'));

        let input = app.take_control_panel_input();
        assert_eq!(input, Some("test".to_string()));
        assert_eq!(app.get_control_panel_input(), Some(""));
    }

    // Tab tests
    #[test]
    fn test_tab_needs_terminal() {
        let tab = Tab::with_worktree(PathBuf::from("/tmp"), "test".to_string(), String::new());
        let area = Rect::new(0, 0, 80, 24);
        assert!(tab.pair.needs_left(area));
        assert!(tab.pair.needs_right(area));
    }

    #[test]
    fn test_tab_needs_terminal_zero_size() {
        let tab = Tab::with_worktree(PathBuf::from("/tmp"), "test".to_string(), String::new());
        let area = Rect::new(0, 0, 2, 2); // inner would be 0x0
        assert!(!tab.pair.needs_left(area));
    }

    #[test]
    fn test_tab_kind_control_panel() {
        let tab = Tab::control_panel();
        assert!(tab.is_control_panel());
        assert!(tab.worktree_path().is_none());
        assert!(tab.branch().is_none());
    }

    #[test]
    fn test_tab_kind_worktree() {
        let tab = Tab::with_worktree(
            PathBuf::from("/tmp/test"),
            "feature".to_string(),
            "my prompt".to_string(),
        );
        assert!(!tab.is_control_panel());
        assert_eq!(tab.worktree_path(), Some(&PathBuf::from("/tmp/test")));
        assert_eq!(tab.branch(), Some("feature"));
        assert_eq!(tab.worktree_prompt(), Some("my prompt"));
    }
}
