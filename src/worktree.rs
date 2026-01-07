//! Git worktree management module
//!
//! Provides a simple interface for managing git worktrees with a consistent
//! path resolution convention: worktrees are created as siblings of the main
//! repo at `../<repo-name>.<branch-name>`.

use std::path::{Path, PathBuf};
use std::process::Command;

use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};

/// Generate a merge commit message using Claude CLI
///
/// This function calls the `claude -p` command to generate a commit message
/// based on the diff. It's kept separate from git operations for testability.
pub fn generate_merge_message(branch: &str, main_branch: &str, diff: &str) -> Result<String> {
    let prompt = format!(
        "Generate a concise git merge commit message for merging branch '{}' into '{}'. \
         The message should summarize the changes. Here's the diff:\n\n{}",
        branch, main_branch, diff
    );

    let output = Command::new("claude")
        .args(["-p", &prompt])
        .output()
        .wrap_err("Failed to execute claude")?;

    if !output.status.success() {
        return Err(eyre!(
            "claude failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Represents a git worktree
#[derive(Debug, Clone, PartialEq)]
pub struct Worktree {
    /// Absolute path to the worktree directory
    pub path: PathBuf,
    /// Branch name (None for bare/detached worktrees)
    pub branch: Option<String>,
    /// Whether this is the main worktree
    pub is_main: bool,
    /// Whether the worktree is bare
    pub is_bare: bool,
}

/// Warning returned when safety checks fail
#[derive(Debug, Clone, PartialEq)]
pub enum RemoveWarning {
    /// Branch has not been merged to main
    NotMerged { branch: String },
    /// Worktree has uncommitted changes
    UncommittedChanges,
}

/// Status information for a worktree
#[derive(Debug, Clone)]
pub struct WorktreeStatus {
    /// The worktree
    pub worktree: Worktree,
    /// Whether there are uncommitted changes
    pub is_dirty: bool,
    /// Commits ahead of main
    pub ahead: usize,
    /// Commits behind main
    pub behind: usize,
}

/// Generate a git-friendly branch name from a prompt
///
/// Takes the first few words, lowercases, and replaces non-alphanumeric with dashes.
/// Example: "Add user authentication" -> "add-user-authentication"
pub fn slugify_prompt(prompt: &str) -> String {
    let words: Vec<&str> = prompt
        .split_whitespace()
        .take(5) // Take first 5 words max
        .collect();

    let slug: String = words
        .join("-")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    // Remove consecutive dashes and trim dashes from ends
    let mut result = String::new();
    let mut prev_dash = true; // Start true to skip leading dashes
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash {
                result.push(c);
                prev_dash = true;
            }
        } else {
            result.push(c);
            prev_dash = false;
        }
    }

    // Trim trailing dash
    if result.ends_with('-') {
        result.pop();
    }

    // Ensure non-empty
    if result.is_empty() {
        result = "worktree".to_string();
    }

    result
}

/// Trait for git command execution, enabling dependency injection for testing
pub trait GitBackend: Send {
    fn execute(&self, args: &[&str]) -> std::io::Result<std::process::Output>;
    fn execute_in_dir(&self, dir: &Path, args: &[&str]) -> std::io::Result<std::process::Output>;
}

/// Native git backend using std::process::Command
pub struct NativeGit;

impl GitBackend for NativeGit {
    fn execute(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new("git").args(args).output()
    }

    fn execute_in_dir(&self, dir: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new("git").current_dir(dir).args(args).output()
    }
}

/// Manages git worktrees with a consistent path convention
pub struct WorktreeManager<G: GitBackend = NativeGit> {
    /// Root path of the main repository
    repo_root: PathBuf,
    /// Name of the repository (last component of repo_root)
    repo_name: String,
    /// Git backend for command execution
    git: G,
}

impl WorktreeManager<NativeGit> {
    /// Create a new WorktreeManager for the current repository
    pub fn new() -> Result<Self> {
        Self::with_backend(NativeGit)
    }
}

impl<G: GitBackend> WorktreeManager<G> {
    /// Create a new WorktreeManager with a custom git backend
    pub fn with_backend(git: G) -> Result<Self> {
        let output = git
            .execute(&["rev-parse", "--show-toplevel"])
            .wrap_err("Failed to execute git")?;

        if !output.status.success() {
            return Err(eyre!(
                "Not a git repository: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let repo_root = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        let repo_name = repo_root
            .file_name()
            .ok_or_else(|| eyre!("Invalid repository path"))?
            .to_string_lossy()
            .to_string();

        Ok(Self {
            repo_root,
            repo_name,
            git,
        })
    }

    /// Get the repository root path
    #[allow(dead_code)] // Used in tests
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Get the repository name
    #[allow(dead_code)] // Used in tests
    pub fn repo_name(&self) -> &str {
        &self.repo_name
    }

    /// Resolve the path for a worktree given a branch name
    ///
    /// Convention: `../<repo-name>.<branch-name>`
    /// Example: repo at `/code/myapp` with branch `feature-x` -> `/code/myapp.feature-x`
    pub fn resolve_worktree_path(&self, branch: &str) -> PathBuf {
        let parent = self.repo_root.parent().unwrap_or(Path::new("/"));
        parent.join(format!("{}.{}", self.repo_name, branch))
    }

    /// List all worktrees for this repository
    pub fn list(&self) -> Result<Vec<Worktree>> {
        let output = self
            .git
            .execute(&["worktree", "list", "--porcelain"])
            .wrap_err("Failed to execute git worktree list")?;

        if !output.status.success() {
            return Err(eyre!(
                "git worktree list failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_worktree_list(&stdout)
    }

    /// Create a new worktree with a new branch
    ///
    /// Creates a worktree at the resolved path for the given branch name.
    /// The `prompt` parameter is stored for later use when opening the worktree
    /// in maestro-tui (not used by this function directly).
    pub fn create(&self, branch: &str, _prompt: Option<&str>) -> Result<Worktree> {
        let path = self.resolve_worktree_path(branch);

        if path.exists() {
            return Err(eyre!("Worktree path already exists: {}", path.display()));
        }

        let path_str = path.to_string_lossy();
        let output = self
            .git
            .execute(&["worktree", "add", "-b", branch, &path_str])
            .wrap_err("Failed to execute git worktree add")?;

        if !output.status.success() {
            return Err(eyre!(
                "git worktree add failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(Worktree {
            path,
            branch: Some(branch.to_string()),
            is_main: false,
            is_bare: false,
        })
    }

    /// Get the path to switch to for a given branch
    ///
    /// Returns the worktree path if it exists, or an error if not found.
    pub fn switch(&self, branch: &str) -> Result<PathBuf> {
        let worktrees = self.list()?;

        // First try exact branch match
        for wt in &worktrees {
            if wt.branch.as_deref() == Some(branch) {
                return Ok(wt.path.clone());
            }
        }

        // Try resolved path
        let expected_path = self.resolve_worktree_path(branch);
        for wt in &worktrees {
            if wt.path == expected_path {
                return Ok(wt.path.clone());
            }
        }

        Err(eyre!("No worktree found for branch: {}", branch))
    }

    /// Check if a branch has been merged to main/master
    fn is_branch_merged(&self, branch: &str) -> Result<bool> {
        // Try 'main' first, then 'master'
        for main_branch in &["main", "master"] {
            let output = self
                .git
                .execute(&["branch", "--merged", main_branch])
                .wrap_err("Failed to check merged branches")?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let line = line.trim().trim_start_matches("* ");
                    if line == branch {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
        }

        // If neither main nor master exists, assume not merged
        Ok(false)
    }

    /// Check if a worktree has uncommitted changes
    fn has_uncommitted_changes(&self, path: &Path) -> Result<bool> {
        let output = self
            .git
            .execute_in_dir(path, &["status", "--porcelain"])
            .wrap_err("Failed to check worktree status")?;

        if !output.status.success() {
            return Err(eyre!(
                "git status failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    /// Remove a worktree with safety checks
    ///
    /// Returns warnings if the branch is not merged or has uncommitted changes.
    /// Set `force` to true to proceed despite warnings.
    pub fn remove(&self, branch: &str, force: bool) -> Result<Vec<RemoveWarning>> {
        let path = self.switch(branch)?;
        let mut warnings = Vec::new();

        // Safety check 1: Is branch merged?
        if !self.is_branch_merged(branch)? {
            warnings.push(RemoveWarning::NotMerged {
                branch: branch.to_string(),
            });
        }

        // Safety check 2: Uncommitted changes?
        if self.has_uncommitted_changes(&path)? {
            warnings.push(RemoveWarning::UncommittedChanges);
        }

        // If we have warnings and not forcing, return them
        if !warnings.is_empty() && !force {
            return Ok(warnings);
        }

        // Proceed with removal
        let path_str = path.to_string_lossy();
        let args = if force {
            vec!["worktree", "remove", "--force", &path_str]
        } else {
            vec!["worktree", "remove", &path_str]
        };

        let output = self
            .git
            .execute(&args)
            .wrap_err("Failed to execute git worktree remove")?;

        if !output.status.success() {
            return Err(eyre!(
                "git worktree remove failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(warnings)
    }

    /// Get the diff between main and a branch
    ///
    /// Returns the diff output as a string, useful for generating commit messages.
    pub fn get_branch_diff(&self, branch: &str) -> Result<String> {
        let main_branch = self.get_main_branch()?;
        let diff_output = self
            .git
            .execute(&["diff", &format!("{}...{}", main_branch, branch)])
            .wrap_err("Failed to get diff")?;

        Ok(String::from_utf8_lossy(&diff_output.stdout).to_string())
    }

    /// Merge a worktree branch into main
    ///
    /// If `commit_message` is provided, uses it for the merge commit.
    /// Otherwise, uses git's default merge behavior.
    pub fn merge(&self, branch: &str, commit_message: Option<&str>) -> Result<()> {
        let main_branch = self.get_main_branch()?;

        // Checkout main branch
        let output = self
            .git
            .execute(&["checkout", &main_branch])
            .wrap_err("Failed to checkout main branch")?;

        if !output.status.success() {
            return Err(eyre!(
                "git checkout failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        // Perform merge with or without custom message
        let output = match commit_message {
            Some(msg) => self
                .git
                .execute(&["merge", branch, "-m", msg])
                .wrap_err("Failed to merge")?,
            None => self
                .git
                .execute(&["merge", branch])
                .wrap_err("Failed to merge")?,
        };

        if !output.status.success() {
            return Err(eyre!(
                "git merge failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(())
    }

    /// Get the main branch name (main or master)
    fn get_main_branch(&self) -> Result<String> {
        for branch in &["main", "master"] {
            let output = self
                .git
                .execute(&["rev-parse", "--verify", branch])
                .wrap_err("Failed to verify branch")?;

            if output.status.success() {
                return Ok(branch.to_string());
            }
        }

        Err(eyre!("No main or master branch found"))
    }

    /// Get the number of commits a branch is ahead/behind main
    ///
    /// Returns (ahead, behind) where:
    /// - ahead = commits in branch not in main
    /// - behind = commits in main not in branch
    pub fn get_ahead_behind(&self, branch: &str) -> Result<(usize, usize)> {
        let main_branch = self.get_main_branch()?;

        // For main branch itself, return (0, 0)
        if branch == main_branch {
            return Ok((0, 0));
        }

        let output = self
            .git
            .execute(&[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{}...{}", main_branch, branch),
            ])
            .wrap_err("Failed to get ahead/behind count")?;

        if !output.status.success() {
            // Branch might not exist or have no common ancestor
            return Ok((0, 0));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = stdout.split_whitespace().collect();

        if parts.len() == 2 {
            let behind = parts[0].parse().unwrap_or(0);
            let ahead = parts[1].parse().unwrap_or(0);
            Ok((ahead, behind))
        } else {
            Ok((0, 0))
        }
    }

    /// List all worktrees with their status information
    pub fn list_with_status(&self) -> Result<Vec<WorktreeStatus>> {
        let worktrees = self.list()?;
        let mut statuses = Vec::new();

        for wt in worktrees {
            let is_dirty = self.has_uncommitted_changes(&wt.path).unwrap_or(false);
            let (ahead, behind) = if let Some(ref branch) = wt.branch {
                self.get_ahead_behind(branch).unwrap_or((0, 0))
            } else {
                (0, 0)
            };

            statuses.push(WorktreeStatus {
                worktree: wt,
                is_dirty,
                ahead,
                behind,
            });
        }

        Ok(statuses)
    }
}

/// Parse the output of `git worktree list --porcelain`
fn parse_worktree_list(output: &str) -> Result<Vec<Worktree>> {
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;
    let mut is_bare = false;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            // Save previous worktree if exists
            if let Some(path) = current_path.take() {
                worktrees.push(Worktree {
                    path: path.clone(),
                    branch: current_branch.take(),
                    is_main: worktrees.is_empty(),
                    is_bare,
                });
                is_bare = false;
            }
            current_path = Some(PathBuf::from(line.strip_prefix("worktree ").unwrap()));
        } else if line.starts_with("branch refs/heads/") {
            current_branch = Some(line.strip_prefix("branch refs/heads/").unwrap().to_string());
        } else if line == "bare" {
            is_bare = true;
        }
    }

    // Don't forget the last worktree
    if let Some(path) = current_path {
        worktrees.push(Worktree {
            path,
            branch: current_branch,
            is_main: worktrees.is_empty(),
            is_bare,
        });
    }

    Ok(worktrees)
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// Mock git backend for testing
    #[derive(Clone)]
    pub struct MockGit {
        /// Map of command args -> output
        pub responses: Arc<Mutex<HashMap<String, std::process::Output>>>,
        /// Record of all executed commands
        pub executed: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl MockGit {
        pub fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(HashMap::new())),
                executed: Arc::new(Mutex::new(Vec::new())),
            }
        }

        /// Set the response for a command
        pub fn set_response(&self, args: &[&str], stdout: &str, success: bool) {
            let key = args.join(" ");
            let output = std::process::Output {
                status: if success {
                    std::process::ExitStatus::default()
                } else {
                    // Create a failed status - this is platform-dependent
                    // For testing, we'll use the success check in the code
                    std::process::ExitStatus::default()
                },
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
            };
            // Note: ExitStatus::default() is always success, so we need a different approach
            self.responses.lock().unwrap().insert(key, output);
        }

        /// Set response with custom success status using raw output
        pub fn set_raw_response(&self, args: &[&str], output: std::process::Output) {
            let key = args.join(" ");
            self.responses.lock().unwrap().insert(key, output);
        }

        /// Get all executed commands
        pub fn get_executed(&self) -> Vec<Vec<String>> {
            self.executed.lock().unwrap().clone()
        }
    }

    impl Default for MockGit {
        fn default() -> Self {
            Self::new()
        }
    }

    impl GitBackend for MockGit {
        fn execute(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
            self.executed
                .lock()
                .unwrap()
                .push(args.iter().map(|s| s.to_string()).collect());

            let key = args.join(" ");
            if let Some(output) = self.responses.lock().unwrap().get(&key) {
                Ok(output.clone())
            } else {
                // Default: return empty success
                Ok(std::process::Output {
                    status: std::process::ExitStatus::default(),
                    stdout: Vec::new(),
                    stderr: b"command not mocked".to_vec(),
                })
            }
        }

        fn execute_in_dir(
            &self,
            _dir: &Path,
            args: &[&str],
        ) -> std::io::Result<std::process::Output> {
            self.execute(args)
        }
    }

    /// Helper to create a successful output
    pub fn success_output(stdout: &str) -> std::process::Output {
        std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list_single() {
        let output = "worktree /home/user/project\nbranch refs/heads/main\n\n";
        let worktrees = parse_worktree_list(output).unwrap();

        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].path, PathBuf::from("/home/user/project"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert!(worktrees[0].is_main);
        assert!(!worktrees[0].is_bare);
    }

    #[test]
    fn test_parse_worktree_list_multiple() {
        let output = "\
worktree /home/user/project
branch refs/heads/main

worktree /home/user/project.feature-x
branch refs/heads/feature-x

worktree /home/user/project.bugfix
branch refs/heads/bugfix
";
        let worktrees = parse_worktree_list(output).unwrap();

        assert_eq!(worktrees.len(), 3);

        assert_eq!(worktrees[0].path, PathBuf::from("/home/user/project"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert!(worktrees[0].is_main);

        assert_eq!(
            worktrees[1].path,
            PathBuf::from("/home/user/project.feature-x")
        );
        assert_eq!(worktrees[1].branch, Some("feature-x".to_string()));
        assert!(!worktrees[1].is_main);

        assert_eq!(
            worktrees[2].path,
            PathBuf::from("/home/user/project.bugfix")
        );
        assert_eq!(worktrees[2].branch, Some("bugfix".to_string()));
        assert!(!worktrees[2].is_main);
    }

    #[test]
    fn test_parse_worktree_list_bare() {
        let output = "worktree /home/user/project.git\nbare\n\n";
        let worktrees = parse_worktree_list(output).unwrap();

        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].is_bare);
        assert!(worktrees[0].branch.is_none());
    }

    #[test]
    fn test_resolve_worktree_path() {
        use mock::{MockGit, success_output};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/myapp\n"),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();

        assert_eq!(
            manager.resolve_worktree_path("feature-x"),
            PathBuf::from("/home/user/myapp.feature-x")
        );

        assert_eq!(
            manager.resolve_worktree_path("bugfix-123"),
            PathBuf::from("/home/user/myapp.bugfix-123")
        );
    }

    #[test]
    fn test_manager_repo_info() {
        use mock::{MockGit, success_output};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/code/awesome-project\n"),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();

        assert_eq!(manager.repo_root(), Path::new("/code/awesome-project"));
        assert_eq!(manager.repo_name(), "awesome-project");
    }

    #[test]
    fn test_list_worktrees() {
        use mock::{MockGit, success_output};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        git.set_raw_response(
            &["worktree", "list", "--porcelain"],
            success_output(
                "\
worktree /home/user/project
branch refs/heads/main

worktree /home/user/project.dev
branch refs/heads/dev
",
            ),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let worktrees = manager.list().unwrap();

        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert_eq!(worktrees[1].branch, Some("dev".to_string()));
    }

    #[test]
    fn test_switch_finds_worktree() {
        use mock::{MockGit, success_output};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        git.set_raw_response(
            &["worktree", "list", "--porcelain"],
            success_output(
                "\
worktree /home/user/project
branch refs/heads/main

worktree /home/user/project.feature
branch refs/heads/feature
",
            ),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let path = manager.switch("feature").unwrap();

        assert_eq!(path, PathBuf::from("/home/user/project.feature"));
    }

    #[test]
    fn test_switch_not_found() {
        use mock::{MockGit, success_output};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        git.set_raw_response(
            &["worktree", "list", "--porcelain"],
            success_output("worktree /home/user/project\nbranch refs/heads/main\n"),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let result = manager.switch("nonexistent");

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No worktree found")
        );
    }

    #[test]
    fn test_slugify_prompt_basic() {
        assert_eq!(
            slugify_prompt("Add user authentication"),
            "add-user-authentication"
        );
        assert_eq!(slugify_prompt("Fix bug in login"), "fix-bug-in-login");
    }

    #[test]
    fn test_slugify_prompt_special_chars() {
        assert_eq!(slugify_prompt("Add feature: auth!"), "add-feature-auth");
        assert_eq!(slugify_prompt("Fix #123 bug"), "fix-123-bug");
    }

    #[test]
    fn test_slugify_prompt_truncates_words() {
        assert_eq!(
            slugify_prompt("one two three four five six seven"),
            "one-two-three-four-five"
        );
    }

    #[test]
    fn test_slugify_prompt_empty() {
        assert_eq!(slugify_prompt(""), "worktree");
        assert_eq!(slugify_prompt("   "), "worktree");
    }

    #[test]
    fn test_slugify_prompt_consecutive_special() {
        assert_eq!(
            slugify_prompt("test--multiple---dashes"),
            "test-multiple-dashes"
        );
    }

    // --- create tests ---

    #[test]
    fn test_create_worktree_succeeds() {
        use mock::{success_output, MockGit};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/tmp/test-project\n"),
        );
        git.set_raw_response(
            &["worktree", "add", "-b", "feature-x", "/tmp/test-project.feature-x"],
            success_output(""),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let worktree = manager.create("feature-x", Some("Test prompt")).unwrap();

        assert_eq!(worktree.path, PathBuf::from("/tmp/test-project.feature-x"));
        assert_eq!(worktree.branch, Some("feature-x".to_string()));
        assert!(!worktree.is_main);
        assert!(!worktree.is_bare);
    }

    // --- remove tests ---

    #[test]
    fn test_remove_returns_not_merged_warning() {
        use mock::{success_output, MockGit};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        // List worktrees
        git.set_raw_response(
            &["worktree", "list", "--porcelain"],
            success_output(
                "\
worktree /home/user/project
branch refs/heads/main

worktree /home/user/project.feature
branch refs/heads/feature
",
            ),
        );
        // Check merged - returns empty (not merged)
        git.set_raw_response(
            &["branch", "--merged", "main"],
            success_output("  main\n"),
        );
        // Check uncommitted changes - clean
        git.set_raw_response(
            &["status", "--porcelain"],
            success_output(""),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let warnings = manager.remove("feature", false).unwrap();

        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            &warnings[0],
            RemoveWarning::NotMerged { branch } if branch == "feature"
        ));
    }

    #[test]
    fn test_remove_returns_uncommitted_changes_warning() {
        use mock::{success_output, MockGit};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        git.set_raw_response(
            &["worktree", "list", "--porcelain"],
            success_output(
                "\
worktree /home/user/project
branch refs/heads/main

worktree /home/user/project.feature
branch refs/heads/feature
",
            ),
        );
        // Check merged - branch is merged
        git.set_raw_response(
            &["branch", "--merged", "main"],
            success_output("  main\n  feature\n"),
        );
        // Check uncommitted changes - has changes
        git.set_raw_response(
            &["status", "--porcelain"],
            success_output(" M src/main.rs\n"),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let warnings = manager.remove("feature", false).unwrap();

        assert_eq!(warnings.len(), 1);
        assert!(matches!(&warnings[0], RemoveWarning::UncommittedChanges));
    }

    #[test]
    fn test_remove_with_force_proceeds() {
        use mock::{success_output, MockGit};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        git.set_raw_response(
            &["worktree", "list", "--porcelain"],
            success_output(
                "\
worktree /home/user/project
branch refs/heads/main

worktree /home/user/project.feature
branch refs/heads/feature
",
            ),
        );
        // Not merged
        git.set_raw_response(
            &["branch", "--merged", "main"],
            success_output("  main\n"),
        );
        // Has changes
        git.set_raw_response(
            &["status", "--porcelain"],
            success_output(" M src/main.rs\n"),
        );
        // Force remove succeeds
        git.set_raw_response(
            &["worktree", "remove", "--force", "/home/user/project.feature"],
            success_output(""),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let warnings = manager.remove("feature", true).unwrap();

        // Should have warnings but proceed anyway
        assert_eq!(warnings.len(), 2);
    }

    // --- get_ahead_behind tests ---

    #[test]
    fn test_get_ahead_behind_returns_counts() {
        use mock::{success_output, MockGit};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        // Check for main branch
        git.set_raw_response(
            &["rev-parse", "--verify", "main"],
            success_output("abc123\n"),
        );
        // rev-list output: behind ahead
        git.set_raw_response(
            &["rev-list", "--left-right", "--count", "main...feature"],
            success_output("2\t5\n"),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let (ahead, behind) = manager.get_ahead_behind("feature").unwrap();

        assert_eq!(ahead, 5);
        assert_eq!(behind, 2);
    }

    #[test]
    fn test_get_ahead_behind_main_branch_is_zero() {
        use mock::{success_output, MockGit};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        git.set_raw_response(
            &["rev-parse", "--verify", "main"],
            success_output("abc123\n"),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let (ahead, behind) = manager.get_ahead_behind("main").unwrap();

        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
    }

    // --- merge tests ---

    #[test]
    fn test_merge_with_custom_message() {
        use mock::{success_output, MockGit};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        // Check for main branch
        git.set_raw_response(
            &["rev-parse", "--verify", "main"],
            success_output("abc123\n"),
        );
        // Checkout main
        git.set_raw_response(
            &["checkout", "main"],
            success_output(""),
        );
        // Merge with message
        git.set_raw_response(
            &["merge", "feature", "-m", "Custom merge message"],
            success_output(""),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let result = manager.merge("feature", Some("Custom merge message"));

        assert!(result.is_ok());
    }

    #[test]
    fn test_merge_without_message() {
        use mock::{success_output, MockGit};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        git.set_raw_response(
            &["rev-parse", "--verify", "main"],
            success_output("abc123\n"),
        );
        git.set_raw_response(
            &["checkout", "main"],
            success_output(""),
        );
        git.set_raw_response(
            &["merge", "feature"],
            success_output(""),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let result = manager.merge("feature", None);

        assert!(result.is_ok());
    }

    // --- list_with_status tests ---

    #[test]
    fn test_list_with_status_includes_dirty_and_ahead_behind() {
        use mock::{success_output, MockGit};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/home/user/project\n"),
        );
        git.set_raw_response(
            &["worktree", "list", "--porcelain"],
            success_output(
                "\
worktree /home/user/project
branch refs/heads/main

worktree /home/user/project.feature
branch refs/heads/feature
",
            ),
        );
        // Main is clean
        git.set_raw_response(
            &["status", "--porcelain"],
            success_output(""),
        );
        // Check main branch exists
        git.set_raw_response(
            &["rev-parse", "--verify", "main"],
            success_output("abc123\n"),
        );
        // Main is at 0,0
        git.set_raw_response(
            &["rev-list", "--left-right", "--count", "main...main"],
            success_output("0\t0\n"),
        );
        // Feature is 1 ahead, 2 behind
        git.set_raw_response(
            &["rev-list", "--left-right", "--count", "main...feature"],
            success_output("2\t1\n"),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();
        let statuses = manager.list_with_status().unwrap();

        assert_eq!(statuses.len(), 2);

        // Main worktree
        assert_eq!(statuses[0].worktree.branch, Some("main".to_string()));
        assert!(!statuses[0].is_dirty);
        assert_eq!(statuses[0].ahead, 0);
        assert_eq!(statuses[0].behind, 0);

        // Feature worktree
        assert_eq!(statuses[1].worktree.branch, Some("feature".to_string()));
        assert_eq!(statuses[1].ahead, 1);
        assert_eq!(statuses[1].behind, 2);
    }

    // --- RemoveWarning tests ---

    #[test]
    fn test_remove_warning_equality() {
        let w1 = RemoveWarning::NotMerged { branch: "test".to_string() };
        let w2 = RemoveWarning::NotMerged { branch: "test".to_string() };
        let w3 = RemoveWarning::NotMerged { branch: "other".to_string() };
        let w4 = RemoveWarning::UncommittedChanges;

        assert_eq!(w1, w2);
        assert_ne!(w1, w3);
        assert_ne!(w1, w4);
    }

    // --- WorktreeStatus tests ---

    #[test]
    fn test_worktree_status_debug() {
        let status = WorktreeStatus {
            worktree: Worktree {
                path: PathBuf::from("/test"),
                branch: Some("main".to_string()),
                is_main: true,
                is_bare: false,
            },
            is_dirty: true,
            ahead: 5,
            behind: 3,
        };

        // Just verify Debug trait is implemented
        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("WorktreeStatus"));
    }
}
