mod app;
mod diff_viewer;
mod event_handler;
mod input;
mod pty;
mod render;
mod terminal;
mod terminal_pair;
mod theme;
mod worktree;

use std::time::{Duration, Instant};

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::{Terminal as RatatuiTerminal, backend::CrosstermBackend, layout::Rect};

use crate::app::{App, Command, Dialog, Tab, TabKind};
use crate::event_handler::{
    KeyAction, load_bd_ready, process_control_panel_key, process_dialog_key, process_mouse_click,
    process_terminal_key,
};
use crate::worktree::WorktreeManager;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut app = App::new();

    // Load bd ready output at startup
    app.execute(Command::ReloadBdReady(load_bd_ready()));

    // Tab 0 is always the control panel (already created by App::new())
    // Load existing worktrees as additional tabs (no stored prompt for existing worktrees)
    if let Ok(wt_manager) = WorktreeManager::new()
        && let Ok(worktrees) = wt_manager.list()
    {
        for wt in worktrees {
            let branch = wt.branch.unwrap_or_else(|| "detached".to_string());
            app.tabs
                .push(Tab::with_worktree(wt.path, branch, String::new()));
        }
    }

    // Setup terminal with mouse support
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        Clear(ClearType::All),
        EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = RatatuiTerminal::new(backend)?;

    let result = run(&mut app, &mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        Clear(ClearType::All),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run(
    app: &mut App,
    terminal: &mut RatatuiTerminal<CrosstermBackend<std::io::Stdout>>,
) -> color_eyre::Result<()> {
    let wt_manager = WorktreeManager::new().ok();
    let mut tab_area = Rect::default();
    let mut quit_button_x = 0u16;
    let mut main_area = Rect::default();

    // Track last diff refresh time for polling
    let mut last_diff_refresh = Instant::now();
    const DIFF_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

    // Track last worktree check time
    let mut last_worktree_check = Instant::now();
    const WORKTREE_CHECK_INTERVAL: Duration = Duration::from_secs(2);

    loop {
        // Refresh diff viewers periodically
        if last_diff_refresh.elapsed() >= DIFF_REFRESH_INTERVAL {
            for tab in &mut app.tabs {
                tab.refresh_diff_viewer();
            }
            last_diff_refresh = Instant::now();
        }

        // Check for removed worktrees periodically and remove their tabs
        if last_worktree_check.elapsed() >= WORKTREE_CHECK_INTERVAL {
            if let Some(ref manager) = wt_manager
                && let Ok(existing_worktrees) = manager.list()
            {
                let existing_paths: std::collections::HashSet<_> =
                    existing_worktrees.iter().map(|wt| &wt.path).collect();

                // Find tabs whose worktree paths no longer exist
                let tabs_to_remove: Vec<usize> = app
                    .tabs
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, tab)| {
                        if let TabKind::Worktree { ref path, .. } = tab.kind
                            && !existing_paths.contains(path)
                        {
                            return Some(idx);
                        }
                        None
                    })
                    .collect();

                // Remove tabs in reverse order to maintain correct indices
                for idx in tabs_to_remove.into_iter().rev() {
                    app.remove_tab(idx);
                }
            }
            last_worktree_check = Instant::now();
        }

        terminal.draw(|frame| {
            (tab_area, quit_button_x, main_area) = render::render(app, frame);
        })?;

        if !event::poll(Duration::from_millis(16))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                // Dialog takes priority
                if !matches!(app.dialog, Dialog::None) {
                    process_dialog_key(app, &key, &wt_manager);
                    continue;
                }

                let action = if app.current_tab().is_control_panel() {
                    process_control_panel_key(app, &key, &wt_manager)
                } else {
                    process_terminal_key(app, &key, &wt_manager)
                };

                if matches!(action, KeyAction::Quit) {
                    return Ok(());
                }
            }
            Event::Mouse(mouse) => {
                if process_mouse_click(app, &mouse, tab_area, quit_button_x, main_area) {
                    return Ok(());
                }
            }
            _ => {}
        }
    }
}
