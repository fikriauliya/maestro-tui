//! File system watcher for git diff updates
//!
//! Uses inotify (Linux) or similar mechanisms to watch for file changes
//! in worktree directories, triggering diff refreshes only when needed.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebouncedEvent, DebouncedEventKind, Debouncer, new_debouncer};

/// Events from the file watcher
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// A file changed in the given worktree path
    FileChanged(PathBuf),
}

/// File watcher that monitors worktree directories for changes
pub struct FileWatcher {
    /// Debounced watcher instance
    _debouncer: Debouncer<RecommendedWatcher>,
    /// Receiver for watch events
    event_rx: Receiver<WatchEvent>,
    /// Set of currently watched paths
    watched_paths: HashSet<PathBuf>,
}

impl FileWatcher {
    /// Create a new file watcher with debouncing
    pub fn new() -> Result<Self, notify::Error> {
        let (tx, event_rx) = mpsc::channel();

        // Create debounced watcher with 300ms debounce time
        // This prevents excessive refreshes when many files change rapidly
        let debouncer = new_debouncer(
            Duration::from_millis(300),
            move |res: Result<Vec<DebouncedEvent>, notify::Error>| {
                if let Ok(events) = res {
                    for event in events {
                        if event.kind == DebouncedEventKind::Any {
                            // Find the worktree root for this path
                            if let Some(path) = find_worktree_root(&event.path) {
                                let _ = tx.send(WatchEvent::FileChanged(path));
                            }
                        }
                    }
                }
            },
        )?;

        Ok(Self {
            _debouncer: debouncer,
            event_rx,
            watched_paths: HashSet::new(),
        })
    }

    /// Watch a worktree directory for changes
    pub fn watch(&mut self, path: PathBuf) -> Result<(), notify::Error> {
        if self.watched_paths.contains(&path) {
            return Ok(());
        }

        self._debouncer
            .watcher()
            .watch(&path, RecursiveMode::Recursive)?;

        self.watched_paths.insert(path);
        Ok(())
    }

    /// Stop watching a worktree directory
    pub fn unwatch(&mut self, path: &PathBuf) -> Result<(), notify::Error> {
        if !self.watched_paths.contains(path) {
            return Ok(());
        }

        self._debouncer.watcher().unwatch(path)?;
        self.watched_paths.remove(path);
        Ok(())
    }

    /// Try to receive a watch event without blocking
    pub fn try_recv(&self) -> Option<WatchEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Get all pending events (drains the channel)
    pub fn drain_events(&self) -> Vec<WatchEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.try_recv() {
            events.push(event);
        }
        events
    }

    /// Get unique worktree paths that need refreshing
    pub fn get_changed_worktrees(&self) -> HashSet<PathBuf> {
        let mut paths = HashSet::new();
        for event in self.drain_events() {
            let WatchEvent::FileChanged(path) = event;
            paths.insert(path);
        }
        paths
    }
}

/// Find the worktree root directory for a given path
/// Walks up the directory tree to find the git worktree root
fn find_worktree_root(path: &Path) -> Option<PathBuf> {
    let mut current = path.to_path_buf();

    // If it's a file, start from its parent
    if current.is_file() {
        current = current.parent()?.to_path_buf();
    }

    // Walk up looking for .git (file or directory)
    loop {
        let git_path = current.join(".git");
        if git_path.exists() {
            return Some(current);
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_find_worktree_root_not_git() {
        let dir = tempdir().unwrap();
        let result = find_worktree_root(&dir.path().to_path_buf());
        // No .git directory, so should return None
        assert!(result.is_none());
    }

    #[test]
    fn test_find_worktree_root_with_git_dir() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();

        let result = find_worktree_root(&dir.path().to_path_buf());
        assert_eq!(result, Some(dir.path().to_path_buf()));
    }

    #[test]
    fn test_find_worktree_root_nested_file() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::create_dir_all(dir.path().join("src/nested")).unwrap();
        let file_path = dir.path().join("src/nested/file.rs");
        fs::write(&file_path, "test").unwrap();

        let result = find_worktree_root(&file_path);
        assert_eq!(result, Some(dir.path().to_path_buf()));
    }
}
