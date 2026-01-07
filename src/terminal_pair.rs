//! Terminal pair abstraction for managing left/right terminal panes.
//!
//! Consolidates terminal management logic that was previously duplicated
//! between left and right terminals in the Tab struct.

use ratatui::layout::Rect;

use crate::app::{Pane, inner_area};
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rect(width: u16, height: u16) -> Rect {
        Rect::new(0, 0, width, height)
    }

    // --- new() tests ---

    #[test]
    fn test_new_creates_empty_pair() {
        let pair = TerminalPair::new();
        assert!(pair.get(Pane::Left).is_none());
        assert!(pair.get(Pane::Right).is_none());
    }

    #[test]
    fn test_default_creates_empty_pair() {
        let pair = TerminalPair::default();
        assert!(pair.get(Pane::Left).is_none());
        assert!(pair.get(Pane::Right).is_none());
    }

    // --- needs_left/needs_right tests ---

    #[test]
    fn test_needs_left_when_empty_and_valid_area() {
        let pair = TerminalPair::new();
        // Border takes 2 chars each side, so need > 4 width and > 2 height
        let area = make_rect(80, 24);
        assert!(pair.needs_left(area));
    }

    #[test]
    fn test_needs_right_when_empty_and_valid_area() {
        let pair = TerminalPair::new();
        let area = make_rect(80, 24);
        assert!(pair.needs_right(area));
    }

    #[test]
    fn test_needs_left_false_when_area_too_small() {
        let pair = TerminalPair::new();
        // inner_area subtracts borders, so a 2x2 area has 0x0 inner
        let area = make_rect(2, 2);
        assert!(!pair.needs_left(area));
    }

    #[test]
    fn test_needs_right_false_when_area_too_small() {
        let pair = TerminalPair::new();
        let area = make_rect(2, 2);
        assert!(!pair.needs_right(area));
    }

    #[test]
    fn test_needs_left_false_when_zero_width() {
        let pair = TerminalPair::new();
        let area = make_rect(0, 24);
        assert!(!pair.needs_left(area));
    }

    #[test]
    fn test_needs_right_false_when_zero_height() {
        let pair = TerminalPair::new();
        let area = make_rect(80, 0);
        assert!(!pair.needs_right(area));
    }

    // --- needs_*_resize tests ---

    #[test]
    fn test_needs_right_resize_false_when_no_terminal() {
        let pair = TerminalPair::new();
        let area = make_rect(80, 24);
        assert!(pair.needs_right_resize(area).is_none());
    }

    #[test]
    fn test_needs_left_resize_false_when_no_terminal() {
        let pair = TerminalPair::new();
        let area = make_rect(80, 24);
        assert!(pair.needs_left_resize(area).is_none());
    }

    // --- update_*_size tests ---

    #[test]
    fn test_update_right_size_changes_last_size() {
        let mut pair = TerminalPair::new();
        assert_eq!(pair.last_right_size, (0, 0));

        // Update to new size
        let area = make_rect(100, 50);
        pair.update_right_size(area);

        // inner_area: width - 2, height - 2
        assert_eq!(pair.last_right_size, (98, 48));
    }

    #[test]
    fn test_update_left_size_changes_last_size() {
        let mut pair = TerminalPair::new();
        assert_eq!(pair.last_left_size, (0, 0));

        let area = make_rect(80, 24);
        pair.update_left_size(area);

        assert_eq!(pair.last_left_size, (78, 22));
    }

    // --- get tests ---

    #[test]
    fn test_get_returns_none_for_empty_left() {
        let pair = TerminalPair::new();
        assert!(pair.get(Pane::Left).is_none());
    }

    #[test]
    fn test_get_returns_none_for_empty_right() {
        let pair = TerminalPair::new();
        assert!(pair.get(Pane::Right).is_none());
    }

    #[test]
    fn test_get_mut_returns_none_for_empty_left() {
        let mut pair = TerminalPair::new();
        assert!(pair.get_mut(Pane::Left).is_none());
    }

    #[test]
    fn test_get_mut_returns_none_for_empty_right() {
        let mut pair = TerminalPair::new();
        assert!(pair.get_mut(Pane::Right).is_none());
    }

    // --- scroll tests (no-op when no terminal) ---

    #[test]
    fn test_scroll_noop_when_no_terminal() {
        let mut pair = TerminalPair::new();
        // Should not panic when scrolling with no terminal
        pair.scroll(Pane::Left, 10);
        pair.scroll(Pane::Right, -5);
    }
}
