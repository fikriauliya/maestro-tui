use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::term::{test::TermSize, Config};
use alacritty_terminal::vte::ansi::Processor;
use alacritty_terminal::Term;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

#[cfg(test)]
use crate::pty::PtyBackend;

struct Listener;

impl EventListener for Listener {
    fn send_event(&self, _event: Event) {}
}

pub struct Terminal {
    term: Arc<Mutex<Term<Listener>>>,
    pty_writer: Box<dyn Write + Send>,
    pty_master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    reader_thread: Option<JoinHandle<()>>,
}

impl Terminal {
    pub fn new(cols: u16, rows: u16) -> color_eyre::Result<Self> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        Self::with_command(cols, rows, &shell, &[])
    }

    pub fn with_command(
        cols: u16,
        rows: u16,
        program: &str,
        args: &[&str],
    ) -> color_eyre::Result<Self> {
        Self::with_command_in_dir(cols, rows, program, args, &std::env::current_dir()?)
    }

    pub fn with_command_in_dir(
        cols: u16,
        rows: u16,
        program: &str,
        args: &[&str],
        cwd: &Path,
    ) -> color_eyre::Result<Self> {
        let pty_system = native_pty_system();

        let pty_pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        let mut cmd = CommandBuilder::new(program);
        for arg in args {
            cmd.arg(*arg);
        }
        cmd.cwd(cwd);

        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        let size = TermSize::new(cols as usize, rows as usize);
        let config = Config {
            scrolling_history: 1000, // Limit scrollback to prevent performance issues
            ..Config::default()
        };
        let term = Term::new(config, &size, Listener);
        let term = Arc::new(Mutex::new(term));

        let mut reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;
        let term_clone = Arc::clone(&term);

        let reader_thread = thread::spawn(move || {
            let mut processor: Processor = Processor::new();
            let mut buf = [0u8; 4096];

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let mut term = term_clone.lock().unwrap();
                        processor.advance(&mut *term, &buf[..n]);
                    }
                    Err(_) => break,
                }
            }
        });

        let writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        Ok(Self {
            term,
            pty_writer: writer,
            pty_master: pty_pair.master,
            child,
            reader_thread: Some(reader_thread),
        })
    }

    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.pty_writer.write_all(data)?;
        self.pty_writer.flush()
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        // Resize the PTY (notifies child process via SIGWINCH)
        let _ = self.pty_master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        // Resize the terminal emulator state
        let size = TermSize::new(cols as usize, rows as usize);
        self.term.lock().unwrap().resize(size);
    }

    pub fn scroll(&mut self, lines: i32) {
        let mut term = self.term.lock().unwrap();
        let current_offset = term.grid().display_offset();
        let history_size = term.grid().history_size();

        // Skip scroll if already at boundary
        if lines > 0 && current_offset >= history_size {
            return; // Already at top of scrollback
        }
        if lines < 0 && current_offset == 0 {
            return; // Already at bottom
        }

        term.scroll_display(Scroll::Delta(lines));
    }

    pub fn widget(&self) -> TerminalWidget {
        TerminalWidget {
            term: Arc::clone(&self.term),
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Kill the child process
        let _ = self.child.kill();
        // Wait for child to exit (prevents zombie processes)
        let _ = self.child.wait();
        // Join the reader thread (it will exit once the PTY is closed)
        if let Some(thread) = self.reader_thread.take() {
            let _ = thread.join();
        }
    }
}

/// Testable terminal that uses a mock PTY backend
#[cfg(test)]
pub struct TestTerminal {
    term: Arc<Mutex<Term<Listener>>>,
    pty: Box<dyn PtyBackend>,
}

#[cfg(test)]
impl TestTerminal {
    pub fn new<P: PtyBackend + 'static>(cols: u16, rows: u16, pty: P) -> Self {
        let size = TermSize::new(cols as usize, rows as usize);
        let term = Term::new(Config::default(), &size, Listener);

        Self {
            term: Arc::new(Mutex::new(term)),
            pty: Box::new(pty),
        }
    }

    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.pty.write(data)
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let size = TermSize::new(cols as usize, rows as usize);
        self.term.lock().unwrap().resize(size);
        self.pty.resize(cols, rows);
    }

    pub fn process_output(&mut self) {
        if let Ok(Some(data)) = self.pty.try_read() {
            let mut processor: Processor = Processor::new();
            let mut term = self.term.lock().unwrap();
            processor.advance(&mut *term, &data);
        }
    }

    pub fn widget(&self) -> TerminalWidget {
        TerminalWidget {
            term: Arc::clone(&self.term),
        }
    }
}

pub struct TerminalWidget {
    term: Arc<Mutex<Term<Listener>>>,
}

impl Widget for TerminalWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let term = self.term.lock().unwrap();
        let content = term.renderable_content();

        // Get the display offset to calculate screen y from grid line
        let display_offset = content.display_offset as i32;

        for cell in content.display_iter {
            let x = cell.point.column.0 as u16;
            // Convert grid line to screen y: line 0 is at screen_lines-1, negative lines are above
            // With display_offset, the topmost visible line is -(display_offset)
            // Screen y = line - (-(display_offset)) = line + display_offset
            let grid_line = cell.point.line.0;
            let screen_y = grid_line + display_offset;

            if x >= area.width || screen_y < 0 || screen_y >= area.height as i32 {
                continue;
            }
            let y = screen_y as u16;

            let fg = convert_color(cell.fg);
            let bg = convert_color(cell.bg);
            let mut style = Style::default().fg(fg).bg(bg);

            if cell.flags.contains(CellFlags::BOLD) {
                style = style.add_modifier(Modifier::BOLD);
            }
            if cell.flags.contains(CellFlags::ITALIC) {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if cell.flags.intersects(CellFlags::ALL_UNDERLINES) {
                style = style.add_modifier(Modifier::UNDERLINED);
            }

            if let Some(buf_cell) = buf.cell_mut((area.x + x, area.y + y)) {
                buf_cell.set_char(cell.c);
                buf_cell.set_style(style);
            }
        }
    }
}

// Flexoki dark theme terminal colors
// Reference: https://stephango.com/flexoki
mod flexoki {
    use ratatui::style::Color;

    // Base colors
    pub const BLACK: Color = Color::Rgb(0x10, 0x0F, 0x0F);
    pub const TX_3: Color = Color::Rgb(0x6F, 0x6E, 0x69);
    pub const TX: Color = Color::Rgb(0xB7, 0xB5, 0xAC);
    pub const PAPER: Color = Color::Rgb(0xCE, 0xCB, 0xC4);

    // Accent colors (400 series for dark theme)
    pub const RED: Color = Color::Rgb(0xD1, 0x4D, 0x41);
    pub const ORANGE: Color = Color::Rgb(0xDA, 0x70, 0x2C);
    pub const YELLOW: Color = Color::Rgb(0xD0, 0xA2, 0x15);
    pub const GREEN: Color = Color::Rgb(0x87, 0x9A, 0x39);
    pub const CYAN: Color = Color::Rgb(0x3A, 0xA9, 0x9F);
    pub const BLUE: Color = Color::Rgb(0x43, 0x85, 0xBE);
    pub const PURPLE: Color = Color::Rgb(0x8B, 0x7E, 0xC8);
    pub const MAGENTA: Color = Color::Rgb(0xCE, 0x5D, 0x97);
}

pub fn convert_color(color: alacritty_terminal::vte::ansi::Color) -> Color {
    use alacritty_terminal::vte::ansi::Color as AC;
    use alacritty_terminal::vte::ansi::NamedColor;

    match color {
        AC::Named(named) => match named {
            NamedColor::Black => flexoki::BLACK,
            NamedColor::Red => flexoki::RED,
            NamedColor::Green => flexoki::GREEN,
            NamedColor::Yellow => flexoki::YELLOW,
            NamedColor::Blue => flexoki::BLUE,
            NamedColor::Magenta => flexoki::MAGENTA,
            NamedColor::Cyan => flexoki::CYAN,
            NamedColor::White => flexoki::TX,
            NamedColor::BrightBlack => flexoki::TX_3,
            NamedColor::BrightRed => flexoki::ORANGE,
            NamedColor::BrightGreen => flexoki::GREEN,
            NamedColor::BrightYellow => flexoki::YELLOW,
            NamedColor::BrightBlue => flexoki::PURPLE,
            NamedColor::BrightMagenta => flexoki::MAGENTA,
            NamedColor::BrightCyan => flexoki::CYAN,
            NamedColor::BrightWhite => flexoki::PAPER,
            _ => Color::Reset,
        },
        AC::Spec(rgb) => Color::Rgb(rgb.r, rgb.g, rgb.b),
        AC::Indexed(idx) => Color::Indexed(idx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::mock::MockPty;

    #[test]
    fn test_terminal_write_with_mock() {
        let mock = MockPty::new(80, 24);
        let mock_clone = mock.clone();
        let mut term = TestTerminal::new(80, 24, mock);

        term.write(b"hello").unwrap();
        term.write(b"world").unwrap();

        let written = mock_clone.get_written();
        assert_eq!(written.len(), 2);
        assert_eq!(written[0], b"hello");
        assert_eq!(written[1], b"world");
    }

    #[test]
    fn test_terminal_resize_with_mock() {
        let mock = MockPty::new(80, 24);
        let mock_clone = mock.clone();
        let mut term = TestTerminal::new(80, 24, mock);

        term.resize(120, 40);

        assert_eq!(mock_clone.get_size(), (120, 40));
    }

    #[test]
    fn test_terminal_widget_creation() {
        let mock = MockPty::new(80, 24);
        let term = TestTerminal::new(80, 24, mock);

        // Just verify widget can be created without panic
        let _widget = term.widget();
    }

    #[test]
    fn test_terminal_process_output() {
        let mock = MockPty::new(80, 24);
        mock.queue_read(b"Hello".to_vec());
        let mut term = TestTerminal::new(80, 24, mock);

        // Process the queued output
        term.process_output();

        // Terminal should have processed the ANSI data
        // (We just verify it doesn't panic)
    }

    #[test]
    fn test_convert_color_named() {
        use alacritty_terminal::vte::ansi::Color as AC;
        use alacritty_terminal::vte::ansi::NamedColor;

        assert_eq!(convert_color(AC::Named(NamedColor::Red)), flexoki::RED);
        assert_eq!(convert_color(AC::Named(NamedColor::Green)), flexoki::GREEN);
        assert_eq!(convert_color(AC::Named(NamedColor::BrightBlue)), flexoki::PURPLE);
    }

    #[test]
    fn test_convert_color_rgb() {
        use alacritty_terminal::vte::ansi::Color as AC;
        use alacritty_terminal::vte::ansi::Rgb;

        let rgb = Rgb { r: 255, g: 128, b: 64 };
        assert_eq!(convert_color(AC::Spec(rgb)), Color::Rgb(255, 128, 64));
    }

    #[test]
    fn test_convert_color_indexed() {
        use alacritty_terminal::vte::ansi::Color as AC;

        assert_eq!(convert_color(AC::Indexed(42)), Color::Indexed(42));
    }
}
