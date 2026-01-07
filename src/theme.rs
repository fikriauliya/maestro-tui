//! Theme support for the maestro-tui application.
//!
//! Provides multiple color themes that can be selected at runtime.

#![allow(dead_code)]

use ratatui::style::Color;

/// Accent colors used throughout the UI
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccentColors {
    pub red: Color,
    pub orange: Color,
    pub yellow: Color,
    pub green: Color,
    pub cyan: Color,
    pub blue: Color,
    pub purple: Color,
    pub magenta: Color,
}

/// A color theme for the application
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    /// Background color
    pub bg: Color,
    /// Secondary background
    pub bg2: Color,
    /// UI element background
    pub ui: Color,
    /// Muted text color
    pub tx_muted: Color,
    /// Normal text color
    pub tx: Color,
    /// Bright/highlight color
    pub highlight: Color,
    /// Primary accent (used for focus, active tabs)
    pub accent: Color,
    /// Accent colors for semantic highlighting
    pub accents: AccentColors,
}

impl Theme {
    /// Get border color based on focus state
    pub fn border_color(&self, focused: bool) -> Color {
        if focused { self.accent } else { self.tx_muted }
    }

    /// Get active tab colors (fg, bg)
    pub fn active_tab_colors(&self) -> (Color, Color) {
        (self.bg, self.accent)
    }

    /// Get inactive tab colors (fg, bg)
    pub fn inactive_tab_colors(&self) -> (Color, Color) {
        (self.tx, self.ui)
    }

    /// Get active control tab colors (fg, bg) - uses purple to distinguish from worktree tabs
    pub fn active_control_tab_colors(&self) -> (Color, Color) {
        (self.bg, self.accents.purple)
    }

    /// Get inactive control tab colors (fg, bg)
    pub fn inactive_control_tab_colors(&self) -> (Color, Color) {
        (self.accents.purple, self.ui)
    }

    /// Get status bar key style colors (fg, bg)
    pub fn status_key_colors(&self) -> (Color, Color) {
        (self.bg, self.accents.green)
    }

    /// Get status bar action text color
    pub fn status_action_color(&self) -> Color {
        self.highlight
    }
}

// ============================================================================
// Flexoki Dark - https://stephango.com/flexoki
// ============================================================================
pub const FLEXOKI_DARK: Theme = Theme {
    name: "Flexoki Dark",
    bg: Color::Rgb(0x1C, 0x1B, 0x1A),
    bg2: Color::Rgb(0x28, 0x27, 0x26),
    ui: Color::Rgb(0x34, 0x33, 0x31),
    tx_muted: Color::Rgb(0x6F, 0x6E, 0x69),
    tx: Color::Rgb(0x87, 0x85, 0x80),
    highlight: Color::Rgb(0xCE, 0xCB, 0xC4),
    accent: Color::Rgb(0x3A, 0xA9, 0x9F), // Cyan
    accents: AccentColors {
        red: Color::Rgb(0xD1, 0x4D, 0x41),
        orange: Color::Rgb(0xDA, 0x70, 0x2C),
        yellow: Color::Rgb(0xD0, 0xA2, 0x15),
        green: Color::Rgb(0x87, 0x9A, 0x39),
        cyan: Color::Rgb(0x3A, 0xA9, 0x9F),
        blue: Color::Rgb(0x43, 0x85, 0xBE),
        purple: Color::Rgb(0x8B, 0x7E, 0xC8),
        magenta: Color::Rgb(0xCE, 0x5D, 0x97),
    },
};

// ============================================================================
// Dracula - https://draculatheme.com
// ============================================================================
pub const DRACULA: Theme = Theme {
    name: "Dracula",
    bg: Color::Rgb(0x28, 0x2A, 0x36),
    bg2: Color::Rgb(0x44, 0x47, 0x5A),
    ui: Color::Rgb(0x44, 0x47, 0x5A),
    tx_muted: Color::Rgb(0x62, 0x72, 0xA4),
    tx: Color::Rgb(0xBD, 0xBF, 0xC3),
    highlight: Color::Rgb(0xF8, 0xF8, 0xF2),
    accent: Color::Rgb(0xBD, 0x93, 0xF9), // Purple
    accents: AccentColors {
        red: Color::Rgb(0xFF, 0x55, 0x55),
        orange: Color::Rgb(0xFF, 0xB8, 0x6C),
        yellow: Color::Rgb(0xF1, 0xFA, 0x8C),
        green: Color::Rgb(0x50, 0xFA, 0x7B),
        cyan: Color::Rgb(0x8B, 0xE9, 0xFD),
        blue: Color::Rgb(0x6B, 0xE5, 0xF3),
        purple: Color::Rgb(0xBD, 0x93, 0xF9),
        magenta: Color::Rgb(0xFF, 0x79, 0xC6),
    },
};

// ============================================================================
// Nord - https://www.nordtheme.com
// ============================================================================
pub const NORD: Theme = Theme {
    name: "Nord",
    bg: Color::Rgb(0x2E, 0x34, 0x40),
    bg2: Color::Rgb(0x3B, 0x42, 0x52),
    ui: Color::Rgb(0x43, 0x4C, 0x5E),
    tx_muted: Color::Rgb(0x4C, 0x56, 0x6A),
    tx: Color::Rgb(0xD8, 0xDE, 0xE9),
    highlight: Color::Rgb(0xEC, 0xEF, 0xF4),
    accent: Color::Rgb(0x88, 0xC0, 0xD0), // Frost
    accents: AccentColors {
        red: Color::Rgb(0xBF, 0x61, 0x6A),
        orange: Color::Rgb(0xD0, 0x87, 0x70),
        yellow: Color::Rgb(0xEB, 0xCB, 0x8B),
        green: Color::Rgb(0xA3, 0xBE, 0x8C),
        cyan: Color::Rgb(0x88, 0xC0, 0xD0),
        blue: Color::Rgb(0x81, 0xA1, 0xC1),
        purple: Color::Rgb(0xB4, 0x8E, 0xAD),
        magenta: Color::Rgb(0xB4, 0x8E, 0xAD),
    },
};

// ============================================================================
// Catppuccin Mocha - https://catppuccin.com
// ============================================================================
pub const CATPPUCCIN_MOCHA: Theme = Theme {
    name: "Catppuccin",
    bg: Color::Rgb(0x1E, 0x1E, 0x2E),
    bg2: Color::Rgb(0x31, 0x32, 0x44),
    ui: Color::Rgb(0x45, 0x47, 0x5A),
    tx_muted: Color::Rgb(0x6C, 0x70, 0x86),
    tx: Color::Rgb(0xA6, 0xAD, 0xC8),
    highlight: Color::Rgb(0xCD, 0xD6, 0xF4),
    accent: Color::Rgb(0x89, 0xB4, 0xFA), // Blue
    accents: AccentColors {
        red: Color::Rgb(0xF3, 0x8B, 0xA8),
        orange: Color::Rgb(0xFA, 0xB3, 0x87),
        yellow: Color::Rgb(0xF9, 0xE2, 0xAF),
        green: Color::Rgb(0xA6, 0xE3, 0xA1),
        cyan: Color::Rgb(0x94, 0xE2, 0xD5),
        blue: Color::Rgb(0x89, 0xB4, 0xFA),
        purple: Color::Rgb(0xCB, 0xA6, 0xF7),
        magenta: Color::Rgb(0xF5, 0xC2, 0xE7),
    },
};

// ============================================================================
// Gruvbox Dark - https://github.com/morhetz/gruvbox
// ============================================================================
pub const GRUVBOX_DARK: Theme = Theme {
    name: "Gruvbox",
    bg: Color::Rgb(0x28, 0x28, 0x28),
    bg2: Color::Rgb(0x3C, 0x38, 0x36),
    ui: Color::Rgb(0x50, 0x49, 0x45),
    tx_muted: Color::Rgb(0x92, 0x83, 0x74),
    tx: Color::Rgb(0xBD, 0xAE, 0x93),
    highlight: Color::Rgb(0xEB, 0xDB, 0xB2),
    accent: Color::Rgb(0xFE, 0x80, 0x19), // Orange
    accents: AccentColors {
        red: Color::Rgb(0xFB, 0x49, 0x34),
        orange: Color::Rgb(0xFE, 0x80, 0x19),
        yellow: Color::Rgb(0xFA, 0xBD, 0x2F),
        green: Color::Rgb(0xB8, 0xBB, 0x26),
        cyan: Color::Rgb(0x8E, 0xC0, 0x7C),
        blue: Color::Rgb(0x83, 0xA5, 0x98),
        purple: Color::Rgb(0xD3, 0x86, 0x9B),
        magenta: Color::Rgb(0xD3, 0x86, 0x9B),
    },
};

/// All available themes
pub const ALL_THEMES: &[Theme] = &[FLEXOKI_DARK, DRACULA, NORD, CATPPUCCIN_MOCHA, GRUVBOX_DARK];

/// Get the default theme
pub fn default_theme() -> Theme {
    FLEXOKI_DARK
}

// Legacy color constants for backwards compatibility
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
pub const RED: Color = Color::Rgb(0xD1, 0x4D, 0x41);
pub const ORANGE: Color = Color::Rgb(0xDA, 0x70, 0x2C);
pub const YELLOW: Color = Color::Rgb(0xD0, 0xA2, 0x15);
pub const GREEN: Color = Color::Rgb(0x87, 0x9A, 0x39);
pub const CYAN: Color = Color::Rgb(0x3A, 0xA9, 0x9F);
pub const BLUE: Color = Color::Rgb(0x43, 0x85, 0xBE);
pub const PURPLE: Color = Color::Rgb(0x8B, 0x7E, 0xC8);
pub const MAGENTA: Color = Color::Rgb(0xCE, 0x5D, 0x97);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_themes_not_empty() {
        assert!(!ALL_THEMES.is_empty());
    }

    #[test]
    fn test_default_theme_in_all_themes() {
        let default = default_theme();
        assert!(ALL_THEMES.iter().any(|t| t.name == default.name));
    }

    #[test]
    fn test_theme_border_colors_differ() {
        let theme = &FLEXOKI_DARK;
        assert_ne!(theme.border_color(true), theme.border_color(false));
    }

    #[test]
    fn test_themes_have_unique_names() {
        let names: Vec<_> = ALL_THEMES.iter().map(|t| t.name).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len());
    }
}
