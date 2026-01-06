# maestro-tui

A dual-pane terminal multiplexer designed for AI-assisted development workflows.

## Why maestro-tui?

Modern AI coding assistants like Claude Code are powerful, but juggling between your terminal and an AI session breaks your flow. **maestro-tui** solves this by putting your shell and Claude Code side-by-side in a unified interface.

**One worktree, one tab.** Each task gets its own git worktree and dedicated tab. Start a new feature? Type a prompt, and maestro-tui creates a fresh worktree with Claude Code ready to help. No more context switching. No more branch juggling. Just focused, parallel workstreams.

### Key Features

- **Dual-pane layout** - Shell on the left, Claude Code on the right. Always visible, always accessible.
- **Worktree-per-tab** - Each tab is a separate git worktree. Work on multiple features simultaneously without stashing or branch switching.
- **Prompt-driven workflow** - Describe what you want to build in the control panel. Maestro creates the worktree and tab automatically.
- **Touch-friendly** - Clickable tabs and quit button. Works great over SSH from an iPad.
- **Keyboard shortcuts** - Alt+0-9 for tabs, Ctrl+h/l for panes, Ctrl+x to quit.

## Installation

### Prerequisites

- Rust 1.75+ (2024 edition)
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

- **Tab 0** - Control panel for creating new worktree tabs
- **Tab 1+** - Worktree tabs with dual terminal panes

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Alt+0` | Switch to control panel |
| `Alt+1-9` | Switch to tab 1-9 |
| `Ctrl+h` | Focus left pane (shell) |
| `Ctrl+l` | Focus right pane (Claude) |
| `Ctrl+x` | Quit |

### Mouse

- Click tabs to switch
- Click `[X]` to quit

### Creating a New Task

1. Go to Tab 0 (control panel) with `Alt+0` or click
2. Type a prompt describing your task
3. Press Enter

Maestro creates a new git worktree and tab with Claude Code ready to assist.

## For Developers

### Project Structure

```
src/
├── main.rs      # Entry point, event loop, UI rendering
├── app.rs       # Application state, commands, key handling
├── terminal.rs  # PTY management, terminal emulation
├── input.rs     # Key-to-bytes conversion for terminal
├── ui.rs        # Styling (Flexoki dark theme)
├── pty.rs       # PTY abstraction with mock for testing
└── worktree.rs  # Git worktree management
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
- **Flexoki** dark color theme

The app runs a 60fps render loop. Each terminal pane has a background reader thread for PTY output. State is shared via `Arc<Mutex<T>>`.

### Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run `cargo test` and `cargo clippy`
5. Submit a pull request

## License

MIT
