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
}
