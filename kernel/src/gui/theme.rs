//! Plan 9 Inspired Theme
//!
//! Colors and styling based on Plan 9's rio window manager.

use super::Color;

/// Theme configuration
pub struct Theme {
    /// Desktop background color
    pub desktop_bg: Color,
    /// Window background color
    pub window_bg: Color,
    /// Window title bar color (active)
    pub title_active: Color,
    /// Window title bar color (inactive)
    pub title_inactive: Color,
    /// Title text color (active)
    pub title_text_active: Color,
    /// Title text color (inactive)
    pub title_text_inactive: Color,
    /// Window border color
    pub border: Color,
    /// Text color
    pub text: Color,
    /// Selection/highlight color
    pub selection: Color,
    /// Scrollbar color
    pub scrollbar: Color,
    /// Button face color
    pub button_face: Color,
}

impl Theme {
    /// Default Plan 9-style theme
    pub const fn plan9() -> Self {
        Self {
            desktop_bg: Color::rgb(85, 170, 170),       // Plan 9 teal
            window_bg: Color::PALEYELLOW,               // Classic yellow
            title_active: Color::rgb(85, 170, 170),     // Teal
            title_inactive: Color::rgb(153, 153, 153),  // Gray
            title_text_active: Color::BLACK,
            title_text_inactive: Color::WHITE,
            border: Color::rgb(153, 153, 153),
            text: Color::BLACK,
            selection: Color::rgb(0, 0, 170),
            scrollbar: Color::rgb(204, 204, 153),
            button_face: Color::LIGHTGREY,
        }
    }
    
    /// Dark theme
    pub const fn dark() -> Self {
        Self {
            desktop_bg: Color::rgb(32, 32, 32),
            window_bg: Color::rgb(48, 48, 48),
            title_active: Color::rgb(64, 96, 128),
            title_inactive: Color::rgb(64, 64, 64),
            title_text_active: Color::WHITE,
            title_text_inactive: Color::rgb(160, 160, 160),
            border: Color::rgb(96, 96, 96),
            text: Color::rgb(224, 224, 224),
            selection: Color::rgb(64, 128, 192),
            scrollbar: Color::rgb(80, 80, 80),
            button_face: Color::rgb(64, 64, 64),
        }
    }
    
    /// Light theme
    pub const fn light() -> Self {
        Self {
            desktop_bg: Color::rgb(192, 192, 192),
            window_bg: Color::WHITE,
            title_active: Color::rgb(0, 0, 128),
            title_inactive: Color::rgb(128, 128, 128),
            title_text_active: Color::WHITE,
            title_text_inactive: Color::rgb(192, 192, 192),
            border: Color::rgb(64, 64, 64),
            text: Color::BLACK,
            selection: Color::rgb(0, 0, 128),
            scrollbar: Color::rgb(192, 192, 192),
            button_face: Color::rgb(192, 192, 192),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::plan9()
    }
}

// Global theme
static mut CURRENT_THEME: Theme = Theme::plan9();

/// Get the current theme
pub fn current() -> &'static Theme {
    unsafe { &CURRENT_THEME }
}

/// Set the current theme
pub fn set(theme: Theme) {
    unsafe { CURRENT_THEME = theme; }
}
