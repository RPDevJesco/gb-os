//! Layout System for VGA Mode 13h
//!
//! Provides structured layout primitives for positioning UI elements
//! on the 320x200 VGA screen. Designed for both game overlay and ROM browser.
//!
//! # Design Principles
//! - No heap allocations (bare-metal compatible)
//! - Simple cursor-based vertical flow
//! - Automatic bounds checking
//! - Consistent spacing via element height constants
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::gui::layout::{Region, LayoutCursor, ElementHeight};
//!
//! // Define a region (right sidebar)
//! let region = Region::new(240, 0, 80, 200);
//!
//! // Create a cursor with padding
//! let mut cursor = region.cursor(4);
//!
//! // Render elements, cursor auto-advances
//! if cursor.fits(ElementHeight::LABEL) {
//!     draw_text(fb, cursor.x, cursor.take(ElementHeight::LABEL), "TITLE");
//! }
//! cursor.space(4);  // Add gap
//! ```
#[cfg(target_arch = "x86")]
use crate::graphics::vga_mode13h::{SCREEN_WIDTH, SCREEN_HEIGHT};

#[cfg(not(target_arch = "x86"))]
pub const SCREEN_WIDTH: usize = 320;
#[cfg(not(target_arch = "x86"))]
pub const SCREEN_HEIGHT: usize = 200;

// =============================================================================
// Game Boy Screen Layout (VGA Mode 13h: 320x200)
// =============================================================================

/// Game Boy native screen dimensions
pub const GB_WIDTH: usize = 160;
pub const GB_HEIGHT: usize = 144;

/// Game Boy screen position (centered horizontally, near top)
pub const GB_X: usize = (SCREEN_WIDTH - GB_WIDTH) / 2;  // 80
pub const GB_Y: usize = 28;  // Leaves room for overlay at bottom

/// Border around Game Boy screen
pub const GB_BORDER: usize = 4;
pub const GB_BORDER_COLOR: u8 = 0x08;  // Dark gray

/// Right edge of Game Boy screen (where sidebar can start)
pub const GB_RIGHT: usize = GB_X + GB_WIDTH;  // 240

/// Bottom edge of Game Boy screen
pub const GB_BOTTOM: usize = GB_Y + GB_HEIGHT;  // 172

// =============================================================================
// Element Heights
// =============================================================================

/// Standard heights for common UI elements (in pixels)
///
/// These are based on font sizes and typical padding:
/// - font_4x6: char height 6, cell height 7
/// - font_8x8: char height 8
pub mod element {
    /// Single line of text (4x6 font)
    pub const TEXT_4X6: usize = 7;

    /// Single line of text (8x8 font)
    pub const TEXT_8X8: usize = 8;

    /// Label with small gap after
    pub const LABEL: usize = 8;

    /// Section header with gap after
    pub const SECTION_HEADER: usize = 10;

    /// Party slot (species line + HP bar)
    pub const PARTY_SLOT: usize = 16;

    /// Badge row (label + 2 rows of badge boxes)
    pub const BADGE_ROW: usize = 24;

    /// List item in browser (8x8 font + padding)
    pub const LIST_ITEM: usize = 12;

    /// Small gap between related items
    pub const GAP_SMALL: usize = 2;

    /// Medium gap between sections
    pub const GAP_MEDIUM: usize = 4;

    /// Large gap between major sections
    pub const GAP_LARGE: usize = 8;
}

// =============================================================================
// Region
// =============================================================================

/// A rectangular region of the screen
#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Region {
    /// Create a new region
    pub const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self { x, y, width, height }
    }

    /// Full screen region
    pub const fn fullscreen() -> Self {
        Self::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT)
    }

    /// Top sidebar (above Game Boy screen, full width)
    pub const fn top_sidebar() -> Self {
        Self::new(0, 0, SCREEN_WIDTH, GB_Y - GB_BORDER)
    }

    /// Bottom sidebar (below Game Boy screen, full width)
    pub const fn bottom_sidebar() -> Self {
        const START_Y: usize = GB_BOTTOM + GB_BORDER;
        Self::new(0, START_Y, SCREEN_WIDTH, SCREEN_HEIGHT - START_Y)
    }

    /// Right sidebar (where game overlay goes)
    /// Positioned 9px right of Game Boy screen edge to reduce overlap
    pub const fn right_sidebar() -> Self {
        const SIDEBAR_OFFSET: usize = 4;  // One badge width
        Self::new(GB_RIGHT + SIDEBAR_OFFSET, 0, SCREEN_WIDTH - GB_RIGHT - SIDEBAR_OFFSET, SCREEN_HEIGHT)
    }

    /// Left sidebar (left of Game Boy screen)
    pub const fn left_sidebar() -> Self {
        const SIDEBAR_OFFSET: usize = 4;  // One badge width
        Self::new(0, 0, GB_X - SIDEBAR_OFFSET, SCREEN_HEIGHT)
    }

    /// Main content area (Game Boy screen region)
    pub const fn main_content() -> Self {
        Self::new(GB_X, GB_Y, GB_WIDTH, GB_HEIGHT)
    }

    /// Center panel for dialogs/menus
    pub const fn center_panel(width: usize, height: usize) -> Self {
        let x = (SCREEN_WIDTH - width) / 2;
        let y = (SCREEN_HEIGHT - height) / 2;
        Self::new(x, y, width, height)
    }

    /// Create a cursor for this region with specified padding
    pub const fn cursor(&self, padding: usize) -> LayoutCursor {
        LayoutCursor::new(
            self.x + padding,
            self.y + padding,
            self.width - padding * 2,
            self.height - padding * 2,
        )
    }

    /// Create a cursor with no padding
    pub const fn cursor_no_pad(&self) -> LayoutCursor {
        LayoutCursor::new(self.x, self.y, self.width, self.height)
    }

    /// Right edge x coordinate
    pub const fn right(&self) -> usize {
        self.x + self.width
    }

    /// Bottom edge y coordinate
    pub const fn bottom(&self) -> usize {
        self.y + self.height
    }

    /// Check if a point is within this region
    pub const fn contains(&self, px: usize, py: usize) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// Create a sub-region (relative coordinates)
    pub const fn sub_region(&self, x_off: usize, y_off: usize, w: usize, h: usize) -> Self {
        let max_w = self.width - x_off;
        let max_h = self.height - y_off;
        Self::new(
            self.x + x_off,
            self.y + y_off,
            if w < max_w { w } else { max_w },
            if h < max_h { h } else { max_h },
        )
    }
}

// =============================================================================
// Layout Cursor
// =============================================================================

/// Tracks current position during vertical layout flow
///
/// The cursor moves down as elements are placed, automatically
/// tracking available space and preventing overflow.
#[derive(Clone, Copy, Debug)]
pub struct LayoutCursor {
    /// Left edge of content area
    pub x: usize,
    /// Current Y position (moves down as elements placed)
    pub y: usize,
    /// Width of content area
    pub width: usize,
    /// Maximum Y position (bottom boundary)
    pub max_y: usize,
    /// Starting Y (for reset)
    start_y: usize,
}

impl LayoutCursor {
    /// Create a new cursor
    pub const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            max_y: y + height,
            start_y: y,
        }
    }

    /// Check if an element of given height fits at current position
    #[inline]
    pub const fn fits(&self, height: usize) -> bool {
        self.y + height <= self.max_y
    }

    /// Get remaining vertical space
    #[inline]
    pub const fn remaining(&self) -> usize {
        if self.y >= self.max_y {
            0
        } else {
            self.max_y - self.y
        }
    }

    /// Advance cursor by height, return the Y position before advancing
    ///
    /// This is the primary method for placing elements:
    /// ```rust,ignore
    /// let y = cursor.take(element::TEXT_4X6);
    /// draw_text(fb, cursor.x, y, "Hello");
    /// ```
    #[inline]
    pub fn take(&mut self, height: usize) -> usize {
        let y = self.y;
        let new_y = self.y + height;
        self.y = if new_y < self.max_y { new_y } else { self.max_y };
        y
    }

    /// Add vertical spacing without returning position
    #[inline]
    pub fn space(&mut self, pixels: usize) {
        let new_y = self.y + pixels;
        self.y = if new_y < self.max_y { new_y } else { self.max_y };
    }

    /// Skip an element's worth of space (for hidden elements)
    ///
    /// Use this when an element is conditionally hidden but you
    /// want to maintain consistent layout:
    /// ```rust,ignore
    /// if show_badges {
    ///     draw_badges(fb, cursor.x, cursor.take(element::BADGE_ROW), badges);
    /// } else {
    ///     cursor.skip(element::BADGE_ROW);  // Maintain spacing
    /// }
    /// ```
    #[inline]
    pub fn skip(&mut self, height: usize) {
        self.space(height);
    }

    /// Try to take space, returns Some(y) if fits, None if not
    #[inline]
    pub fn try_take(&mut self, height: usize) -> Option<usize> {
        if self.fits(height) {
            Some(self.take(height))
        } else {
            None
        }
    }

    /// Reset cursor to starting position
    #[inline]
    pub fn reset(&mut self) {
        self.y = self.start_y;
    }

    /// Get current position as (x, y) tuple
    #[inline]
    pub const fn pos(&self) -> (usize, usize) {
        (self.x, self.y)
    }

    /// Get right edge of content area
    #[inline]
    pub const fn right(&self) -> usize {
        self.x + self.width
    }

    /// Move to an absolute Y position (clamped to bounds)
    #[inline]
    pub fn move_to(&mut self, y: usize) {
        if y < self.start_y {
            self.y = self.start_y;
        } else if y > self.max_y {
            self.y = self.max_y;
        } else {
            self.y = y;
        }
    }

    /// Move to bottom minus offset (for bottom-anchored elements)
    #[inline]
    pub fn from_bottom(&mut self, offset: usize) {
        self.y = self.max_y.saturating_sub(offset);
    }
}

// =============================================================================
// Layout Helpers
// =============================================================================

/// Calculate centered X position for content of given width
#[inline]
pub const fn center_x(content_width: usize) -> usize {
    (SCREEN_WIDTH - content_width) / 2
}

/// Calculate centered X position within a region
#[inline]
pub const fn center_x_in(region: &Region, content_width: usize) -> usize {
    region.x + (region.width - content_width) / 2
}

/// Calculate text width for 4x6 font
#[inline]
pub const fn text_width_4x6(len: usize) -> usize {
    if len == 0 { 0 } else { len * 5 - 1 }  // CELL_WIDTH=5, minus trailing space
}

/// Calculate text width for 8x8 font
#[inline]
pub const fn text_width_8x8(len: usize) -> usize {
    len * 8
}

// =============================================================================
// Common Layouts
// =============================================================================

/// Pre-defined layout configurations for common use cases
pub mod layouts {
    use super::*;

    /// ROM browser layout constants
    pub mod browser {
        pub const TITLE_Y: usize = 15;
        pub const LIST_START_Y: usize = 45;
        pub const LIST_ITEM_HEIGHT: usize = 12;
        pub const LIST_X: usize = 40;
        pub const MAX_VISIBLE: usize = 10;
        pub const INSTRUCTIONS_Y: usize = 175;

        pub const BORDER_X: usize = 20;
        pub const BORDER_Y: usize = 10;
        pub const BORDER_W: usize = 280;
        pub const BORDER_H: usize = 180;
        pub const BORDER_THICKNESS: usize = 3;
    }

    /// Game overlay layout constants
    pub mod overlay {
        use super::*;

        /// Get sidebar region for overlay
        pub const fn sidebar() -> Region {
            Region::right_sidebar()
        }

        /// Standard padding inside sidebar
        pub const PADDING: usize = 4;

        /// Create cursor for overlay rendering
        pub const fn cursor() -> LayoutCursor {
            Region::right_sidebar().cursor(PADDING)
        }
    }
}
