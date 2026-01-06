use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::input::key_to_bytes;
use crate::terminal::Terminal;

#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub enum Pane {
    #[default]
    Left,
    Right,
}

/// The kind of tab - control panel or worktree terminal
#[derive(Debug, Clone, PartialEq)]
pub enum TabKind {
    /// Control panel with text input for creating new worktrees
    ControlPanel { input: String },
    /// Worktree tab with dual terminal panes
    Worktree { path: PathBuf, branch: String },
}

/// Commands that can be executed by the application
#[derive(Debug, PartialEq)]
pub enum Command {
    Quit,
    SwitchTab(usize),
    FocusPane(Pane),
    TogglePane,
    WriteToTerminal(Vec<u8>),
    /// Update the control panel input text
    UpdateControlPanelInput(char),
    /// Delete last character from control panel input
    DeleteControlPanelChar,
}

pub struct Tab {
    pub kind: TabKind,
    pub focused: Pane,
    pub left_term: Option<Terminal>,
    pub right_term: Option<Terminal>,
    last_left_size: (u16, u16),
    last_right_size: (u16, u16),
}

impl Tab {
    pub fn new() -> Self {
        Self {
            kind: TabKind::ControlPanel { input: String::new() },
            focused: Pane::Left,
            left_term: None,
            right_term: None,
            last_left_size: (0, 0),
            last_right_size: (0, 0),
        }
    }

    pub fn control_panel() -> Self {
        Self {
            kind: TabKind::ControlPanel { input: String::new() },
            focused: Pane::Left,
            left_term: None,
            right_term: None,
            last_left_size: (0, 0),
            last_right_size: (0, 0),
        }
    }

    pub fn with_worktree(path: PathBuf, branch: String) -> Self {
        Self {
            kind: TabKind::Worktree { path, branch },
            focused: Pane::Left,
            left_term: None,
            right_term: None,
            last_left_size: (0, 0),
            last_right_size: (0, 0),
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
    pub fn branch(&self) -> Option<&str> {
        match &self.kind {
            TabKind::Worktree { branch, .. } => Some(branch),
            _ => None,
        }
    }

    /// Check if left terminal needs to be created
    pub fn needs_left_terminal(&self, area: Rect) -> bool {
        let inner = inner_area(area);
        self.left_term.is_none() && inner.width > 0 && inner.height > 0
    }

    /// Check if right terminal needs to be created
    pub fn needs_right_terminal(&self, area: Rect) -> bool {
        let inner = inner_area(area);
        self.right_term.is_none() && inner.width > 0 && inner.height > 0
    }

    /// Check if left terminal needs resize, returns new size if so
    pub fn needs_left_resize(&self, area: Rect) -> Option<(u16, u16)> {
        let inner = inner_area(area);
        let size = (inner.width, inner.height);
        if self.left_term.is_some() && self.last_left_size != size && size.0 > 0 && size.1 > 0 {
            Some(size)
        } else {
            None
        }
    }

    /// Check if right terminal needs resize, returns new size if so
    pub fn needs_right_resize(&self, area: Rect) -> Option<(u16, u16)> {
        let inner = inner_area(area);
        let size = (inner.width, inner.height);
        if self.right_term.is_some() && self.last_right_size != size && size.0 > 0 && size.1 > 0 {
            Some(size)
        } else {
            None
        }
    }

    /// Set the left terminal and track its size
    pub fn set_left_terminal(&mut self, term: Terminal, area: Rect) {
        let inner = inner_area(area);
        self.left_term = Some(term);
        self.last_left_size = (inner.width, inner.height);
    }

    /// Set the right terminal and track its size
    pub fn set_right_terminal(&mut self, term: Terminal, area: Rect) {
        let inner = inner_area(area);
        self.right_term = Some(term);
        self.last_right_size = (inner.width, inner.height);
    }

    /// Update left terminal size tracking after resize
    pub fn update_left_size(&mut self, area: Rect) {
        let inner = inner_area(area);
        self.last_left_size = (inner.width, inner.height);
    }

    /// Update right terminal size tracking after resize
    pub fn update_right_size(&mut self, area: Rect) {
        let inner = inner_area(area);
        self.last_right_size = (inner.width, inner.height);
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
}

impl App {
    pub fn new() -> Self {
        Self {
            tabs: vec![Tab::new()],
            active_tab: 0,
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
                let term = match tab.focused {
                    Pane::Left => tab.left_term.as_mut(),
                    Pane::Right => tab.right_term.as_mut(),
                };
                if let Some(term) = term {
                    let _ = term.write(&bytes);
                }
            }
            Command::UpdateControlPanelInput(c) => {
                if let TabKind::ControlPanel { ref mut input } = self.current_tab_mut().kind {
                    input.push(c);
                }
            }
            Command::DeleteControlPanelChar => {
                if let TabKind::ControlPanel { ref mut input } = self.current_tab_mut().kind {
                    input.pop();
                }
            }
        }
    }

    /// Get the control panel input if current tab is control panel
    pub fn get_control_panel_input(&self) -> Option<&str> {
        match &self.current_tab().kind {
            TabKind::ControlPanel { input } => Some(input),
            _ => None,
        }
    }

    /// Take the control panel input (clears it) - used when submitting
    pub fn take_control_panel_input(&mut self) -> Option<String> {
        match &mut self.current_tab_mut().kind {
            TabKind::ControlPanel { input } => {
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

    /// Add a new worktree tab and switch to it, returns the new tab index
    pub fn add_worktree_tab(&mut self, path: PathBuf, branch: String) -> usize {
        self.tabs.push(Tab::with_worktree(path, branch));
        let new_idx = self.tabs.len() - 1;
        self.active_tab = new_idx;
        new_idx
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

/// Handle a key event for terminal tabs (passthrough model with Ctrl shortcuts)
pub fn handle_key(key: &KeyEvent) -> Option<Command> {
    // Check for Ctrl+key shortcuts
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            // Ctrl+0 switches to tab 0 (control panel)
            KeyCode::Char('0') => return Some(Command::SwitchTab(0)),
            // Ctrl+1-9 switches to tabs 1-9
            KeyCode::Char(c @ '1'..='9') => {
                let tab_idx = (c as usize) - ('0' as usize);
                return Some(Command::SwitchTab(tab_idx));
            }
            // Ctrl+h focuses left pane
            KeyCode::Char('h') => return Some(Command::FocusPane(Pane::Left)),
            // Ctrl+l focuses right pane
            KeyCode::Char('l') => return Some(Command::FocusPane(Pane::Right)),
            // Ctrl+x quits the application
            KeyCode::Char('x') => return Some(Command::Quit),
            // Ctrl+Tab toggles panes
            KeyCode::Tab => return Some(Command::TogglePane),
            // Pass through recognized Ctrl sequences (Ctrl+C, Ctrl+Z, etc.)
            KeyCode::Char(c) if c.is_ascii_alphabetic() => {
                let bytes = key_to_bytes(key);
                if !bytes.is_empty() {
                    return Some(Command::WriteToTerminal(bytes));
                }
            }
            _ => {}
        }
        // Unrecognized Ctrl combos (like Ctrl+@, Ctrl+[) - do nothing
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn make_ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
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

    // handle_key tests (Ctrl shortcuts)
    #[test]
    fn test_ctrl_switch_tab() {
        assert_eq!(
            handle_key(&make_ctrl_key(KeyCode::Char('0'))),
            Some(Command::SwitchTab(0))
        );
        assert_eq!(
            handle_key(&make_ctrl_key(KeyCode::Char('1'))),
            Some(Command::SwitchTab(1))
        );
        assert_eq!(
            handle_key(&make_ctrl_key(KeyCode::Char('5'))),
            Some(Command::SwitchTab(5))
        );
        assert_eq!(
            handle_key(&make_ctrl_key(KeyCode::Char('9'))),
            Some(Command::SwitchTab(9))
        );
    }

    #[test]
    fn test_ctrl_focus_pane() {
        assert_eq!(
            handle_key(&make_ctrl_key(KeyCode::Char('h'))),
            Some(Command::FocusPane(Pane::Left))
        );
        assert_eq!(
            handle_key(&make_ctrl_key(KeyCode::Char('l'))),
            Some(Command::FocusPane(Pane::Right))
        );
    }

    #[test]
    fn test_ctrl_toggle_pane() {
        assert_eq!(
            handle_key(&make_ctrl_key(KeyCode::Tab)),
            Some(Command::TogglePane)
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
        let idx = app.add_worktree_tab(PathBuf::from("/tmp/test"), "test-branch".to_string());
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(idx, 1);
        assert_eq!(app.active_tab, 1);
    }

    #[test]
    fn test_app_switch_tab() {
        let mut app = App::new();
        app.add_worktree_tab(PathBuf::from("/tmp/test1"), "branch1".to_string());
        app.add_worktree_tab(PathBuf::from("/tmp/test2"), "branch2".to_string());
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
        let tab = Tab::with_worktree(PathBuf::from("/tmp"), "test".to_string());
        let area = Rect::new(0, 0, 80, 24);
        assert!(tab.needs_left_terminal(area));
        assert!(tab.needs_right_terminal(area));
    }

    #[test]
    fn test_tab_needs_terminal_zero_size() {
        let tab = Tab::with_worktree(PathBuf::from("/tmp"), "test".to_string());
        let area = Rect::new(0, 0, 2, 2); // inner would be 0x0
        assert!(!tab.needs_left_terminal(area));
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
        let tab = Tab::with_worktree(PathBuf::from("/tmp/test"), "feature".to_string());
        assert!(!tab.is_control_panel());
        assert_eq!(tab.worktree_path(), Some(&PathBuf::from("/tmp/test")));
        assert_eq!(tab.branch(), Some("feature"));
    }
}
