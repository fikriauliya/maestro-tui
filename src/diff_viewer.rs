//! Git diff viewer module
//!
//! Provides a real-time git diff viewer that shows changes sorted by file modification time.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme;

/// A git diff viewer that shows changes in real-time
pub struct DiffViewer {
    /// Styled lines of diff output
    lines: Vec<Line<'static>>,
    /// Current scroll offset (line index at top of view)
    scroll_offset: u16,
    /// Hash of last diff content (for change detection)
    last_diff_hash: u64,
    /// Path to the worktree directory
    worktree_path: PathBuf,
    /// Total number of lines
    total_lines: u16,
}

impl DiffViewer {
    /// Create a new DiffViewer for the given worktree path
    pub fn new(worktree_path: PathBuf) -> Self {
        let mut viewer = Self {
            lines: Vec::new(),
            scroll_offset: 0,
            last_diff_hash: 0,
            worktree_path,
            total_lines: 0,
        };
        viewer.refresh();
        viewer
    }

    /// Refresh the diff content. Returns true if the diff changed.
    pub fn refresh(&mut self) -> bool {
        // Get quick hash of current diff state
        let new_hash = self.get_diff_hash();
        if new_hash == self.last_diff_hash {
            return false;
        }
        self.last_diff_hash = new_hash;

        // Get changed files sorted by mtime
        let files = self.get_changed_files_sorted();

        if files.is_empty() {
            self.lines = self.empty_state_lines();
            self.total_lines = self.lines.len() as u16;
            return true;
        }

        // Generate styled diff for each file
        let mut lines = Vec::new();
        for file in &files {
            let diff = self.get_file_diff(file);
            lines.extend(self.parse_diff(&diff));
            lines.push(Line::from("")); // Blank line between files
        }

        self.lines = lines;
        self.total_lines = self.lines.len() as u16;

        // Auto-scroll to bottom when diff changes
        // (so newest changes are visible)
        // We'll let the widget method handle the actual scroll position

        true
    }

    /// Scroll by the given delta (positive = down/forward, negative = up/back)
    pub fn scroll(&mut self, delta: i16) {
        if delta < 0 {
            // Scroll up (towards beginning)
            self.scroll_offset = self.scroll_offset.saturating_sub((-delta) as u16);
        } else {
            // Scroll down (towards end) - clamp at total_lines
            let new_offset = self.scroll_offset.saturating_add(delta as u16);
            self.scroll_offset = new_offset.min(self.total_lines.saturating_sub(1));
        }
    }

    /// Get a Paragraph widget for rendering
    pub fn widget(&self, height: u16) -> Paragraph<'static> {
        let start = self.scroll_offset as usize;
        let end = (start + height as usize).min(self.lines.len());

        let visible_lines: Vec<Line<'static>> = if start < self.lines.len() {
            self.lines[start..end].to_vec()
        } else {
            Vec::new()
        };

        Paragraph::new(visible_lines)
    }

    // --- Private methods ---

    /// Get a hash of the current diff state (for change detection)
    fn get_diff_hash(&self) -> u64 {
        let output = Command::new("git")
            .current_dir(&self.worktree_path)
            .args(["diff", "--stat"])
            .output();

        match output {
            Ok(o) => {
                let mut hasher = DefaultHasher::new();
                o.stdout.hash(&mut hasher);
                hasher.finish()
            }
            Err(_) => 0,
        }
    }

    /// Get list of changed files sorted by modification time (oldest first)
    fn get_changed_files_sorted(&self) -> Vec<String> {
        let output = Command::new("git")
            .current_dir(&self.worktree_path)
            .args(["diff", "--name-only"])
            .output();

        let files: Vec<String> = match output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            }
            _ => Vec::new(),
        };

        // Sort by mtime (oldest first, so newest appears at bottom)
        let mut files_with_mtime: Vec<(String, SystemTime)> = files
            .into_iter()
            .map(|f| {
                let path = self.worktree_path.join(&f);
                let mtime = path
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                (f, mtime)
            })
            .collect();

        files_with_mtime.sort_by_key(|(_, mtime)| *mtime);
        files_with_mtime.into_iter().map(|(f, _)| f).collect()
    }

    /// Get diff for a specific file
    fn get_file_diff(&self, file: &str) -> String {
        let output = Command::new("git")
            .current_dir(&self.worktree_path)
            .args(["diff", "--", file])
            .output();

        match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => String::new(),
        }
    }

    /// Parse diff output into styled lines
    fn parse_diff(&self, diff: &str) -> Vec<Line<'static>> {
        diff.lines()
            .map(|line| {
                let line_owned = line.to_string();

                if line.starts_with("diff --git") {
                    // File header - bold cyan
                    Line::from(Span::styled(
                        line_owned,
                        Style::default()
                            .fg(theme::CYAN)
                            .add_modifier(Modifier::BOLD),
                    ))
                } else if line.starts_with("+++") || line.starts_with("---") {
                    // File path lines - dim
                    Line::from(Span::styled(
                        line_owned,
                        Style::default().fg(theme::TX_3),
                    ))
                } else if line.starts_with("@@") {
                    // Hunk header - cyan
                    Line::from(Span::styled(
                        line_owned,
                        Style::default().fg(theme::CYAN),
                    ))
                } else if line.starts_with('+') {
                    // Addition - green
                    Line::from(Span::styled(
                        line_owned,
                        Style::default().fg(theme::GREEN),
                    ))
                } else if line.starts_with('-') {
                    // Deletion - red
                    Line::from(Span::styled(
                        line_owned,
                        Style::default().fg(theme::RED),
                    ))
                } else {
                    // Context line - normal text
                    Line::from(Span::styled(
                        line_owned,
                        Style::default().fg(theme::TX),
                    ))
                }
            })
            .collect()
    }

    /// Generate empty state lines when there are no changes
    fn empty_state_lines(&self) -> Vec<Line<'static>> {
        vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "   No uncommitted changes",
                Style::default().fg(theme::TX_2),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "   Changes will appear here",
                Style::default().fg(theme::TX_3),
            )),
            Line::from(Span::styled(
                "   as you work.",
                Style::default().fg(theme::TX_3),
            )),
        ]
    }

    // --- Test helpers (pub for testing) ---

    #[cfg(test)]
    pub(crate) fn test_parse_diff(&self, diff: &str) -> Vec<Line<'static>> {
        self.parse_diff(diff)
    }

    #[cfg(test)]
    pub(crate) fn test_empty_state_lines(&self) -> Vec<Line<'static>> {
        self.empty_state_lines()
    }

    #[cfg(test)]
    pub(crate) fn set_lines_for_test(&mut self, lines: Vec<Line<'static>>) {
        self.lines = lines;
        self.total_lines = self.lines.len() as u16;
    }

    #[cfg(test)]
    pub(crate) fn get_scroll_offset(&self) -> u16 {
        self.scroll_offset
    }

    #[cfg(test)]
    pub(crate) fn get_total_lines(&self) -> u16 {
        self.total_lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a DiffViewer for testing without running git commands
    fn create_test_viewer() -> DiffViewer {
        DiffViewer {
            lines: Vec::new(),
            scroll_offset: 0,
            last_diff_hash: 0,
            worktree_path: PathBuf::from("/tmp/test"),
            total_lines: 0,
        }
    }

    // --- parse_diff tests ---

    #[test]
    fn test_parse_diff_file_header() {
        let viewer = create_test_viewer();
        let diff = "diff --git a/src/main.rs b/src/main.rs";
        let lines = viewer.test_parse_diff(diff);

        assert_eq!(lines.len(), 1);
        // Check that it's styled with CYAN and BOLD
        let span = &lines[0].spans[0];
        assert!(span.content.contains("diff --git"));
        assert_eq!(span.style.fg, Some(theme::CYAN));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_parse_diff_file_paths() {
        let viewer = create_test_viewer();
        let diff = "--- a/src/main.rs\n+++ b/src/main.rs";
        let lines = viewer.test_parse_diff(diff);

        assert_eq!(lines.len(), 2);
        // Both should be styled with TX_3 (dim)
        assert_eq!(lines[0].spans[0].style.fg, Some(theme::TX_3));
        assert_eq!(lines[1].spans[0].style.fg, Some(theme::TX_3));
    }

    #[test]
    fn test_parse_diff_hunk_header() {
        let viewer = create_test_viewer();
        let diff = "@@ -10,5 +10,7 @@ fn main() {";
        let lines = viewer.test_parse_diff(diff);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].style.fg, Some(theme::CYAN));
    }

    #[test]
    fn test_parse_diff_additions() {
        let viewer = create_test_viewer();
        let diff = "+    let x = 5;";
        let lines = viewer.test_parse_diff(diff);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].style.fg, Some(theme::GREEN));
    }

    #[test]
    fn test_parse_diff_deletions() {
        let viewer = create_test_viewer();
        let diff = "-    let y = 10;";
        let lines = viewer.test_parse_diff(diff);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].style.fg, Some(theme::RED));
    }

    #[test]
    fn test_parse_diff_context_lines() {
        let viewer = create_test_viewer();
        let diff = "     println!(\"Hello\");";
        let lines = viewer.test_parse_diff(diff);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].style.fg, Some(theme::TX));
    }

    #[test]
    fn test_parse_diff_full_diff() {
        let viewer = create_test_viewer();
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    let x = 5;
-    let y = 10;
     println!("Hello");
 }"#;
        let lines = viewer.test_parse_diff(diff);

        assert_eq!(lines.len(), 9);
        // File header (cyan, bold)
        assert_eq!(lines[0].spans[0].style.fg, Some(theme::CYAN));
        // File paths (dim)
        assert_eq!(lines[1].spans[0].style.fg, Some(theme::TX_3));
        assert_eq!(lines[2].spans[0].style.fg, Some(theme::TX_3));
        // Hunk header (cyan)
        assert_eq!(lines[3].spans[0].style.fg, Some(theme::CYAN));
        // Context (normal)
        assert_eq!(lines[4].spans[0].style.fg, Some(theme::TX));
        // Addition (green)
        assert_eq!(lines[5].spans[0].style.fg, Some(theme::GREEN));
        // Deletion (red)
        assert_eq!(lines[6].spans[0].style.fg, Some(theme::RED));
    }

    #[test]
    fn test_parse_diff_empty() {
        let viewer = create_test_viewer();
        let lines = viewer.test_parse_diff("");

        // Empty string produces zero lines (empty iterator)
        assert_eq!(lines.len(), 0);
    }

    // --- scroll tests ---

    #[test]
    fn test_scroll_down() {
        let mut viewer = create_test_viewer();
        // Set up some lines
        viewer.set_lines_for_test(vec![
            Line::from("line 1"),
            Line::from("line 2"),
            Line::from("line 3"),
            Line::from("line 4"),
            Line::from("line 5"),
        ]);

        assert_eq!(viewer.get_scroll_offset(), 0);

        viewer.scroll(2);
        assert_eq!(viewer.get_scroll_offset(), 2);

        viewer.scroll(1);
        assert_eq!(viewer.get_scroll_offset(), 3);
    }

    #[test]
    fn test_scroll_up() {
        let mut viewer = create_test_viewer();
        viewer.set_lines_for_test(vec![
            Line::from("line 1"),
            Line::from("line 2"),
            Line::from("line 3"),
        ]);
        viewer.scroll_offset = 2;

        viewer.scroll(-1);
        assert_eq!(viewer.get_scroll_offset(), 1);

        viewer.scroll(-1);
        assert_eq!(viewer.get_scroll_offset(), 0);
    }

    #[test]
    fn test_scroll_up_clamps_at_zero() {
        let mut viewer = create_test_viewer();
        viewer.set_lines_for_test(vec![Line::from("line 1")]);
        viewer.scroll_offset = 1;

        // Scroll up more than offset
        viewer.scroll(-10);
        assert_eq!(viewer.get_scroll_offset(), 0);
    }

    #[test]
    fn test_scroll_down_clamps_at_max() {
        let mut viewer = create_test_viewer();
        viewer.set_lines_for_test(vec![
            Line::from("line 1"),
            Line::from("line 2"),
            Line::from("line 3"),
        ]);

        // Scroll down more than total lines
        viewer.scroll(100);
        // Should clamp to total_lines - 1 = 2
        assert_eq!(viewer.get_scroll_offset(), 2);
    }

    #[test]
    fn test_scroll_empty_lines() {
        let mut viewer = create_test_viewer();
        // No lines set

        viewer.scroll(5);
        // Should remain at 0 (saturating_sub prevents underflow)
        assert_eq!(viewer.get_scroll_offset(), 0);

        viewer.scroll(-5);
        assert_eq!(viewer.get_scroll_offset(), 0);
    }

    // --- widget tests ---

    #[test]
    fn test_widget_returns_visible_lines() {
        let mut viewer = create_test_viewer();
        viewer.set_lines_for_test(vec![
            Line::from("line 1"),
            Line::from("line 2"),
            Line::from("line 3"),
            Line::from("line 4"),
            Line::from("line 5"),
        ]);

        // Get widget with height 3
        let _widget = viewer.widget(3);
        // Widget should show lines 0-2 (first 3 lines)
        // Note: We can't easily inspect Paragraph contents, but we verify it doesn't panic
    }

    #[test]
    fn test_widget_with_scroll_offset() {
        let mut viewer = create_test_viewer();
        viewer.set_lines_for_test(vec![
            Line::from("line 1"),
            Line::from("line 2"),
            Line::from("line 3"),
            Line::from("line 4"),
            Line::from("line 5"),
        ]);
        viewer.scroll_offset = 2;

        // Get widget with height 2
        let _widget = viewer.widget(2);
        // Widget should show lines 2-3 (lines 3 and 4)
    }

    #[test]
    fn test_widget_scroll_beyond_content() {
        let mut viewer = create_test_viewer();
        viewer.set_lines_for_test(vec![
            Line::from("line 1"),
            Line::from("line 2"),
        ]);
        viewer.scroll_offset = 10; // Beyond content

        // Should not panic, returns empty
        let _widget = viewer.widget(5);
    }

    #[test]
    fn test_widget_empty_lines() {
        let viewer = create_test_viewer();
        // No lines

        let _widget = viewer.widget(10);
        // Should not panic
    }

    // --- empty_state_lines tests ---

    #[test]
    fn test_empty_state_lines_content() {
        let viewer = create_test_viewer();
        let lines = viewer.test_empty_state_lines();

        assert_eq!(lines.len(), 6);
        // Check that the message is present
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(all_text.contains("No uncommitted changes"));
        assert!(all_text.contains("Changes will appear here"));
    }

    #[test]
    fn test_empty_state_lines_styling() {
        let viewer = create_test_viewer();
        let lines = viewer.test_empty_state_lines();

        // Line 2 (index 2) should have TX_2 color for "No uncommitted changes"
        assert_eq!(lines[2].spans[0].style.fg, Some(theme::TX_2));

        // Lines 4-5 should have TX_3 color
        assert_eq!(lines[4].spans[0].style.fg, Some(theme::TX_3));
        assert_eq!(lines[5].spans[0].style.fg, Some(theme::TX_3));
    }

    // --- Integration-style tests ---

    #[test]
    fn test_total_lines_updated_after_set() {
        let mut viewer = create_test_viewer();
        assert_eq!(viewer.get_total_lines(), 0);

        viewer.set_lines_for_test(vec![
            Line::from("a"),
            Line::from("b"),
            Line::from("c"),
        ]);
        assert_eq!(viewer.get_total_lines(), 3);
    }
}
