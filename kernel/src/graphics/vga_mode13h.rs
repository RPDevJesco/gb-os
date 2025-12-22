//! VGA Mode 13h Graphics Driver
//!
//! Provides low-level drawing primitives for VGA Mode 13h (320x200, 256 colors).
//! The framebuffer is memory-mapped at 0xA0000.

// ============================================================================
// Constants
// ============================================================================

/// Screen dimensions for VGA Mode 13h
pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 200;

/// VGA framebuffer base address
pub const VGA_ADDR: *mut u8 = 0xA0000 as *mut u8;

/// Standard VGA palette color indices
pub mod colors {
    pub const BLACK: u8 = 0x00;
    pub const DARK_GREEN: u8 = 0x02;
    pub const RED: u8 = 0x04;
    pub const LIGHT_GRAY: u8 = 0x07;
    pub const DARK_GRAY: u8 = 0x08;
    pub const GREEN: u8 = 0x0A;
    pub const WHITE: u8 = 0x0F;
    pub const LIGHT_GREEN: u8 = 0x2A; // GB screen green
    pub const HIGHLIGHT_BG: u8 = 0x02; // Dark green background for selection
}

// ============================================================================
// Drawing Primitives
// ============================================================================

/// Fill the entire screen with a single color
///
/// # Arguments
/// * `color` - VGA palette index (0-255)
#[inline(never)]
pub fn fill_screen(color: u8) {
    unsafe {
        for i in 0..(SCREEN_WIDTH * SCREEN_HEIGHT) {
            core::ptr::write_volatile(VGA_ADDR.add(i), color);
        }
    }
}

/// Fill a portion of the screen, preserving debug rows at bottom
///
/// # Arguments
/// * `color` - VGA palette index (0-255)
/// * `preserve_rows` - Number of rows at bottom to preserve (e.g., 5 for debug)
#[inline(never)]
pub fn fill_screen_partial(color: u8, preserve_rows: usize) {
    let rows_to_fill = SCREEN_HEIGHT.saturating_sub(preserve_rows);
    unsafe {
        for i in 0..(SCREEN_WIDTH * rows_to_fill) {
            core::ptr::write_volatile(VGA_ADDR.add(i), color);
        }
    }
}

/// Fill a rectangular region with a color
///
/// # Arguments
/// * `x` - Left edge X coordinate
/// * `y` - Top edge Y coordinate
/// * `w` - Width in pixels
/// * `h` - Height in pixels
/// * `color` - VGA palette index (0-255)
#[inline(never)]
pub fn fill_rect(x: usize, y: usize, w: usize, h: usize, color: u8) {
    unsafe {
        for row in 0..h {
            let py = y + row;
            if py >= SCREEN_HEIGHT {
                break;
            }
            for col in 0..w {
                let px = x + col;
                if px >= SCREEN_WIDTH {
                    break;
                }
                let offset = py * SCREEN_WIDTH + px;
                core::ptr::write_volatile(VGA_ADDR.add(offset), color);
            }
        }
    }
}

/// Set a single pixel
///
/// # Arguments
/// * `x` - X coordinate
/// * `y` - Y coordinate
/// * `color` - VGA palette index (0-255)
#[inline(never)]
pub fn set_pixel(x: usize, y: usize, color: u8) {
    if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
        unsafe {
            let offset = y * SCREEN_WIDTH + x;
            core::ptr::write_volatile(VGA_ADDR.add(offset), color);
        }
    }
}

/// Draw a horizontal line
///
/// # Arguments
/// * `x` - Starting X coordinate
/// * `y` - Y coordinate
/// * `length` - Length in pixels
/// * `color` - VGA palette index (0-255)
#[inline(never)]
pub fn draw_hline(x: usize, y: usize, length: usize, color: u8) {
    fill_rect(x, y, length, 1, color);
}

/// Draw a vertical line
///
/// # Arguments
/// * `x` - X coordinate
/// * `y` - Starting Y coordinate
/// * `length` - Length in pixels
/// * `color` - VGA palette index (0-255)
#[inline(never)]
pub fn draw_vline(x: usize, y: usize, length: usize, color: u8) {
    fill_rect(x, y, 1, length, color);
}

/// Draw a rectangle outline (not filled)
///
/// # Arguments
/// * `x` - Left edge X coordinate
/// * `y` - Top edge Y coordinate
/// * `w` - Width in pixels
/// * `h` - Height in pixels
/// * `color` - VGA palette index (0-255)
#[inline(never)]
pub fn draw_rect(x: usize, y: usize, w: usize, h: usize, color: u8) {
    draw_hline(x, y, w, color); // Top
    draw_hline(x, y + h - 1, w, color); // Bottom
    draw_vline(x, y, h, color); // Left
    draw_vline(x + w - 1, y, h, color); // Right
}

/// Draw a thick border (multiple pixel width)
///
/// # Arguments
/// * `x` - Left edge X coordinate
/// * `y` - Top edge Y coordinate
/// * `w` - Width in pixels
/// * `h` - Height in pixels
/// * `thickness` - Border thickness in pixels
/// * `color` - VGA palette index (0-255)
#[inline(never)]
pub fn draw_thick_border(x: usize, y: usize, w: usize, h: usize, thickness: usize, color: u8) {
    // Top
    fill_rect(x, y, w, thickness, color);
    // Bottom
    fill_rect(x, y + h - thickness, w, thickness, color);
    // Left
    fill_rect(x, y, thickness, h, color);
    // Right
    fill_rect(x + w - thickness, y, thickness, h, color);
}

/// Draw an 8x8 bitmap (for font rendering, sprites, etc.)
///
/// Takes bitmap by value as [u8; 8] to avoid slice pointer issues
/// in bare-metal environments.
///
/// # Arguments
/// * `x` - Left edge X coordinate
/// * `y` - Top edge Y coordinate
/// * `bitmap` - Array of 8 bytes, one per row, MSB is leftmost pixel
/// * `color` - VGA palette index for set pixels (0-255)
#[inline(never)]
pub fn draw_bitmap_8x8(x: usize, y: usize, bitmap: [u8; 8], color: u8) {
    unsafe {
        for (row, bits) in bitmap.iter().enumerate() {
            let py = y + row;
            if py >= SCREEN_HEIGHT {
                continue;
            }
            for col in 0..8 {
                if (bits >> (7 - col)) & 1 != 0 {
                    let px = x + col;
                    if px >= SCREEN_WIDTH {
                        continue;
                    }
                    let offset = py * SCREEN_WIDTH + px;
                    core::ptr::write_volatile(VGA_ADDR.add(offset), color);
                }
            }
        }
    }
}
