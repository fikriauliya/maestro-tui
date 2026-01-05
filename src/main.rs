use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Paragraph},
    DefaultTerminal, Frame,
};

#[derive(Default, PartialEq)]
enum Pane {
    #[default]
    Left,
    Right,
}

#[derive(Default)]
struct App {
    focused: Pane,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut app = App::default();
    let terminal = ratatui::init();
    let result = run(&mut app, terminal);
    ratatui::restore();
    result
}

fn run(app: &mut App, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
    loop {
        terminal.draw(|frame| render(app, frame))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Left | KeyCode::Char('h') => app.focused = Pane::Left,
                KeyCode::Right | KeyCode::Char('l') => app.focused = Pane::Right,
                _ => {}
            }
        }
    }
    Ok(())
}

fn render(app: &App, frame: &mut Frame) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(frame.area());

    let left_block = Block::bordered()
        .title("Left")
        .border_style(border_style(app.focused == Pane::Left));
    let right_block = Block::bordered()
        .title("Right")
        .border_style(border_style(app.focused == Pane::Right));

    let left_pane = Paragraph::new("Pane 1").block(left_block);
    let right_pane = Paragraph::new("Pane 2").block(right_block);

    frame.render_widget(left_pane, left);
    frame.render_widget(right_pane, right);
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}
