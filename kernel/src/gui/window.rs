//! Window Management
//!
//! Plan 9 rio-style windows with minimal chrome.

use super::{Rect, Color, Framebuffer, theme};

/// Window title bar height
pub const TITLE_HEIGHT: u32 = 20;
/// Window border width
pub const BORDER_WIDTH: u32 = 3;

/// Window flags
#[derive(Debug, Clone, Copy)]
pub struct WindowFlags {
    pub visible: bool,
    pub focused: bool,
    pub movable: bool,
    pub resizable: bool,
    pub has_title: bool,
}

impl Default for WindowFlags {
    fn default() -> Self {
        Self {
            visible: true,
            focused: false,
            movable: true,
            resizable: true,
            has_title: true,
        }
    }
}

/// A window in the GUI
pub struct Window {
    /// Unique window ID
    pub id: u32,
    /// Window title
    title: [u8; 64],
    title_len: usize,
    /// Position and size (outer bounds including decoration)
    pub bounds: Rect,
    /// Window flags
    pub flags: WindowFlags,
    /// Content dimensions
    content_width: u32,
    content_height: u32,
    /// Dirty flag (needs redraw)
    dirty: bool,
}

impl Window {
    /// Create a new window
    pub fn new(id: u32, title: &str, x: i32, y: i32, width: u32, height: u32) -> Self {
        let mut title_buf = [0u8; 64];
        let title_bytes = title.as_bytes();
        let len = title_bytes.len().min(63);
        title_buf[..len].copy_from_slice(&title_bytes[..len]);

        // Calculate content size (inside decorations)
        let content_w = width.saturating_sub(BORDER_WIDTH * 2);
        let content_h = height.saturating_sub(TITLE_HEIGHT + BORDER_WIDTH);

        Self {
            id,
            title: title_buf,
            title_len: len,
            bounds: Rect::new(x, y, width, height),
            flags: WindowFlags::default(),
            content_width: content_w,
            content_height: content_h,
            dirty: true,
        }
    }

    /// Get window title
    pub fn title(&self) -> &str {
        core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")
    }

    /// Get content area rectangle (relative to window)
    pub fn content_rect(&self) -> Rect {
        Rect::new(
            BORDER_WIDTH as i32,
            TITLE_HEIGHT as i32,
            self.content_width,
            self.content_height,
        )
    }

    /// Get absolute content area rectangle
    pub fn content_rect_abs(&self) -> Rect {
        Rect::new(
            self.bounds.x + BORDER_WIDTH as i32,
            self.bounds.y + TITLE_HEIGHT as i32,
            self.content_width,
            self.content_height,
        )
    }

    /// Get title bar rectangle
    pub fn title_rect(&self) -> Rect {
        Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            TITLE_HEIGHT,
        )
    }

    /// Check if point is in title bar
    pub fn in_title_bar(&self, x: i32, y: i32) -> bool {
        self.title_rect().contains(x, y)
    }

    /// Check if point is in window bounds
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.bounds.contains(x, y)
    }

    /// Move window to new position
    pub fn move_to(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
        self.dirty = true;
    }

    /// Resize window
    pub fn resize(&mut self, width: u32, height: u32) {
        self.bounds.width = width.max(100);
        self.bounds.height = height.max(TITLE_HEIGHT + BORDER_WIDTH + 20);
        self.content_width = self.bounds.width.saturating_sub(BORDER_WIDTH * 2);
        self.content_height = self.bounds.height.saturating_sub(TITLE_HEIGHT + BORDER_WIDTH);
        self.dirty = true;
    }

    /// Draw the window to the framebuffer
    pub fn draw(&self, fb: &mut Framebuffer) {
        let theme = theme::current();

        // Window border
        fb.fill_rect(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
            theme.border,
        );

        // Title bar
        let title_color = if self.flags.focused {
            theme.title_active
        } else {
            theme.title_inactive
        };

        fb.fill_rect(
            self.bounds.x + 1,
            self.bounds.y + 1,
            self.bounds.width - 2,
            TITLE_HEIGHT - 1,
            title_color,
        );

        // Title text
        let text_color = if self.flags.focused {
            theme.title_text_active
        } else {
            theme.title_text_inactive
        };

        fb.draw_string(
            self.bounds.x + 6,
            self.bounds.y + 3,
            self.title(),
            text_color,
            Some(title_color),
        );

        // Content area
        fb.fill_rect(
            self.bounds.x + BORDER_WIDTH as i32,
            self.bounds.y + TITLE_HEIGHT as i32,
            self.content_width,
            self.content_height,
            theme.window_bg,
        );
    }

    /// Draw text in the content area (using theme colors)
    pub fn draw_text(&self, fb: &mut Framebuffer, x: i32, y: i32, text: &str, color: Color) {
        let theme = theme::current();
        let abs_x = self.bounds.x + BORDER_WIDTH as i32 + x;
        let abs_y = self.bounds.y + TITLE_HEIGHT as i32 + y;

        fb.draw_string(abs_x, abs_y, text, color, Some(theme.window_bg));
    }

    /// Draw text in the content area with custom colors
    pub fn draw_text_color(&self, fb: &mut Framebuffer, x: i32, y: i32, text: &str, fg: Color, bg: Color) {
        let abs_x = self.bounds.x + BORDER_WIDTH as i32 + x;
        let abs_y = self.bounds.y + TITLE_HEIGHT as i32 + y;

        fb.draw_string(abs_x, abs_y, text, fg, Some(bg));
    }

    /// Fill content area with color
    pub fn fill_content(&self, fb: &mut Framebuffer, color: Color) {
        fb.fill_rect(
            self.bounds.x + BORDER_WIDTH as i32,
            self.bounds.y + TITLE_HEIGHT as i32,
            self.content_width,
            self.content_height,
            color,
        );
    }

    /// Mark window as dirty (needs redraw)
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Check if window is dirty
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}
