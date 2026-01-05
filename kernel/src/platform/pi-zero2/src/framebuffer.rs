//! Framebuffer Graphics
//!
//! High-level drawing operations for the GPU framebuffer.
//! Supports both 32-bit ARGB and Game Boy palette rendering.

use crate::mailbox::{self, FramebufferInfo};

// ============================================================================
// Game Boy Display Constants
// ============================================================================

/// Game Boy native screen dimensions
pub const GB_WIDTH: usize = 160;
pub const GB_HEIGHT: usize = 144;

/// Default display dimensions for GPi Case 2W
pub const DISPLAY_WIDTH: usize = 640;
pub const DISPLAY_HEIGHT: usize = 480;

/// Bytes per pixel (32-bit ARGB)
pub const BYTES_PER_PIXEL: usize = 4;

// ============================================================================
// Color Definitions
// ============================================================================

/// 32-bit ARGB color type.
pub type Color = u32;

/// Common colors (ARGB format).
pub mod colors {
    use super::Color;

    pub const BLACK: Color = 0xFF00_0000;
    pub const WHITE: Color = 0xFFFF_FFFF;
    pub const RED: Color = 0xFFFF_0000;
    pub const GREEN: Color = 0xFF00_FF00;
    pub const BLUE: Color = 0xFF00_00FF;
    pub const YELLOW: Color = 0xFFFF_FF00;
    pub const CYAN: Color = 0xFF00_FFFF;
    pub const MAGENTA: Color = 0xFFFF_00FF;
    pub const GRAY: Color = 0xFF80_8080;
    pub const DARK_GRAY: Color = 0xFF40_4040;
    pub const LIGHT_GRAY: Color = 0xFFC0_C0C0;

    // Game Boy green palette (classic DMG)
    pub const GB_LIGHTEST: Color = 0xFF9B_BC0F;
    pub const GB_LIGHT: Color = 0xFF8B_AC0F;
    pub const GB_DARK: Color = 0xFF30_6230;
    pub const GB_DARKEST: Color = 0xFF0F_380F;
}

/// DMG (original Game Boy) palette - 4 shades of green.
pub const DMG_PALETTE: [Color; 4] = [
    colors::GB_LIGHTEST,
    colors::GB_LIGHT,
    colors::GB_DARK,
    colors::GB_DARKEST,
];

// ============================================================================
// Framebuffer Structure
// ============================================================================

/// Framebuffer for GPU rendering.
pub struct Framebuffer {
    /// Physical address of framebuffer memory.
    addr: usize,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bytes per row.
    pub pitch: u32,
    /// Bits per pixel.
    pub depth: u32,
    /// Total size in bytes.
    size: u32,
}

impl Framebuffer {
    /// Initialize framebuffer via VideoCore mailbox.
    ///
    /// # Arguments
    /// * `width` - Display width in pixels
    /// * `height` - Display height in pixels
    /// * `depth` - Bits per pixel (typically 32)
    pub fn new(width: u32, height: u32, depth: u32) -> Option<Self> {
        let info = mailbox::allocate_framebuffer(width, height, depth)?;

        Some(Self {
            addr: info.addr as usize,
            width: info.width,
            height: info.height,
            pitch: info.pitch,
            depth: info.depth,
            size: info.size,
        })
    }

    /// Create a framebuffer from pre-allocated info.
    pub fn from_info(info: FramebufferInfo) -> Self {
        Self {
            addr: info.addr as usize,
            width: info.width,
            height: info.height,
            pitch: info.pitch,
            depth: info.depth,
            size: info.size,
        }
    }

    /// Get framebuffer as a mutable byte slice.
    ///
    /// # Safety
    /// Caller must ensure exclusive access to framebuffer memory.
    pub unsafe fn as_slice(&mut self) -> &mut [u8] {
        core::slice::from_raw_parts_mut(self.addr as *mut u8, self.size as usize)
    }

    /// Get framebuffer as a mutable u32 slice (for 32-bit operations).
    pub unsafe fn as_u32_slice(&mut self) -> &mut [u32] {
        core::slice::from_raw_parts_mut(
            self.addr as *mut u32,
            (self.size / 4) as usize,
        )
    }

    /// Get raw pointer to framebuffer.
    #[inline(always)]
    pub fn ptr(&self) -> *mut u8 {
        self.addr as *mut u8
    }

    /// Calculate byte offset for a pixel.
    #[inline(always)]
    fn offset(&self, x: u32, y: u32) -> usize {
        (y * self.pitch + x * (self.depth / 8)) as usize
    }

    // ========================================================================
    // Basic Drawing Operations
    // ========================================================================

    /// Clear the entire framebuffer to a color.
    pub fn clear(&mut self, color: Color) {
        let pixels = (self.width * self.height) as usize;
        unsafe {
            let fb = self.as_u32_slice();
            for i in 0..pixels {
                fb[i] = color;
            }
        }
    }

    /// Set a single pixel.
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }

        unsafe {
            let ptr = (self.addr + self.offset(x, y)) as *mut u32;
            core::ptr::write_volatile(ptr, color);
        }
    }

    /// Get a pixel color.
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> Color {
        if x >= self.width || y >= self.height {
            return 0;
        }

        unsafe {
            let ptr = (self.addr + self.offset(x, y)) as *const u32;
            core::ptr::read_volatile(ptr)
        }
    }

    /// Draw a horizontal line.
    pub fn draw_hline(&mut self, x: u32, y: u32, width: u32, color: Color) {
        if y >= self.height {
            return;
        }

        let x_end = (x + width).min(self.width);
        for px in x..x_end {
            self.set_pixel(px, y, color);
        }
    }

    /// Draw a vertical line.
    pub fn draw_vline(&mut self, x: u32, y: u32, height: u32, color: Color) {
        if x >= self.width {
            return;
        }

        let y_end = (y + height).min(self.height);
        for py in y..y_end {
            self.set_pixel(x, py, color);
        }
    }

    /// Fill a rectangle.
    pub fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        let x_end = (x + width).min(self.width);
        let y_end = (y + height).min(self.height);

        for py in y..y_end {
            for px in x..x_end {
                self.set_pixel(px, py, color);
            }
        }
    }

    /// Draw a rectangle outline.
    pub fn draw_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        self.draw_hline(x, y, width, color);
        self.draw_hline(x, y + height - 1, width, color);
        self.draw_vline(x, y, height, color);
        self.draw_vline(x + width - 1, y, height, color);
    }

    // ========================================================================
    // Text Rendering (8x8 font)
    // ========================================================================

    /// Draw a single character using an 8x8 font.
    pub fn draw_char(&mut self, x: u32, y: u32, ch: char, fg: Color, bg: Color) {
        let glyph = get_glyph(ch);

        for row in 0..8 {
            let bits = glyph[row];
            for col in 0..8 {
                let color = if (bits >> (7 - col)) & 1 != 0 { fg } else { bg };
                self.set_pixel(x + col, y + row as u32, color);
            }
        }
    }

    /// Draw a string using an 8x8 font.
    pub fn draw_string(&mut self, x: u32, y: u32, s: &str, fg: Color, bg: Color) {
        let mut cx = x;
        let mut cy = y;

        for ch in s.chars() {
            match ch {
                '\n' => {
                    cx = x;
                    cy += 8;
                }
                '\r' => {
                    cx = x;
                }
                _ => {
                    if cx + 8 <= self.width {
                        self.draw_char(cx, cy, ch, fg, bg);
                    }
                    cx += 8;
                }
            }
        }
    }

    // ========================================================================
    // Game Boy Screen Rendering
    // ========================================================================

    /// Draw a border around the Game Boy screen area.
    pub fn draw_gb_border(&mut self, color: Color) {
        let scale = self.calculate_gb_scale();
        let (offset_x, offset_y) = self.calculate_gb_offset(scale);

        let border_width = (GB_WIDTH as u32) * scale;
        let border_height = (GB_HEIGHT as u32) * scale;
        let border_thickness = 4;

        // Top
        self.fill_rect(
            offset_x - border_thickness,
            offset_y - border_thickness,
            border_width + border_thickness * 2,
            border_thickness,
            color,
        );

        // Bottom
        self.fill_rect(
            offset_x - border_thickness,
            offset_y + border_height,
            border_width + border_thickness * 2,
            border_thickness,
            color,
        );

        // Left
        self.fill_rect(
            offset_x - border_thickness,
            offset_y,
            border_thickness,
            border_height,
            color,
        );

        // Right
        self.fill_rect(
            offset_x + border_width,
            offset_y,
            border_thickness,
            border_height,
            color,
        );
    }

    /// Calculate the best integer scale factor for Game Boy screen.
    fn calculate_gb_scale(&self) -> u32 {
        let scale_x = self.width / GB_WIDTH as u32;
        let scale_y = self.height / GB_HEIGHT as u32;
        scale_x.min(scale_y).max(1)
    }

    /// Calculate offset to center Game Boy screen.
    fn calculate_gb_offset(&self, scale: u32) -> (u32, u32) {
        let scaled_width = GB_WIDTH as u32 * scale;
        let scaled_height = GB_HEIGHT as u32 * scale;

        let offset_x = (self.width - scaled_width) / 2;
        let offset_y = (self.height - scaled_height) / 2;

        (offset_x, offset_y)
    }

    /// Blit Game Boy screen data using DMG palette.
    ///
    /// # Arguments
    /// * `data` - 160x144 palette indices (0-3)
    pub fn blit_gb_screen_dmg(&mut self, data: &[u8]) {
        let scale = self.calculate_gb_scale();
        let (offset_x, offset_y) = self.calculate_gb_offset(scale);

        for y in 0..GB_HEIGHT {
            for x in 0..GB_WIDTH {
                let idx = y * GB_WIDTH + x;
                let palette_idx = (data[idx] & 0x03) as usize;
                let color = DMG_PALETTE[palette_idx];

                // Scale the pixel
                let px = offset_x + (x as u32) * scale;
                let py = offset_y + (y as u32) * scale;

                self.fill_rect(px, py, scale, scale, color);
            }
        }
    }

    /// Blit Game Boy screen data using custom palette.
    ///
    /// # Arguments
    /// * `data` - 160x144 palette indices
    /// * `palette` - Color lookup table
    pub fn blit_gb_screen(&mut self, data: &[u8], palette: &[Color]) {
        let scale = self.calculate_gb_scale();
        let (offset_x, offset_y) = self.calculate_gb_offset(scale);

        for y in 0..GB_HEIGHT {
            for x in 0..GB_WIDTH {
                let idx = y * GB_WIDTH + x;
                let palette_idx = data[idx] as usize;
                let color = palette.get(palette_idx).copied().unwrap_or(colors::BLACK);

                let px = offset_x + (x as u32) * scale;
                let py = offset_y + (y as u32) * scale;

                self.fill_rect(px, py, scale, scale, color);
            }
        }
    }
}

// ============================================================================
// 8x8 Font Data
// ============================================================================

/// Get glyph data for a character (8 bytes, one per row).
fn get_glyph(ch: char) -> &'static [u8; 8] {
    let index = ch as usize;

    // Printable ASCII range (32-126)
    if index >= 32 && index < 127 {
        &FONT_8X8[index - 32]
    } else {
        &FONT_8X8[0] // Default to space for unknown characters
    }
}

/// Basic 8x8 bitmap font for printable ASCII characters (32-126).
static FONT_8X8: [[u8; 8]; 95] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // 32: ' '
    [0x18, 0x3C, 0x3C, 0x18, 0x18, 0x00, 0x18, 0x00], // 33: '!'
    [0x36, 0x36, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00], // 34: '"'
    [0x36, 0x36, 0x7F, 0x36, 0x7F, 0x36, 0x36, 0x00], // 35: '#'
    [0x0C, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x0C, 0x00], // 36: '$'
    [0x00, 0x63, 0x33, 0x18, 0x0C, 0x66, 0x63, 0x00], // 37: '%'
    [0x1C, 0x36, 0x1C, 0x6E, 0x3B, 0x33, 0x6E, 0x00], // 38: '&'
    [0x06, 0x06, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00], // 39: '''
    [0x18, 0x0C, 0x06, 0x06, 0x06, 0x0C, 0x18, 0x00], // 40: '('
    [0x06, 0x0C, 0x18, 0x18, 0x18, 0x0C, 0x06, 0x00], // 41: ')'
    [0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00], // 42: '*'
    [0x00, 0x0C, 0x0C, 0x3F, 0x0C, 0x0C, 0x00, 0x00], // 43: '+'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x06], // 44: ','
    [0x00, 0x00, 0x00, 0x3F, 0x00, 0x00, 0x00, 0x00], // 45: '-'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x00], // 46: '.'
    [0x60, 0x30, 0x18, 0x0C, 0x06, 0x03, 0x01, 0x00], // 47: '/'
    [0x3E, 0x63, 0x73, 0x7B, 0x6F, 0x67, 0x3E, 0x00], // 48: '0'
    [0x0C, 0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x3F, 0x00], // 49: '1'
    [0x1E, 0x33, 0x30, 0x1C, 0x06, 0x33, 0x3F, 0x00], // 50: '2'
    [0x1E, 0x33, 0x30, 0x1C, 0x30, 0x33, 0x1E, 0x00], // 51: '3'
    [0x38, 0x3C, 0x36, 0x33, 0x7F, 0x30, 0x78, 0x00], // 52: '4'
    [0x3F, 0x03, 0x1F, 0x30, 0x30, 0x33, 0x1E, 0x00], // 53: '5'
    [0x1C, 0x06, 0x03, 0x1F, 0x33, 0x33, 0x1E, 0x00], // 54: '6'
    [0x3F, 0x33, 0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x00], // 55: '7'
    [0x1E, 0x33, 0x33, 0x1E, 0x33, 0x33, 0x1E, 0x00], // 56: '8'
    [0x1E, 0x33, 0x33, 0x3E, 0x30, 0x18, 0x0E, 0x00], // 57: '9'
    [0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C, 0x00], // 58: ':'
    [0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C, 0x06], // 59: ';'
    [0x18, 0x0C, 0x06, 0x03, 0x06, 0x0C, 0x18, 0x00], // 60: '<'
    [0x00, 0x00, 0x3F, 0x00, 0x00, 0x3F, 0x00, 0x00], // 61: '='
    [0x06, 0x0C, 0x18, 0x30, 0x18, 0x0C, 0x06, 0x00], // 62: '>'
    [0x1E, 0x33, 0x30, 0x18, 0x0C, 0x00, 0x0C, 0x00], // 63: '?'
    [0x3E, 0x63, 0x7B, 0x7B, 0x7B, 0x03, 0x1E, 0x00], // 64: '@'
    [0x0C, 0x1E, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x00], // 65: 'A'
    [0x3F, 0x66, 0x66, 0x3E, 0x66, 0x66, 0x3F, 0x00], // 66: 'B'
    [0x3C, 0x66, 0x03, 0x03, 0x03, 0x66, 0x3C, 0x00], // 67: 'C'
    [0x1F, 0x36, 0x66, 0x66, 0x66, 0x36, 0x1F, 0x00], // 68: 'D'
    [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x46, 0x7F, 0x00], // 69: 'E'
    [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x06, 0x0F, 0x00], // 70: 'F'
    [0x3C, 0x66, 0x03, 0x03, 0x73, 0x66, 0x7C, 0x00], // 71: 'G'
    [0x33, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x33, 0x00], // 72: 'H'
    [0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00], // 73: 'I'
    [0x78, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E, 0x00], // 74: 'J'
    [0x67, 0x66, 0x36, 0x1E, 0x36, 0x66, 0x67, 0x00], // 75: 'K'
    [0x0F, 0x06, 0x06, 0x06, 0x46, 0x66, 0x7F, 0x00], // 76: 'L'
    [0x63, 0x77, 0x7F, 0x7F, 0x6B, 0x63, 0x63, 0x00], // 77: 'M'
    [0x63, 0x67, 0x6F, 0x7B, 0x73, 0x63, 0x63, 0x00], // 78: 'N'
    [0x1C, 0x36, 0x63, 0x63, 0x63, 0x36, 0x1C, 0x00], // 79: 'O'
    [0x3F, 0x66, 0x66, 0x3E, 0x06, 0x06, 0x0F, 0x00], // 80: 'P'
    [0x1E, 0x33, 0x33, 0x33, 0x3B, 0x1E, 0x38, 0x00], // 81: 'Q'
    [0x3F, 0x66, 0x66, 0x3E, 0x36, 0x66, 0x67, 0x00], // 82: 'R'
    [0x1E, 0x33, 0x07, 0x0E, 0x38, 0x33, 0x1E, 0x00], // 83: 'S'
    [0x3F, 0x2D, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00], // 84: 'T'
    [0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x3F, 0x00], // 85: 'U'
    [0x33, 0x33, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00], // 86: 'V'
    [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00], // 87: 'W'
    [0x63, 0x63, 0x36, 0x1C, 0x1C, 0x36, 0x63, 0x00], // 88: 'X'
    [0x33, 0x33, 0x33, 0x1E, 0x0C, 0x0C, 0x1E, 0x00], // 89: 'Y'
    [0x7F, 0x63, 0x31, 0x18, 0x4C, 0x66, 0x7F, 0x00], // 90: 'Z'
    [0x1E, 0x06, 0x06, 0x06, 0x06, 0x06, 0x1E, 0x00], // 91: '['
    [0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x40, 0x00], // 92: '\'
    [0x1E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x1E, 0x00], // 93: ']'
    [0x08, 0x1C, 0x36, 0x63, 0x00, 0x00, 0x00, 0x00], // 94: '^'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF], // 95: '_'
    [0x0C, 0x0C, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00], // 96: '`'
    [0x00, 0x00, 0x1E, 0x30, 0x3E, 0x33, 0x6E, 0x00], // 97: 'a'
    [0x07, 0x06, 0x06, 0x3E, 0x66, 0x66, 0x3B, 0x00], // 98: 'b'
    [0x00, 0x00, 0x1E, 0x33, 0x03, 0x33, 0x1E, 0x00], // 99: 'c'
    [0x38, 0x30, 0x30, 0x3E, 0x33, 0x33, 0x6E, 0x00], // 100: 'd'
    [0x00, 0x00, 0x1E, 0x33, 0x3F, 0x03, 0x1E, 0x00], // 101: 'e'
    [0x1C, 0x36, 0x06, 0x0F, 0x06, 0x06, 0x0F, 0x00], // 102: 'f'
    [0x00, 0x00, 0x6E, 0x33, 0x33, 0x3E, 0x30, 0x1F], // 103: 'g'
    [0x07, 0x06, 0x36, 0x6E, 0x66, 0x66, 0x67, 0x00], // 104: 'h'
    [0x0C, 0x00, 0x0E, 0x0C, 0x0C, 0x0C, 0x1E, 0x00], // 105: 'i'
    [0x30, 0x00, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E], // 106: 'j'
    [0x07, 0x06, 0x66, 0x36, 0x1E, 0x36, 0x67, 0x00], // 107: 'k'
    [0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00], // 108: 'l'
    [0x00, 0x00, 0x33, 0x7F, 0x7F, 0x6B, 0x63, 0x00], // 109: 'm'
    [0x00, 0x00, 0x1F, 0x33, 0x33, 0x33, 0x33, 0x00], // 110: 'n'
    [0x00, 0x00, 0x1E, 0x33, 0x33, 0x33, 0x1E, 0x00], // 111: 'o'
    [0x00, 0x00, 0x3B, 0x66, 0x66, 0x3E, 0x06, 0x0F], // 112: 'p'
    [0x00, 0x00, 0x6E, 0x33, 0x33, 0x3E, 0x30, 0x78], // 113: 'q'
    [0x00, 0x00, 0x3B, 0x6E, 0x66, 0x06, 0x0F, 0x00], // 114: 'r'
    [0x00, 0x00, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x00], // 115: 's'
    [0x08, 0x0C, 0x3E, 0x0C, 0x0C, 0x2C, 0x18, 0x00], // 116: 't'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x33, 0x6E, 0x00], // 117: 'u'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00], // 118: 'v'
    [0x00, 0x00, 0x63, 0x6B, 0x7F, 0x7F, 0x36, 0x00], // 119: 'w'
    [0x00, 0x00, 0x63, 0x36, 0x1C, 0x36, 0x63, 0x00], // 120: 'x'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x3E, 0x30, 0x1F], // 121: 'y'
    [0x00, 0x00, 0x3F, 0x19, 0x0C, 0x26, 0x3F, 0x00], // 122: 'z'
    [0x38, 0x0C, 0x0C, 0x07, 0x0C, 0x0C, 0x38, 0x00], // 123: '{'
    [0x18, 0x18, 0x18, 0x00, 0x18, 0x18, 0x18, 0x00], // 124: '|'
    [0x07, 0x0C, 0x0C, 0x38, 0x0C, 0x0C, 0x07, 0x00], // 125: '}'
    [0x6E, 0x3B, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // 126: '~'
];
