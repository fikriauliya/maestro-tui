//! Terminal pair abstraction for managing left/right terminal panes.
//!
//! Consolidates terminal management logic that was previously duplicated
//! between left and right terminals in the Tab struct.

use ratatui::layout::Rect;

use crate::app::{inner_area, Pane};
use crate::terminal::Terminal;

/// A pair of terminals (left and right panes) with unified management.
/// Note: Left terminal methods currently unused as left pane shows diff viewer.
#[derive(Default)]
pub struct TerminalPair {
    #[allow(dead_code)]
    left: Option<Terminal>,
    right: Option<Terminal>,
    #[allow(dead_code)]
    last_left_size: (u16, u16),
    last_right_size: (u16, u16),
}

impl TerminalPair {
    /// Create a new empty terminal pair.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a reference to the terminal for the given pane.
    pub fn get(&self, pane: Pane) -> Option<&Terminal> {
        match pane {
            Pane::Left => self.left.as_ref(),
            Pane::Right => self.right.as_ref(),
        }
    }

    /// Get a mutable reference to the terminal for the given pane.
    pub fn get_mut(&mut self, pane: Pane) -> Option<&mut Terminal> {
        match pane {
            Pane::Left => self.left.as_mut(),
            Pane::Right => self.right.as_mut(),
        }
    }

    /// Check if the left terminal needs to be created.
    #[allow(dead_code)]
    pub fn needs_left(&self, area: Rect) -> bool {
        let inner = inner_area(area);
        self.left.is_none() && inner.width > 0 && inner.height > 0
    }

    /// Check if the right terminal needs to be created.
    pub fn needs_right(&self, area: Rect) -> bool {
        let inner = inner_area(area);
        self.right.is_none() && inner.width > 0 && inner.height > 0
    }

    /// Check if left terminal needs resize, returns new size if so.
    #[allow(dead_code)]
    pub fn needs_left_resize(&self, area: Rect) -> Option<(u16, u16)> {
        let inner = inner_area(area);
        let size = (inner.width, inner.height);
        if self.left.is_some() && self.last_left_size != size && size.0 > 0 && size.1 > 0 {
            Some(size)
        } else {
            None
        }
    }

    /// Check if right terminal needs resize, returns new size if so.
    pub fn needs_right_resize(&self, area: Rect) -> Option<(u16, u16)> {
        let inner = inner_area(area);
        let size = (inner.width, inner.height);
        if self.right.is_some() && self.last_right_size != size && size.0 > 0 && size.1 > 0 {
            Some(size)
        } else {
            None
        }
    }

    /// Set the left terminal and track its size.
    #[allow(dead_code)]
    pub fn set_left(&mut self, term: Terminal, area: Rect) {
        let inner = inner_area(area);
        self.left = Some(term);
        self.last_left_size = (inner.width, inner.height);
    }

    /// Set the right terminal and track its size.
    pub fn set_right(&mut self, term: Terminal, area: Rect) {
        let inner = inner_area(area);
        self.right = Some(term);
        self.last_right_size = (inner.width, inner.height);
    }

    /// Update left terminal size tracking after resize.
    #[allow(dead_code)]
    pub fn update_left_size(&mut self, area: Rect) {
        let inner = inner_area(area);
        self.last_left_size = (inner.width, inner.height);
    }

    /// Update right terminal size tracking after resize.
    pub fn update_right_size(&mut self, area: Rect) {
        let inner = inner_area(area);
        self.last_right_size = (inner.width, inner.height);
    }

    /// Scroll the terminal for the given pane by the specified number of lines.
    /// Positive = scroll up (older content), negative = scroll down (newer content).
    pub fn scroll(&mut self, pane: Pane, lines: i32) {
        if let Some(term) = self.get_mut(pane) {
            term.scroll(lines);
        }
    }
}
