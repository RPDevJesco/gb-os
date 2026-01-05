//! GUI Subsystem
//!
//! Provides a simple, clean graphical interface for primitive graphics events.
//!

pub mod font;
pub mod font_4x6;
pub mod font_8x8;
pub mod framebuffer;
pub mod layout;

pub use framebuffer::Framebuffer;

/// Rectangle structure
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.width as i32 &&
            py >= self.y && py < self.y + self.height as i32
    }

    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }
}

/// Point structure
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// RGB Color
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn from_u32(val: u32) -> Self {
        Self {
            r: ((val >> 16) & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            b: (val & 0xFF) as u8,
        }
    }

    pub const fn to_u32(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    // Plan 9 inspired colors
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const PALEYELLOW: Color = Color::rgb(255, 255, 224);
    pub const PALEBLUE: Color = Color::rgb(224, 224, 255);
    pub const PALEGREEN: Color = Color::rgb(224, 255, 224);
    pub const MEDBLUE: Color = Color::rgb(0, 0, 153);
    pub const GREYBLUE: Color = Color::rgb(102, 153, 153);
    pub const PALEGREYBLUE: Color = Color::rgb(156, 182, 182);
    pub const DARKGREY: Color = Color::rgb(102, 102, 102);
    pub const LIGHTGREY: Color = Color::rgb(192, 192, 192);
    pub const BORDER: Color = Color::rgb(153, 153, 153);
}
