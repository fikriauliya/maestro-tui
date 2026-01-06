//! Git worktree management module
//!
//! Provides a simple interface for managing git worktrees with a consistent
//! path resolution convention: worktrees are created as siblings of the main
//! repo at `../<repo-name>.<branch-name>`.

use std::path::{Path, PathBuf};
use std::process::Command;

use color_eyre::eyre::{eyre, Context};
use color_eyre::Result;

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
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Get the repository name
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

    /// Merge a worktree branch into main
    ///
    /// If `use_claude` is true, uses `claude -p` to generate the commit message.
    /// Otherwise, uses git's default merge behavior.
    pub fn merge(&self, branch: &str, use_claude: bool) -> Result<()> {
        // Get the main branch name
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

        if use_claude {
            // Get the diff for context
            let diff_output = self
                .git
                .execute(&["diff", &format!("{}...{}", main_branch, branch)])
                .wrap_err("Failed to get diff")?;

            let diff = String::from_utf8_lossy(&diff_output.stdout);

            // Generate commit message using claude
            let prompt = format!(
                "Generate a concise git merge commit message for merging branch '{}' into '{}'. \
                 The message should summarize the changes. Here's the diff:\n\n{}",
                branch, main_branch, diff
            );

            let claude_output = Command::new("claude")
                .args(["-p", &prompt])
                .output()
                .wrap_err("Failed to execute claude")?;

            if !claude_output.status.success() {
                return Err(eyre!(
                    "claude failed: {}",
                    String::from_utf8_lossy(&claude_output.stderr).trim()
                ));
            }

            let commit_msg = String::from_utf8_lossy(&claude_output.stdout)
                .trim()
                .to_string();

            // Merge with custom message
            let output = self
                .git
                .execute(&["merge", branch, "-m", &commit_msg])
                .wrap_err("Failed to merge")?;

            if !output.status.success() {
                return Err(eyre!(
                    "git merge failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        } else {
            // Standard merge
            let output = self
                .git
                .execute(&["merge", branch])
                .wrap_err("Failed to merge")?;

            if !output.status.success() {
                return Err(eyre!(
                    "git merge failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
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
            current_branch = Some(
                line.strip_prefix("branch refs/heads/")
                    .unwrap()
                    .to_string(),
            );
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

        fn execute_in_dir(&self, _dir: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
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
        use mock::{success_output, MockGit};

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
        use mock::{success_output, MockGit};

        let git = MockGit::new();
        git.set_raw_response(
            &["rev-parse", "--show-toplevel"],
            success_output("/code/awesome-project\n"),
        );

        let manager = WorktreeManager::with_backend(git).unwrap();

        assert_eq!(
            manager.repo_root(),
            Path::new("/code/awesome-project")
        );
        assert_eq!(manager.repo_name(), "awesome-project");
    }

    #[test]
    fn test_list_worktrees() {
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

        let manager = WorktreeManager::with_backend(git).unwrap();
        let path = manager.switch("feature").unwrap();

        assert_eq!(path, PathBuf::from("/home/user/project.feature"));
    }

    #[test]
    fn test_switch_not_found() {
        use mock::{success_output, MockGit};

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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No worktree found"));
    }

    #[test]
    fn test_slugify_prompt_basic() {
        assert_eq!(slugify_prompt("Add user authentication"), "add-user-authentication");
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
        assert_eq!(slugify_prompt("test--multiple---dashes"), "test-multiple-dashes");
    }
}
