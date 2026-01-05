use ratatui::style::{Color, Style};

/// Border style for focused/unfocused panes
pub fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

/// Style for the mode indicator in the status bar
pub fn mode_style(is_insert: bool) -> Style {
    if is_insert {
        Style::default().fg(Color::Black).bg(Color::Green)
    } else {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    }
}

/// Style for active tab in the tab bar
pub fn active_tab_style() -> Style {
    Style::default().fg(Color::Black).bg(Color::Cyan)
}

/// Style for inactive tabs in the tab bar
pub fn inactive_tab_style() -> Style {
    Style::default().fg(Color::White).bg(Color::DarkGray)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_border_style_focused() {
        let style = border_style(true);
        assert_eq!(style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_border_style_unfocused() {
        let style = border_style(false);
        assert_eq!(style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn test_mode_style_insert() {
        let style = mode_style(true);
        assert_eq!(style.fg, Some(Color::Black));
        assert_eq!(style.bg, Some(Color::Green));
    }

    #[test]
    fn test_mode_style_normal() {
        let style = mode_style(false);
        assert_eq!(style.fg, Some(Color::Black));
        assert_eq!(style.bg, Some(Color::Cyan));
    }

    #[test]
    fn test_active_tab_style() {
        let style = active_tab_style();
        assert_eq!(style.fg, Some(Color::Black));
        assert_eq!(style.bg, Some(Color::Cyan));
    }

    #[test]
    fn test_inactive_tab_style() {
        let style = inactive_tab_style();
        assert_eq!(style.fg, Some(Color::White));
        assert_eq!(style.bg, Some(Color::DarkGray));
    }
}
