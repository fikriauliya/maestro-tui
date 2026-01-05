use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::term::{test::TermSize, Config};
use alacritty_terminal::vte::ansi::Processor;
use alacritty_terminal::Term;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

struct Listener;

impl EventListener for Listener {
    fn send_event(&self, _event: Event) {}
}

pub struct Terminal {
    term: Arc<Mutex<Term<Listener>>>,
    pty_writer: Box<dyn Write + Send>,
    _reader_thread: thread::JoinHandle<()>,
}

impl Terminal {
    pub fn new(cols: u16, rows: u16) -> color_eyre::Result<Self> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        Self::with_command(cols, rows, &shell, &[])
    }

    pub fn with_command(cols: u16, rows: u16, program: &str, args: &[&str]) -> color_eyre::Result<Self> {
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

        let _child = pty_pair
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
            _reader_thread: reader_thread,
        })
    }

    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.pty_writer.write_all(data)?;
        self.pty_writer.flush()
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let size = TermSize::new(cols as usize, rows as usize);
        self.term.lock().unwrap().resize(size);
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

fn convert_color(color: alacritty_terminal::vte::ansi::Color) -> Color {
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
