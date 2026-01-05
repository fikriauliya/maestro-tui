# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build           # Development build
cargo build --release # Release build (optimized with LTO, stripped)
cargo run             # Run the application
cargo check           # Check code without building
cargo test            # Run tests
cargo fmt             # Format code
cargo clippy          # Lint code
```

## Architecture

**maestro-tui** is a Rust TUI application providing a dual-pane terminal emulator with vim-like modal editing. Built with Ratatui for UI and Alacritty's terminal library for PTY emulation.

### Core Structure

- `src/main.rs` - Application core: event loop, UI rendering, keyboard handling, tab/pane management
- `src/terminal.rs` - PTY management using `portable-pty`, terminal emulation via `alacritty_terminal`

### Key Patterns

- **Modal System**: Normal mode (navigation/commands) and Insert mode (terminal input). Press `i` to enter Insert, `Esc` to exit.
- **Dual-Pane Layout**: Left pane runs shell, right pane runs Claude CLI. 50/50 horizontal split.
- **Multi-Tab**: Create tabs with `t`, switch with `1-9`. Each tab has independent left/right terminal panes.
- **Thread Model**: Each terminal has a reader thread; main thread handles events at 60fps (16ms poll).
- **State Sharing**: `Arc<Mutex<T>>` for thread-safe terminal state.

### Navigation (Normal Mode)

- `h`/`l` or Left/Right arrows: Focus pane
- `Tab`: Toggle panes
- `t`: New tab
- `1-9`: Switch to tab
- `q`: Quit

## Issue Tracking

This project uses **bd** (beads) for issue tracking.

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```

## Session Completion

When ending a work session, complete ALL steps below. Work is NOT complete until `git push` succeeds.

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Push to remote**:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**Rules:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- If push fails, resolve and retry until it succeeds
