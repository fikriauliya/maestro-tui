# maestro-tui

A dual-pane terminal multiplexer designed for AI-assisted development workflows.

<img width="1783" height="948" alt="Screenshot 2026-01-07 at 17 18 19" src="https://github.com/user-attachments/assets/2e2d881b-ffb1-4e5b-9023-35d6fb47b071" />


## Why maestro-tui?

Modern AI coding assistants like Claude Code are powerful, but juggling between your terminal and an AI session breaks your flow. **maestro-tui** solves this by putting your shell and Claude Code side-by-side in a unified interface.

**One worktree, one tab.** Each task gets its own git worktree and dedicated tab. Start a new feature? Type a prompt, and maestro-tui creates a fresh worktree with Claude Code ready to help. No more context switching. No more branch juggling. Just focused, parallel workstreams.

### Key Features

- **Dual-pane layout** - Diff viewer on the left, Claude Code on the right. Always visible, always accessible.
- **Worktree-per-tab** - Each tab is a separate git worktree. Work on multiple features simultaneously without stashing or branch switching.
- **Prompt-driven workflow** - Describe what you want to build in the control panel. Maestro creates the worktree and tab automatically.
- **Live diff viewer** - See your uncommitted changes in real-time with syntax highlighting. File watcher automatically refreshes when files change.
- **Theme picker** - Choose from 5 built-in themes: Flexoki Dark, Dracula, Nord, Catppuccin Mocha, and Gruvbox Dark.
- **Touch-friendly** - Clickable tabs and quit button. Works great over SSH from an iPad.
- **Vim-style keybindings** - Alt-based shortcuts for all navigation: Alt+1-9 for tabs, Alt+h/l for panes, Alt+u/d for scrolling.

## Installation

### Quick install (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/fikriauliya/maestro-tui/refs/heads/main/install.sh | bash
```

### From crates.io

```bash
cargo install maestro-tui
```

### From GitHub Releases

Download the latest binary from [GitHub Releases](https://github.com/fikriauliya/maestro-tui/releases):

```bash
# Linux x86_64
curl -LO https://github.com/fikriauliya/maestro-tui/releases/latest/download/maestro-tui-linux-x86_64.tar.gz
tar xzf maestro-tui-linux-x86_64.tar.gz
sudo mv maestro-tui /usr/local/bin/
```

### Prerequisites

- [Claude Code](https://claude.ai/code) installed and configured
- Git

### Build from source

```bash
git clone https://github.com/fikriauliya/maestro-tui.git
cd maestro-tui
cargo build --release
```

The binary will be at `target/release/maestro-tui`.

## Usage

```bash
maestro-tui
```

### Tabs

- **Tab 1 (Alt+1)** - Control panel for creating new worktree tabs, with Claude terminal on the right
- **Tab 2+ (Alt+2-9)** - Worktree tabs with diff viewer on the left, Claude Code on the right

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Alt+1` | Switch to control panel |
| `Alt+2-9` | Switch to worktree tabs 1-8 |
| `Alt+h` | Focus left pane (diff viewer) |
| `Alt+l` | Focus right pane (Claude) |
| `Alt+u` | Scroll up (half page) |
| `Alt+d` | Scroll down (half page) |
| `Alt+t` | Open theme picker |
| `Alt+q` | Quit |

### Mouse

- Click tabs to switch
- Click `[X]` to quit

### Creating a New Task

1. Go to the control panel with `Alt+1` or click Tab 1
2. Type a prompt describing your task
3. Press Enter

Maestro creates a new git worktree and tab with Claude Code ready to assist.

## For Developers

### Project Structure

```
src/
├── main.rs          # Entry point, event loop
├── app.rs           # Application state, commands, key handling
├── render.rs        # UI rendering with Ratatui
├── event_handler.rs # Keyboard and mouse event processing
├── terminal.rs      # PTY management, terminal emulation
├── terminal_pair.rs # Dual terminal pane management
├── diff_viewer.rs   # Git diff display with syntax highlighting
├── file_watcher.rs  # File system watcher for auto-refresh
├── theme.rs         # Color themes (Flexoki, Dracula, Nord, etc.)
├── input.rs         # Key-to-bytes conversion for terminal
├── pty.rs           # PTY abstraction with mock for testing
└── worktree.rs      # Git worktree management
```

### Building

```bash
cargo build          # Debug build
cargo build --release  # Optimized release build
```

### Testing

```bash
cargo test           # Run all tests
cargo test -- --nocapture  # With output
```

### Code Quality

```bash
cargo fmt            # Format code
cargo clippy         # Lint
cargo check          # Type check without building
```

### Architecture

- **Ratatui** for terminal UI rendering
- **Alacritty Terminal** for terminal emulation
- **portable-pty** for cross-platform PTY handling
- **notify** for file system watching (event-driven diff updates)
- **5 color themes** - Flexoki Dark (default), Dracula, Nord, Catppuccin Mocha, Gruvbox Dark

The app runs a 60fps render loop. Each terminal pane has a background reader thread for PTY output. File changes are detected via inotify (Linux) / FSEvents (macOS) for efficient diff refreshing. State is shared via `Arc<Mutex<T>>`.

### Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run `cargo test` and `cargo clippy`
5. Submit a pull request

## License

MIT
