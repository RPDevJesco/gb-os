//! Text console for scrolling output
//!
//! This module provides:
//! - Scrolling text console with colors
//! - StringWriter for positioned output
//! - fmt::Write implementation for use with write!() macro
//!
//! Text rendering primitives are in framebuffer.rs

use core::fmt::Write;
use crate::display::framebuffer::{Framebuffer, CHAR_WIDTH, CHAR_HEIGHT};

// ============================================================================
// Console Constants
// ============================================================================

/// Line height (font height + spacing)
pub const LINE_HEIGHT: u32 = CHAR_HEIGHT + 2;

/// Console margin from screen edge
pub const MARGIN: u32 = 8;

// ============================================================================
// Text Console
// ============================================================================

/// A scrolling text console with automatic line wrapping
pub struct Console<'a> {
    fb: &'a mut Framebuffer,
    x: u32,
    y: u32,
    fg: u32,
    bg: u32,
}

impl<'a> Console<'a> {
    /// Create a new console at the default position (top-left with margin)
    pub fn new(fb: &'a mut Framebuffer, fg: u32, bg: u32) -> Self {
        Self {
            fb,
            x: MARGIN,
            y: MARGIN,
            fg,
            bg,
        }
    }

    /// Create a new console at a specific position
    pub fn at(fb: &'a mut Framebuffer, x: u32, y: u32, fg: u32, bg: u32) -> Self {
        Self { fb, x, y, fg, bg }
    }

    /// Get the underlying framebuffer (releases mutable borrow)
    pub fn into_framebuffer(self) -> &'a mut Framebuffer {
        self.fb
    }

    /// Move to the next line
    pub fn newline(&mut self) {
        self.x = MARGIN;
        self.y += LINE_HEIGHT;

        // Wrap to top if we've gone past the bottom
        if self.y + LINE_HEIGHT > self.fb.height {
            self.y = MARGIN;
        }
    }

    /// Print a string (handles newlines)
    pub fn print(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                self.newline();
                continue;
            }

            self.fb.draw_char(self.x, self.y, c as u8, self.fg, self.bg);
            self.x += CHAR_WIDTH;

            // Wrap if we've hit the right edge
            if self.x + CHAR_WIDTH > self.fb.width - MARGIN {
                self.newline();
            }
        }
    }

    /// Print a string followed by a newline
    pub fn println(&mut self, s: &str) {
        self.print(s);
        self.newline();
    }

    /// Set foreground and background colors
    pub fn set_color(&mut self, fg: u32, bg: u32) {
        self.fg = fg;
        self.bg = bg;
    }

    /// Get current cursor position
    pub fn position(&self) -> (u32, u32) {
        (self.x, self.y)
    }

    /// Set cursor position
    pub fn set_position(&mut self, x: u32, y: u32) {
        self.x = x;
        self.y = y;
    }

    /// Clear the screen and reset cursor
    pub fn clear(&mut self) {
        self.fb.clear(self.bg);
        self.x = MARGIN;
        self.y = MARGIN;
    }
}

impl Write for Console<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.print(s);
        Ok(())
    }
}

// ============================================================================
// String Writer (for positioned write!() output)
// ============================================================================

/// A writer that outputs text at a fixed position (no wrapping)
///
/// Useful for status displays where you want to write!() to a specific location
pub struct StringWriter<'a> {
    fb: &'a mut Framebuffer,
    x: u32,
    y: u32,
    fg: u32,
    bg: u32,
}

impl<'a> StringWriter<'a> {
    /// Create a new string writer at (x, y)
    pub fn new(fb: &'a mut Framebuffer, x: u32, y: u32, fg: u32, bg: u32) -> Self {
        Self { fb, x, y, fg, bg }
    }
}

impl Write for StringWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.fb.draw_char(self.x, self.y, c as u8, self.fg, self.bg);
            self.x += CHAR_WIDTH;
        }
        Ok(())
    }
}

// ============================================================================
// Centered Text Helper
// ============================================================================

/// Draw a string centered horizontally on the screen
pub fn draw_centered(fb: &mut Framebuffer, y: u32, s: &str, fg: u32, bg: u32) {
    let width = s.len() as u32 * CHAR_WIDTH;
    let x = (fb.width.saturating_sub(width)) / 2;
    fb.draw_str(x, y, s, fg, bg);
}
