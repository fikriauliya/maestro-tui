//! Flexoki dark theme colors
//!
//! Reference: https://stephango.com/flexoki
//!
//! This module provides a centralized definition of all Flexoki theme colors
//! used throughout the application.

#![allow(dead_code)] // Complete Flexoki palette, not all colors used yet

use ratatui::style::{Color, Style};

// Base colors
pub const BLACK: Color = Color::Rgb(0x10, 0x0F, 0x0F);
pub const BG: Color = Color::Rgb(0x1C, 0x1B, 0x1A);
pub const BG_2: Color = Color::Rgb(0x28, 0x27, 0x26);
pub const UI: Color = Color::Rgb(0x34, 0x33, 0x31);
pub const UI_2: Color = Color::Rgb(0x40, 0x3E, 0x3C);
pub const UI_3: Color = Color::Rgb(0x57, 0x56, 0x53);
pub const TX_3: Color = Color::Rgb(0x6F, 0x6E, 0x69);
pub const TX_2: Color = Color::Rgb(0x87, 0x85, 0x80);
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

/// Border style for focused/unfocused panes
pub fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(CYAN)
    } else {
        Style::default().fg(TX_3)
    }
}

/// Style for active tab in the tab bar
pub fn active_tab_style() -> Style {
    Style::default().fg(BLACK).bg(CYAN)
}

/// Style for inactive tabs in the tab bar
pub fn inactive_tab_style() -> Style {
    Style::default().fg(TX_2).bg(UI)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_border_style_focused() {
        let style = border_style(true);
        assert_eq!(style.fg, Some(CYAN));
    }

    #[test]
    fn test_border_style_unfocused() {
        let style = border_style(false);
        assert_eq!(style.fg, Some(TX_3));
    }

    #[test]
    fn test_active_tab_style() {
        let style = active_tab_style();
        assert_eq!(style.fg, Some(BLACK));
        assert_eq!(style.bg, Some(CYAN));
    }

    #[test]
    fn test_inactive_tab_style() {
        let style = inactive_tab_style();
        assert_eq!(style.fg, Some(TX_2));
        assert_eq!(style.bg, Some(UI));
    }
}
