use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use alacritty_terminal::event::{Event, EventListener};
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
        cmd.cwd(std::env::current_dir()?);

        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        let size = TermSize::new(cols as usize, rows as usize);
        let term = Term::new(Config::default(), &size, Listener);
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

        for cell in content.display_iter {
            let x = cell.point.column.0 as u16;
            let y = cell.point.line.0 as u16;

            if x >= area.width || y >= area.height {
                continue;
            }

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

pub fn convert_color(color: alacritty_terminal::vte::ansi::Color) -> Color {
    use alacritty_terminal::vte::ansi::Color as AC;
    use alacritty_terminal::vte::ansi::NamedColor;

    match color {
        AC::Named(named) => match named {
            NamedColor::Black => Color::Black,
            NamedColor::Red => Color::Red,
            NamedColor::Green => Color::Green,
            NamedColor::Yellow => Color::Yellow,
            NamedColor::Blue => Color::Blue,
            NamedColor::Magenta => Color::Magenta,
            NamedColor::Cyan => Color::Cyan,
            NamedColor::White => Color::White,
            NamedColor::BrightBlack => Color::DarkGray,
            NamedColor::BrightRed => Color::LightRed,
            NamedColor::BrightGreen => Color::LightGreen,
            NamedColor::BrightYellow => Color::LightYellow,
            NamedColor::BrightBlue => Color::LightBlue,
            NamedColor::BrightMagenta => Color::LightMagenta,
            NamedColor::BrightCyan => Color::LightCyan,
            NamedColor::BrightWhite => Color::White,
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

        assert_eq!(convert_color(AC::Named(NamedColor::Red)), Color::Red);
        assert_eq!(convert_color(AC::Named(NamedColor::Green)), Color::Green);
        assert_eq!(convert_color(AC::Named(NamedColor::BrightBlue)), Color::LightBlue);
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
