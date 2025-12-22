//! 8x8 Bitmap Font
//!
//! A simple monospace bitmap font for VGA text rendering.
//! Each character is 8x8 pixels, stored as 8 bytes (one per row).
//! MSB is the leftmost pixel in each row.

use crate::graphics::vga_mode13h::{self, SCREEN_WIDTH};

// Re-export colors for compatibility with existing code
pub use crate::graphics::vga_mode13h::colors;

// ============================================================================
// Font Data
// ============================================================================

/// Number of characters in the font
pub const CHAR_COUNT: usize = 45;

/// Character width in pixels
pub const CHAR_WIDTH: usize = 8;

/// Character height in pixels
pub const CHAR_HEIGHT: usize = 8;

/// Font character index mapping:
/// - 0-25:  A-Z (uppercase letters)
/// - 26-35: 0-9 (digits)
/// - 36:    Space
/// - 37:    . (period)
/// - 38:    : (colon)
/// - 39:    / (forward slash)
/// - 40:    - (hyphen)
/// - 41:    > (greater than / arrow)
/// - 42:    ^ (caret / up arrow)
/// - 43:    v (down arrow, displayed as V)
/// - 44:    _ (underscore)
#[rustfmt::skip]
pub static FONT_DATA: [u8; CHAR_COUNT * CHAR_HEIGHT] = [
    // A (0)
    0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x00,
    // B (1)
    0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00,
    // C (2)
    0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00,
    // D (3)
    0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00,
    // E (4)
    0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x7E, 0x00,
    // F (5)
    0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x60, 0x00,
    // G (6)
    0x3C, 0x66, 0x60, 0x6E, 0x66, 0x66, 0x3C, 0x00,
    // H (7)
    0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00,
    // I (8)
    0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00,
    // J (9)
    0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38, 0x00,
    // K (10)
    0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C, 0x66, 0x00,
    // L (11)
    0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7E, 0x00,
    // M (12)
    0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00,
    // N (13)
    0x66, 0x76, 0x7E, 0x7E, 0x6E, 0x66, 0x66, 0x00,
    // O (14)
    0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00,
    // P (15)
    0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00,
    // Q (16)
    0x3C, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x0E, 0x00,
    // R (17)
    0x7C, 0x66, 0x66, 0x7C, 0x78, 0x6C, 0x66, 0x00,
    // S (18)
    0x3C, 0x66, 0x60, 0x3C, 0x06, 0x66, 0x3C, 0x00,
    // T (19)
    0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00,
    // U (20)
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00,
    // V (21)
    0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00,
    // W (22)
    0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00,
    // X (23)
    0x66, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x66, 0x00,
    // Y (24)
    0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00,
    // Z (25)
    0x7E, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x7E, 0x00,
    // 0 (26)
    0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00,
    // 1 (27)
    0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00,
    // 2 (28)
    0x3C, 0x66, 0x06, 0x0C, 0x30, 0x60, 0x7E, 0x00,
    // 3 (29)
    0x3C, 0x66, 0x06, 0x1C, 0x06, 0x66, 0x3C, 0x00,
    // 4 (30)
    0x06, 0x0E, 0x1E, 0x66, 0x7F, 0x06, 0x06, 0x00,
    // 5 (31)
    0x7E, 0x60, 0x7C, 0x06, 0x06, 0x66, 0x3C, 0x00,
    // 6 (32)
    0x3C, 0x66, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00,
    // 7 (33)
    0x7E, 0x66, 0x0C, 0x18, 0x18, 0x18, 0x18, 0x00,
    // 8 (34)
    0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00,
    // 9 (35)
    0x3C, 0x66, 0x66, 0x3E, 0x06, 0x66, 0x3C, 0x00,
    // Space (36)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // . period (37)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00,
    // : colon (38)
    0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00,
    // / slash (39)
    0x02, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x40, 0x00,
    // - hyphen (40)
    0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00,
    // > greater than (41)
    0x30, 0x18, 0x0C, 0x06, 0x0C, 0x18, 0x30, 0x00,
    // ^ caret/up arrow (42)
    0x18, 0x3C, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
    // v down arrow (43) - displayed as V shape
    0x00, 0x00, 0x00, 0x00, 0x66, 0x3C, 0x18, 0x00,
    // _ underscore (44)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7E, 0x00,
];

// ============================================================================
// Character Index Mapping
// ============================================================================

/// Down arrow character (special case - use this instead of 'v' for arrow)
pub const DOWN_ARROW: u8 = 0x19; // Using a control char that won't conflict

/// Map an ASCII character to its font index
///
/// # Arguments
/// * `ch` - ASCII character byte
///
/// # Returns
/// Font index (0-44), or 36 (space) for unknown characters
#[inline(always)]
pub fn char_to_index(ch: u8) -> usize {
    match ch {
        // Handle special characters first (before letter ranges)
        b' ' => 36,
        b'.' => 37,
        b':' => 38,
        b'/' => 39,
        b'-' => 40,
        b'>' => 41,
        b'^' => 42,
        DOWN_ARROW => 43, // Down arrow (use DOWN_ARROW constant)
        b'_' => 44,
        // Letters (lowercase maps to uppercase)
        b'A'..=b'Z' => (ch - b'A') as usize,
        b'a'..=b'z' => (ch - b'a') as usize,
        // Digits
        b'0'..=b'9' => (ch - b'0' + 26) as usize,
        _ => 36, // Default to space for unknown characters
    }
}

/// Get the bitmap data for a character
///
/// # Arguments
/// * `ch` - ASCII character byte
///
/// # Returns
/// Slice of 8 bytes representing the character bitmap
#[inline(always)]
pub fn get_char_bitmap(ch: u8) -> &'static [u8] {
    let index = char_to_index(ch);
    let start = index * CHAR_HEIGHT;
    // Safety: char_to_index always returns 0-44, so start is 0-352
    // and start + CHAR_HEIGHT is 8-360, which is within FONT_DATA bounds (360 bytes)
    unsafe { FONT_DATA.get_unchecked(start..start + CHAR_HEIGHT) }
}

// ============================================================================
// Buffer-Based Rendering (for mode13h.rs and offscreen buffers)
// ============================================================================

/// Draw a character bitmap to a buffer
#[inline]
fn draw_bitmap_to_buffer(
    buffer: &mut [u8],
    x: usize,
    y: usize,
    bitmap: &[u8],
    color: u8,
) {
    for (row, &bits) in bitmap.iter().enumerate().take(CHAR_HEIGHT) {
        let py = y + row;
        for col in 0..CHAR_WIDTH {
            if (bits >> (7 - col)) & 1 != 0 {
                let px = x + col;
                let offset = py * SCREEN_WIDTH + px;
                if offset < buffer.len() {
                    buffer[offset] = color;
                }
            }
        }
    }
}

/// Draw a character bitmap to a buffer with background color
#[inline]
fn draw_bitmap_to_buffer_bg(
    buffer: &mut [u8],
    x: usize,
    y: usize,
    bitmap: &[u8],
    fg: u8,
    bg: u8,
) {
    for (row, &bits) in bitmap.iter().enumerate().take(CHAR_HEIGHT) {
        let py = y + row;
        for col in 0..CHAR_WIDTH {
            let px = x + col;
            let offset = py * SCREEN_WIDTH + px;
            if offset < buffer.len() {
                buffer[offset] = if (bits >> (7 - col)) & 1 != 0 { fg } else { bg };
            }
        }
    }
}

/// Draw a single character to a buffer (foreground only)
///
/// This is the primary buffer-based draw_char used by mode13h.rs
pub fn draw_char(buffer: &mut [u8], x: usize, y: usize, ch: u8, fg: u8) {
    let bitmap = get_char_bitmap(ch);
    draw_bitmap_to_buffer(buffer, x, y, bitmap, fg);
}

/// Draw a single character to a buffer with background color
pub fn draw_char_bg(buffer: &mut [u8], x: usize, y: usize, ch: u8, fg: u8, bg: u8) {
    let bitmap = get_char_bitmap(ch);
    draw_bitmap_to_buffer_bg(buffer, x, y, bitmap, fg, bg);
}

/// Draw a string to a buffer (foreground only)
pub fn draw_str(buffer: &mut [u8], x: usize, y: usize, s: &str, fg: u8) {
    let mut cx = x;
    for ch in s.bytes() {
        draw_char(buffer, cx, y, ch, fg);
        cx += CHAR_WIDTH;
    }
}

/// Draw a &str string to a buffer (foreground only) - alias for draw_str
pub fn draw_string(buffer: &mut [u8], x: usize, y: usize, s: &str, fg: u8) {
    draw_str(buffer, x, y, s, fg);
}

/// Draw a string to a buffer with background color
pub fn draw_str_bg(buffer: &mut [u8], x: usize, y: usize, s: &str, fg: u8, bg: u8) {
    let mut cx = x;
    for ch in s.bytes() {
        draw_char_bg(buffer, cx, y, ch, fg, bg);
        cx += CHAR_WIDTH;
    }
}

/// Draw a string centered horizontally to a buffer
pub fn draw_string_centered(buffer: &mut [u8], y: usize, s: &str, fg: u8) {
    let width = s.len() * CHAR_WIDTH;
    let x = (SCREEN_WIDTH.saturating_sub(width)) / 2;
    draw_str(buffer, x, y, s, fg);
}

/// Draw an unsigned number to a buffer
pub fn draw_number(buffer: &mut [u8], x: usize, y: usize, n: u32, min_width: usize, fg: u8) {
    let mut digits = [0u8; 10];
    let mut num = n;
    let mut count = 0;

    // Extract digits (reverse order)
    loop {
        digits[count] = b'0' + (num % 10) as u8;
        count += 1;
        num /= 10;
        if num == 0 {
            break;
        }
    }

    // Pad with spaces if needed
    let mut cx = x;
    if count < min_width {
        for _ in 0..(min_width - count) {
            draw_char(buffer, cx, y, b' ', fg);
            cx += CHAR_WIDTH;
        }
    }

    // Draw digits in correct order
    for i in (0..count).rev() {
        draw_char(buffer, cx, y, digits[i], fg);
        cx += CHAR_WIDTH;
    }
}

/// Draw a signed number to a buffer
pub fn draw_signed(buffer: &mut [u8], x: usize, y: usize, n: i32, min_width: usize, fg: u8) {
    let mut cx = x;
    if n < 0 {
        draw_char(buffer, cx, y, b'-', fg);
        cx += CHAR_WIDTH;
        draw_number(buffer, cx, y, (-n) as u32, min_width.saturating_sub(1), fg);
    } else {
        draw_number(buffer, cx, y, n as u32, min_width, fg);
    }
}

/// Draw a hexadecimal number to a buffer
pub fn draw_hex(buffer: &mut [u8], x: usize, y: usize, n: u32, digits: usize, fg: u8) {
    const HEX_CHARS: &[u8] = b"0123456789ABCDEF";
    let mut cx = x;
    for i in (0..digits).rev() {
        let nibble = ((n >> (i * 4)) & 0xF) as usize;
        draw_char(buffer, cx, y, HEX_CHARS[nibble], fg);
        cx += CHAR_WIDTH;
    }
}

/// Calculate the pixel width of a string
#[inline]
pub fn string_width(s: &str) -> usize {
    s.len() * CHAR_WIDTH
}

// ============================================================================
// VGA-Direct Rendering (for rom_browser.rs - writes directly to VGA memory)
// ============================================================================

/// Draw a single character directly to VGA memory
#[inline(always)]
pub fn draw_char_vga(x: usize, y: usize, ch: u8, color: u8) {
    let bitmap = get_char_bitmap(ch);
    vga_mode13h::draw_bitmap_8x8(x, y, bitmap, color);
}

/// Draw a string directly to VGA memory
#[inline(always)]
pub fn draw_string_vga(x: usize, y: usize, s: &str, color: u8) {
    let mut cx = x;
    for ch in s.bytes() {
        draw_char_vga(cx, y, ch, color);
        cx += CHAR_WIDTH;
    }
}

/// Draw a string centered horizontally, directly to VGA memory
#[inline(always)]
pub fn draw_string_centered_vga(y: usize, s: &str, color: u8) {
    let width = s.len() * CHAR_WIDTH;
    let x = (SCREEN_WIDTH.saturating_sub(width)) / 2;
    draw_string_vga(x, y, s, color);
}
