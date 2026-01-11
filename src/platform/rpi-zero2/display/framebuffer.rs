//! Framebuffer management and display rendering
//!
//! This module handles:
//! - Double/triple buffered framebuffer allocation via VideoCore mailbox
//! - Basic drawing primitives (pixel, rect, line, circle, clear)
//! - Text rendering with 8x8 and 8x16 bitmap fonts
//! - Alpha blending and compositing
//! - Clipping rectangle stack
//! - Dirty region tracking for partial updates
//! - Layer-based compositing system
//! - Bitmap/sprite blitting
//! - GameBoy screen blitting with 2x scaling
//! - Vsync and frame timing
//! - Color constants (ARGB8888 format)

use core::ptr::{write_volatile, read_volatile};
use crate::hal::mailbox::{mailbox_call, MailboxBuffer, tags};
use crate::platform_core::mmio::{dsb, clean_dcache_range};

// ============================================================================
// Display Constants
// ============================================================================

/// Default screen width (640x480 for GPi Case 2W)
pub const SCREEN_WIDTH: u32 = 640;

/// Default screen height
pub const SCREEN_HEIGHT: u32 = 480;

/// Bits per pixel (32-bit ARGB)
pub const BITS_PER_PIXEL: u32 = 32;

/// Font character width (8x8 font)
pub const CHAR_WIDTH: u32 = 8;

/// Font character height (8x8 font)
pub const CHAR_HEIGHT: u32 = 8;

/// Large font character width (8x16 font)
pub const CHAR_WIDTH_LARGE: u32 = 8;

/// Large font character height (8x16 font)
pub const CHAR_HEIGHT_LARGE: u32 = 16;

// ============================================================================
// Configuration Constants
// ============================================================================

/// Maximum number of dirty rectangles to track
const MAX_DIRTY_RECTS: usize = 32;

/// Maximum clip stack depth
const MAX_CLIP_DEPTH: usize = 8;

/// Maximum number of compositing layers
const MAX_LAYERS: usize = 4;

/// Number of buffers (2 = double, 3 = triple)
const BUFFER_COUNT: usize = 1;

// ============================================================================
// GameBoy Display Constants
// ============================================================================

/// GameBoy native width
pub const GB_WIDTH: usize = 160;

/// GameBoy native height
pub const GB_HEIGHT: usize = 144;

/// Display scale factor
pub const GB_SCALE: usize = 2;

/// Scaled GameBoy width
pub const GB_SCALED_W: usize = GB_WIDTH * GB_SCALE;

/// Scaled GameBoy height
pub const GB_SCALED_H: usize = GB_HEIGHT * GB_SCALE;

/// X offset to center GameBoy screen
pub const GB_OFFSET_X: usize = (SCREEN_WIDTH as usize - GB_SCALED_W) / 2;

/// Y offset to center GameBoy screen
pub const GB_OFFSET_Y: usize = (SCREEN_HEIGHT as usize - GB_SCALED_H) / 2;

// ============================================================================
// Colors (ARGB8888 format)
// ============================================================================

/// Color constants in ARGB8888 format (alpha in high byte)
pub mod color {
    pub const BLACK: u32 = 0xFF00_0000;
    pub const WHITE: u32 = 0xFFFF_FFFF;
    pub const RED: u32 = 0xFFFF_0000;
    pub const GREEN: u32 = 0xFF00_FF00;
    pub const BLUE: u32 = 0xFF00_00FF;
    pub const CYAN: u32 = 0xFF00_FFFF;
    pub const MAGENTA: u32 = 0xFFFF_00FF;
    pub const YELLOW: u32 = 0xFFFF_FF00;
    pub const GRAY: u32 = 0xFF80_8080;
    pub const DARK_GRAY: u32 = 0xFF40_4040;
    pub const LIGHT_GRAY: u32 = 0xFFC0_C0C0;
    pub const DARK_BLUE: u32 = 0xFF00_0040;
    pub const ORANGE: u32 = 0xFFFF_8000;
    pub const TRANSPARENT: u32 = 0x0000_0000;

    // Menu-specific colors
    pub const MENU_BG: u32 = 0xFF10_1020;
    pub const MENU_HIGHLIGHT: u32 = 0xFF30_3060;
    pub const MENU_TEXT: u32 = 0xFFE0_E0E0;
    pub const MENU_TEXT_DIM: u32 = 0xFF80_8080;
    pub const MENU_ACCENT: u32 = 0xFF40_80FF;

    // Semi-transparent variants
    pub const BLACK_50: u32 = 0x8000_0000;
    pub const BLACK_75: u32 = 0xC000_0000;
    pub const WHITE_50: u32 = 0x80FF_FFFF;
    pub const WHITE_25: u32 = 0x40FF_FFFF;

    /// Create a color with specified alpha (0-255)
    #[inline]
    pub const fn with_alpha(rgb: u32, alpha: u8) -> u32 {
        (rgb & 0x00FF_FFFF) | ((alpha as u32) << 24)
    }

    /// Extract alpha component (0-255)
    #[inline]
    pub const fn alpha(color: u32) -> u8 {
        (color >> 24) as u8
    }

    /// Extract red component (0-255)
    #[inline]
    pub const fn red(color: u32) -> u8 {
        ((color >> 16) & 0xFF) as u8
    }

    /// Extract green component (0-255)
    #[inline]
    pub const fn green(color: u32) -> u8 {
        ((color >> 8) & 0xFF) as u8
    }

    /// Extract blue component (0-255)
    #[inline]
    pub const fn blue(color: u32) -> u8 {
        (color & 0xFF) as u8
    }

    /// Create ARGB color from components
    #[inline]
    pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
        ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    /// Create opaque RGB color
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
        argb(255, r, g, b)
    }

    /// Linearly interpolate between two colors
    #[inline]
    pub fn lerp(c1: u32, c2: u32, t: u8) -> u32 {
        let t = t as u32;
        let inv_t = 255 - t;

        let a = ((alpha(c1) as u32 * inv_t + alpha(c2) as u32 * t) / 255) as u8;
        let r = ((red(c1) as u32 * inv_t + red(c2) as u32 * t) / 255) as u8;
        let g = ((green(c1) as u32 * inv_t + green(c2) as u32 * t) / 255) as u8;
        let b = ((blue(c1) as u32 * inv_t + blue(c2) as u32 * t) / 255) as u8;

        argb(a, r, g, b)
    }
}

/// GameBoy DMG palette (classic green-ish)
pub const GB_PALETTE: [u32; 4] = [
    0xFFE0_F8D0, // Lightest (white-ish green)
    0xFF88_C070, // Light green
    0xFF34_6856, // Dark green
    0xFF08_1820, // Darkest (near black)
];

// ============================================================================
// Integer Math Helpers (for no_std compatibility)
// ============================================================================

/// Integer square root using Newton's method
#[inline]
pub fn isqrt(n: u32) -> u32 {
    if n == 0 {
        return 0;
    }

    let mut x = n;
    let mut y = (x + 1) / 2;

    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }

    x
}

/// Sine lookup table (256 = 1.0, values for 0-90 degrees)
/// Indexed by degree, returns sin * 256
static SIN_TABLE: [i16; 91] = [
    0, 4, 9, 13, 18, 22, 27, 31, 36, 40,
    44, 49, 53, 58, 62, 66, 71, 75, 79, 83,
    88, 92, 96, 100, 104, 108, 112, 116, 120, 124,
    128, 131, 135, 139, 143, 146, 150, 153, 157, 160,
    164, 167, 171, 174, 177, 181, 184, 187, 190, 193,
    196, 198, 201, 204, 207, 209, 212, 214, 217, 219,
    221, 223, 226, 228, 230, 232, 233, 235, 237, 238,
    240, 242, 243, 244, 246, 247, 248, 249, 250, 251,
    252, 252, 253, 254, 254, 255, 255, 255, 256, 256,
    256,
];

/// Get sine and cosine for an angle in degrees (returns values scaled by 256)
#[inline]
pub fn sin_cos_deg(deg: u32) -> (i32, i32) {
    let deg = deg % 360;

    let (sin_sign, cos_sign, lookup_deg) = match deg {
        0..=90 => (1i32, 1i32, deg),
        91..=180 => (1, -1, 180 - deg),
        181..=270 => (-1, -1, deg - 180),
        _ => (-1, 1, 360 - deg),
    };

    let sin_val = SIN_TABLE[lookup_deg as usize] as i32 * sin_sign;
    let cos_val = SIN_TABLE[(90 - lookup_deg) as usize] as i32 * cos_sign;

    (sin_val, cos_val)
}

// ============================================================================
// 8x8 Bitmap Font
// ============================================================================

/// 8x8 bitmap font for ASCII characters 32-126 (95 characters)
/// Each character is 8 bytes, one byte per row, MSB is leftmost pixel
static FONT_8X8: [u8; 95 * 8] = [
    // Space (32)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // ! (33)
    0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x00,
    // " (34)
    0x6C, 0x6C, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00,
    // # (35)
    0x6C, 0x6C, 0xFE, 0x6C, 0xFE, 0x6C, 0x6C, 0x00,
    // $ (36)
    0x18, 0x7E, 0xC0, 0x7C, 0x06, 0xFC, 0x18, 0x00,
    // % (37)
    0x00, 0xC6, 0xCC, 0x18, 0x30, 0x66, 0xC6, 0x00,
    // & (38)
    0x38, 0x6C, 0x38, 0x76, 0xDC, 0xCC, 0x76, 0x00,
    // ' (39)
    0x18, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00,
    // ( (40)
    0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00,
    // ) (41)
    0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00,
    // * (42)
    0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00,
    // + (43)
    0x00, 0x18, 0x18, 0x7E, 0x18, 0x18, 0x00, 0x00,
    // , (44)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30,
    // - (45)
    0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00,
    // . (46)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00,
    // / (47)
    0x06, 0x0C, 0x18, 0x30, 0x60, 0xC0, 0x80, 0x00,
    // 0 (48)
    0x7C, 0xCE, 0xDE, 0xF6, 0xE6, 0xC6, 0x7C, 0x00,
    // 1 (49)
    0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00,
    // 2 (50)
    0x7C, 0xC6, 0x06, 0x7C, 0xC0, 0xC0, 0xFE, 0x00,
    // 3 (51)
    0xFC, 0x06, 0x06, 0x3C, 0x06, 0x06, 0xFC, 0x00,
    // 4 (52)
    0x0C, 0xCC, 0xCC, 0xCC, 0xFE, 0x0C, 0x0C, 0x00,
    // 5 (53)
    0xFE, 0xC0, 0xFC, 0x06, 0x06, 0xC6, 0x7C, 0x00,
    // 6 (54)
    0x7C, 0xC0, 0xC0, 0xFC, 0xC6, 0xC6, 0x7C, 0x00,
    // 7 (55)
    0xFE, 0x06, 0x06, 0x0C, 0x18, 0x18, 0x18, 0x00,
    // 8 (56)
    0x7C, 0xC6, 0xC6, 0x7C, 0xC6, 0xC6, 0x7C, 0x00,
    // 9 (57)
    0x7C, 0xC6, 0xC6, 0x7E, 0x06, 0x06, 0x7C, 0x00,
    // : (58)
    0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00,
    // ; (59)
    0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x30,
    // < (60)
    0x0C, 0x18, 0x30, 0x60, 0x30, 0x18, 0x0C, 0x00,
    // = (61)
    0x00, 0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00,
    // > (62)
    0x30, 0x18, 0x0C, 0x06, 0x0C, 0x18, 0x30, 0x00,
    // ? (63)
    0x3C, 0x66, 0x0C, 0x18, 0x18, 0x00, 0x18, 0x00,
    // @ (64)
    0x7C, 0xC6, 0xDE, 0xDE, 0xDE, 0xC0, 0x7E, 0x00,
    // A (65)
    0x38, 0x6C, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0x00,
    // B (66)
    0xFC, 0xC6, 0xC6, 0xFC, 0xC6, 0xC6, 0xFC, 0x00,
    // C (67)
    0x7C, 0xC6, 0xC0, 0xC0, 0xC0, 0xC6, 0x7C, 0x00,
    // D (68)
    0xF8, 0xCC, 0xC6, 0xC6, 0xC6, 0xCC, 0xF8, 0x00,
    // E (69)
    0xFE, 0xC0, 0xC0, 0xF8, 0xC0, 0xC0, 0xFE, 0x00,
    // F (70)
    0xFE, 0xC0, 0xC0, 0xF8, 0xC0, 0xC0, 0xC0, 0x00,
    // G (71)
    0x7C, 0xC6, 0xC0, 0xCE, 0xC6, 0xC6, 0x7C, 0x00,
    // H (72)
    0xC6, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0xC6, 0x00,
    // I (73)
    0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00,
    // J (74)
    0x06, 0x06, 0x06, 0x06, 0xC6, 0xC6, 0x7C, 0x00,
    // K (75)
    0xC6, 0xCC, 0xD8, 0xF0, 0xD8, 0xCC, 0xC6, 0x00,
    // L (76)
    0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xFE, 0x00,
    // M (77)
    0xC6, 0xEE, 0xFE, 0xD6, 0xC6, 0xC6, 0xC6, 0x00,
    // N (78)
    0xC6, 0xE6, 0xF6, 0xDE, 0xCE, 0xC6, 0xC6, 0x00,
    // O (79)
    0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0x00,
    // P (80)
    0xFC, 0xC6, 0xC6, 0xFC, 0xC0, 0xC0, 0xC0, 0x00,
    // Q (81)
    0x7C, 0xC6, 0xC6, 0xC6, 0xD6, 0xDE, 0x7C, 0x06,
    // R (82)
    0xFC, 0xC6, 0xC6, 0xFC, 0xD8, 0xCC, 0xC6, 0x00,
    // S (83)
    0x7C, 0xC6, 0xC0, 0x7C, 0x06, 0xC6, 0x7C, 0x00,
    // T (84)
    0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00,
    // U (85)
    0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0x00,
    // V (86)
    0xC6, 0xC6, 0xC6, 0xC6, 0x6C, 0x38, 0x10, 0x00,
    // W (87)
    0xC6, 0xC6, 0xC6, 0xD6, 0xFE, 0xEE, 0xC6, 0x00,
    // X (88)
    0xC6, 0xC6, 0x6C, 0x38, 0x6C, 0xC6, 0xC6, 0x00,
    // Y (89)
    0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00,
    // Z (90)
    0xFE, 0x06, 0x0C, 0x18, 0x30, 0x60, 0xFE, 0x00,
    // [ (91)
    0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00,
    // \ (92)
    0xC0, 0x60, 0x30, 0x18, 0x0C, 0x06, 0x02, 0x00,
    // ] (93)
    0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00,
    // ^ (94)
    0x10, 0x38, 0x6C, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // _ (95)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFE,
    // ` (96)
    0x18, 0x18, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00,
    // a (97)
    0x00, 0x00, 0x7C, 0x06, 0x7E, 0xC6, 0x7E, 0x00,
    // b (98)
    0xC0, 0xC0, 0xFC, 0xC6, 0xC6, 0xC6, 0xFC, 0x00,
    // c (99)
    0x00, 0x00, 0x7C, 0xC6, 0xC0, 0xC6, 0x7C, 0x00,
    // d (100)
    0x06, 0x06, 0x7E, 0xC6, 0xC6, 0xC6, 0x7E, 0x00,
    // e (101)
    0x00, 0x00, 0x7C, 0xC6, 0xFE, 0xC0, 0x7C, 0x00,
    // f (102)
    0x1C, 0x30, 0x30, 0x7C, 0x30, 0x30, 0x30, 0x00,
    // g (103)
    0x00, 0x00, 0x7E, 0xC6, 0xC6, 0x7E, 0x06, 0x7C,
    // h (104)
    0xC0, 0xC0, 0xFC, 0xC6, 0xC6, 0xC6, 0xC6, 0x00,
    // i (105)
    0x18, 0x00, 0x38, 0x18, 0x18, 0x18, 0x3C, 0x00,
    // j (106)
    0x18, 0x00, 0x38, 0x18, 0x18, 0x18, 0x18, 0x70,
    // k (107)
    0xC0, 0xC0, 0xC6, 0xCC, 0xF8, 0xCC, 0xC6, 0x00,
    // l (108)
    0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00,
    // m (109)
    0x00, 0x00, 0xEC, 0xFE, 0xD6, 0xC6, 0xC6, 0x00,
    // n (110)
    0x00, 0x00, 0xFC, 0xC6, 0xC6, 0xC6, 0xC6, 0x00,
    // o (111)
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xC6, 0x7C, 0x00,
    // p (112)
    0x00, 0x00, 0xFC, 0xC6, 0xC6, 0xFC, 0xC0, 0xC0,
    // q (113)
    0x00, 0x00, 0x7E, 0xC6, 0xC6, 0x7E, 0x06, 0x06,
    // r (114)
    0x00, 0x00, 0xDC, 0xE6, 0xC0, 0xC0, 0xC0, 0x00,
    // s (115)
    0x00, 0x00, 0x7E, 0xC0, 0x7C, 0x06, 0xFC, 0x00,
    // t (116)
    0x30, 0x30, 0x7C, 0x30, 0x30, 0x30, 0x1C, 0x00,
    // u (117)
    0x00, 0x00, 0xC6, 0xC6, 0xC6, 0xC6, 0x7E, 0x00,
    // v (118)
    0x00, 0x00, 0xC6, 0xC6, 0xC6, 0x6C, 0x38, 0x00,
    // w (119)
    0x00, 0x00, 0xC6, 0xC6, 0xD6, 0xFE, 0x6C, 0x00,
    // x (120)
    0x00, 0x00, 0xC6, 0x6C, 0x38, 0x6C, 0xC6, 0x00,
    // y (121)
    0x00, 0x00, 0xC6, 0xC6, 0xC6, 0x7E, 0x06, 0x7C,
    // z (122)
    0x00, 0x00, 0xFE, 0x0C, 0x38, 0x60, 0xFE, 0x00,
    // { (123)
    0x0E, 0x18, 0x18, 0x70, 0x18, 0x18, 0x0E, 0x00,
    // | (124)
    0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00,
    // } (125)
    0x70, 0x18, 0x18, 0x0E, 0x18, 0x18, 0x70, 0x00,
    // ~ (126)
    0x72, 0x9C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

// ============================================================================
// 8x16 Large Bitmap Font
// ============================================================================

/// 8x16 bitmap font for ASCII characters 32-126 (95 characters)
/// Each character is 16 bytes, one byte per row, MSB is leftmost pixel
static FONT_8X16: [u8; 95 * 16] = [
    // Space (32)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // ! (33)
    0x00, 0x00, 0x18, 0x3C, 0x3C, 0x3C, 0x18, 0x18,
    0x18, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00,
    // " (34)
    0x00, 0x66, 0x66, 0x66, 0x24, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // # (35)
    0x00, 0x00, 0x00, 0x6C, 0x6C, 0xFE, 0x6C, 0x6C,
    0x6C, 0xFE, 0x6C, 0x6C, 0x00, 0x00, 0x00, 0x00,
    // $ (36)
    0x18, 0x18, 0x7C, 0xC6, 0xC2, 0xC0, 0x7C, 0x06,
    0x06, 0x86, 0xC6, 0x7C, 0x18, 0x18, 0x00, 0x00,
    // % (37)
    0x00, 0x00, 0x00, 0x00, 0xC2, 0xC6, 0x0C, 0x18,
    0x30, 0x60, 0xC6, 0x86, 0x00, 0x00, 0x00, 0x00,
    // & (38)
    0x00, 0x00, 0x38, 0x6C, 0x6C, 0x38, 0x76, 0xDC,
    0xCC, 0xCC, 0xCC, 0x76, 0x00, 0x00, 0x00, 0x00,
    // ' (39)
    0x00, 0x30, 0x30, 0x30, 0x60, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // ( (40)
    0x00, 0x00, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x30,
    0x30, 0x30, 0x18, 0x0C, 0x00, 0x00, 0x00, 0x00,
    // ) (41)
    0x00, 0x00, 0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x0C,
    0x0C, 0x0C, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00,
    // * (42)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x3C, 0xFF,
    0x3C, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // + (43)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x7E,
    0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // , (44)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x18, 0x18, 0x18, 0x30, 0x00, 0x00, 0x00,
    // - (45)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFE,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // . (46)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00,
    // / (47)
    0x00, 0x00, 0x00, 0x00, 0x02, 0x06, 0x0C, 0x18,
    0x30, 0x60, 0xC0, 0x80, 0x00, 0x00, 0x00, 0x00,
    // 0 (48)
    0x00, 0x00, 0x38, 0x6C, 0xC6, 0xC6, 0xD6, 0xD6,
    0xC6, 0xC6, 0x6C, 0x38, 0x00, 0x00, 0x00, 0x00,
    // 1 (49)
    0x00, 0x00, 0x18, 0x38, 0x78, 0x18, 0x18, 0x18,
    0x18, 0x18, 0x18, 0x7E, 0x00, 0x00, 0x00, 0x00,
    // 2 (50)
    0x00, 0x00, 0x7C, 0xC6, 0x06, 0x0C, 0x18, 0x30,
    0x60, 0xC0, 0xC6, 0xFE, 0x00, 0x00, 0x00, 0x00,
    // 3 (51)
    0x00, 0x00, 0x7C, 0xC6, 0x06, 0x06, 0x3C, 0x06,
    0x06, 0x06, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 4 (52)
    0x00, 0x00, 0x0C, 0x1C, 0x3C, 0x6C, 0xCC, 0xFE,
    0x0C, 0x0C, 0x0C, 0x1E, 0x00, 0x00, 0x00, 0x00,
    // 5 (53)
    0x00, 0x00, 0xFE, 0xC0, 0xC0, 0xC0, 0xFC, 0x06,
    0x06, 0x06, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 6 (54)
    0x00, 0x00, 0x38, 0x60, 0xC0, 0xC0, 0xFC, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 7 (55)
    0x00, 0x00, 0xFE, 0xC6, 0x06, 0x06, 0x0C, 0x18,
    0x30, 0x30, 0x30, 0x30, 0x00, 0x00, 0x00, 0x00,
    // 8 (56)
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xC6, 0x7C, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 9 (57)
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xC6, 0x7E, 0x06,
    0x06, 0x06, 0x0C, 0x78, 0x00, 0x00, 0x00, 0x00,
    // : (58)
    0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00,
    0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
    // ; (59)
    0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00,
    0x00, 0x18, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00,
    // < (60)
    0x00, 0x00, 0x00, 0x06, 0x0C, 0x18, 0x30, 0x60,
    0x30, 0x18, 0x0C, 0x06, 0x00, 0x00, 0x00, 0x00,
    // = (61)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x7E, 0x00, 0x00,
    0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // > (62)
    0x00, 0x00, 0x00, 0x60, 0x30, 0x18, 0x0C, 0x06,
    0x0C, 0x18, 0x30, 0x60, 0x00, 0x00, 0x00, 0x00,
    // ? (63)
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0x0C, 0x18, 0x18,
    0x18, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00,
    // @ (64)
    0x00, 0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xDE, 0xDE,
    0xDE, 0xDC, 0xC0, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // A (65)
    0x00, 0x00, 0x10, 0x38, 0x6C, 0xC6, 0xC6, 0xFE,
    0xC6, 0xC6, 0xC6, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // B (66)
    0x00, 0x00, 0xFC, 0x66, 0x66, 0x66, 0x7C, 0x66,
    0x66, 0x66, 0x66, 0xFC, 0x00, 0x00, 0x00, 0x00,
    // C (67)
    0x00, 0x00, 0x3C, 0x66, 0xC2, 0xC0, 0xC0, 0xC0,
    0xC0, 0xC2, 0x66, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // D (68)
    0x00, 0x00, 0xF8, 0x6C, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x6C, 0xF8, 0x00, 0x00, 0x00, 0x00,
    // E (69)
    0x00, 0x00, 0xFE, 0x66, 0x62, 0x68, 0x78, 0x68,
    0x60, 0x62, 0x66, 0xFE, 0x00, 0x00, 0x00, 0x00,
    // F (70)
    0x00, 0x00, 0xFE, 0x66, 0x62, 0x68, 0x78, 0x68,
    0x60, 0x60, 0x60, 0xF0, 0x00, 0x00, 0x00, 0x00,
    // G (71)
    0x00, 0x00, 0x3C, 0x66, 0xC2, 0xC0, 0xC0, 0xDE,
    0xC6, 0xC6, 0x66, 0x3A, 0x00, 0x00, 0x00, 0x00,
    // H (72)
    0x00, 0x00, 0xC6, 0xC6, 0xC6, 0xC6, 0xFE, 0xC6,
    0xC6, 0xC6, 0xC6, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // I (73)
    0x00, 0x00, 0x3C, 0x18, 0x18, 0x18, 0x18, 0x18,
    0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // J (74)
    0x00, 0x00, 0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C,
    0xCC, 0xCC, 0xCC, 0x78, 0x00, 0x00, 0x00, 0x00,
    // K (75)
    0x00, 0x00, 0xE6, 0x66, 0x66, 0x6C, 0x78, 0x78,
    0x6C, 0x66, 0x66, 0xE6, 0x00, 0x00, 0x00, 0x00,
    // L (76)
    0x00, 0x00, 0xF0, 0x60, 0x60, 0x60, 0x60, 0x60,
    0x60, 0x62, 0x66, 0xFE, 0x00, 0x00, 0x00, 0x00,
    // M (77)
    0x00, 0x00, 0xC6, 0xEE, 0xFE, 0xFE, 0xD6, 0xC6,
    0xC6, 0xC6, 0xC6, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // N (78)
    0x00, 0x00, 0xC6, 0xE6, 0xF6, 0xFE, 0xDE, 0xCE,
    0xC6, 0xC6, 0xC6, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // O (79)
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // P (80)
    0x00, 0x00, 0xFC, 0x66, 0x66, 0x66, 0x7C, 0x60,
    0x60, 0x60, 0x60, 0xF0, 0x00, 0x00, 0x00, 0x00,
    // Q (81)
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6,
    0xC6, 0xD6, 0xDE, 0x7C, 0x0C, 0x0E, 0x00, 0x00,
    // R (82)
    0x00, 0x00, 0xFC, 0x66, 0x66, 0x66, 0x7C, 0x6C,
    0x66, 0x66, 0x66, 0xE6, 0x00, 0x00, 0x00, 0x00,
    // S (83)
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0x60, 0x38, 0x0C,
    0x06, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // T (84)
    0x00, 0x00, 0x7E, 0x7E, 0x5A, 0x18, 0x18, 0x18,
    0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // U (85)
    0x00, 0x00, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // V (86)
    0x00, 0x00, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6,
    0xC6, 0x6C, 0x38, 0x10, 0x00, 0x00, 0x00, 0x00,
    // W (87)
    0x00, 0x00, 0xC6, 0xC6, 0xC6, 0xC6, 0xD6, 0xD6,
    0xD6, 0xFE, 0xEE, 0x6C, 0x00, 0x00, 0x00, 0x00,
    // X (88)
    0x00, 0x00, 0xC6, 0xC6, 0x6C, 0x7C, 0x38, 0x38,
    0x7C, 0x6C, 0xC6, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // Y (89)
    0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18,
    0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // Z (90)
    0x00, 0x00, 0xFE, 0xC6, 0x86, 0x0C, 0x18, 0x30,
    0x60, 0xC2, 0xC6, 0xFE, 0x00, 0x00, 0x00, 0x00,
    // [ (91)
    0x00, 0x00, 0x3C, 0x30, 0x30, 0x30, 0x30, 0x30,
    0x30, 0x30, 0x30, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // \ (92)
    0x00, 0x00, 0x00, 0x80, 0xC0, 0x60, 0x30, 0x18,
    0x0C, 0x06, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
    // ] (93)
    0x00, 0x00, 0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C,
    0x0C, 0x0C, 0x0C, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // ^ (94)
    0x10, 0x38, 0x6C, 0xC6, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // _ (95)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00,
    // ` (96)
    0x00, 0x30, 0x18, 0x0C, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // a (97)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0x0C, 0x7C,
    0xCC, 0xCC, 0xCC, 0x76, 0x00, 0x00, 0x00, 0x00,
    // b (98)
    0x00, 0x00, 0xE0, 0x60, 0x60, 0x78, 0x6C, 0x66,
    0x66, 0x66, 0x66, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // c (99)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x7C, 0xC6, 0xC0,
    0xC0, 0xC0, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // d (100)
    0x00, 0x00, 0x1C, 0x0C, 0x0C, 0x3C, 0x6C, 0xCC,
    0xCC, 0xCC, 0xCC, 0x76, 0x00, 0x00, 0x00, 0x00,
    // e (101)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x7C, 0xC6, 0xFE,
    0xC0, 0xC0, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // f (102)
    0x00, 0x00, 0x1C, 0x36, 0x32, 0x30, 0x78, 0x30,
    0x30, 0x30, 0x30, 0x78, 0x00, 0x00, 0x00, 0x00,
    // g (103)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x76, 0xCC, 0xCC,
    0xCC, 0xCC, 0xCC, 0x7C, 0x0C, 0xCC, 0x78, 0x00,
    // h (104)
    0x00, 0x00, 0xE0, 0x60, 0x60, 0x6C, 0x76, 0x66,
    0x66, 0x66, 0x66, 0xE6, 0x00, 0x00, 0x00, 0x00,
    // i (105)
    0x00, 0x00, 0x18, 0x18, 0x00, 0x38, 0x18, 0x18,
    0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // j (106)
    0x00, 0x00, 0x06, 0x06, 0x00, 0x0E, 0x06, 0x06,
    0x06, 0x06, 0x06, 0x06, 0x66, 0x66, 0x3C, 0x00,
    // k (107)
    0x00, 0x00, 0xE0, 0x60, 0x60, 0x66, 0x6C, 0x78,
    0x78, 0x6C, 0x66, 0xE6, 0x00, 0x00, 0x00, 0x00,
    // l (108)
    0x00, 0x00, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18,
    0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // m (109)
    0x00, 0x00, 0x00, 0x00, 0x00, 0xEC, 0xFE, 0xD6,
    0xD6, 0xD6, 0xD6, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // n (110)
    0x00, 0x00, 0x00, 0x00, 0x00, 0xDC, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00,
    // o (111)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x7C, 0xC6, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // p (112)
    0x00, 0x00, 0x00, 0x00, 0x00, 0xDC, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x7C, 0x60, 0x60, 0xF0, 0x00,
    // q (113)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x76, 0xCC, 0xCC,
    0xCC, 0xCC, 0xCC, 0x7C, 0x0C, 0x0C, 0x1E, 0x00,
    // r (114)
    0x00, 0x00, 0x00, 0x00, 0x00, 0xDC, 0x76, 0x66,
    0x60, 0x60, 0x60, 0xF0, 0x00, 0x00, 0x00, 0x00,
    // s (115)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x7C, 0xC6, 0x60,
    0x38, 0x0C, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // t (116)
    0x00, 0x00, 0x10, 0x30, 0x30, 0xFC, 0x30, 0x30,
    0x30, 0x30, 0x36, 0x1C, 0x00, 0x00, 0x00, 0x00,
    // u (117)
    0x00, 0x00, 0x00, 0x00, 0x00, 0xCC, 0xCC, 0xCC,
    0xCC, 0xCC, 0xCC, 0x76, 0x00, 0x00, 0x00, 0x00,
    // v (118)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x3C, 0x18, 0x00, 0x00, 0x00, 0x00,
    // w (119)
    0x00, 0x00, 0x00, 0x00, 0x00, 0xC6, 0xC6, 0xD6,
    0xD6, 0xD6, 0xFE, 0x6C, 0x00, 0x00, 0x00, 0x00,
    // x (120)
    0x00, 0x00, 0x00, 0x00, 0x00, 0xC6, 0x6C, 0x38,
    0x38, 0x38, 0x6C, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // y (121)
    0x00, 0x00, 0x00, 0x00, 0x00, 0xC6, 0xC6, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7E, 0x06, 0x0C, 0xF8, 0x00,
    // z (122)
    0x00, 0x00, 0x00, 0x00, 0x00, 0xFE, 0xCC, 0x18,
    0x30, 0x60, 0xC6, 0xFE, 0x00, 0x00, 0x00, 0x00,
    // { (123)
    0x00, 0x00, 0x0E, 0x18, 0x18, 0x18, 0x70, 0x18,
    0x18, 0x18, 0x18, 0x0E, 0x00, 0x00, 0x00, 0x00,
    // | (124)
    0x00, 0x00, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18,
    0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00,
    // } (125)
    0x00, 0x00, 0x70, 0x18, 0x18, 0x18, 0x0E, 0x18,
    0x18, 0x18, 0x18, 0x70, 0x00, 0x00, 0x00, 0x00,
    // ~ (126)
    0x00, 0x00, 0x76, 0xDC, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

// ============================================================================
// Dirty Rectangle Tracking
// ============================================================================

/// A rectangle marking a dirty (modified) region
#[derive(Clone, Copy, Default)]
pub struct DirtyRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl DirtyRect {
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// Check if this rect overlaps another
    pub fn overlaps(&self, other: &DirtyRect) -> bool {
        self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }

    /// Merge another rect into this one (bounding box union)
    pub fn merge(&mut self, other: &DirtyRect) {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.w).max(other.x + other.w);
        let y2 = (self.y + self.h).max(other.y + other.h);
        self.x = x1;
        self.y = y1;
        self.w = x2 - x1;
        self.h = y2 - y1;
    }

    /// Check if this rect contains a point
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

// ============================================================================
// Clipping Rectangle
// ============================================================================

/// A clipping rectangle that constrains drawing operations
#[derive(Clone, Copy)]
pub struct ClipRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl ClipRect {
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// Create a clip rect covering the full screen
    pub const fn full(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            w: width,
            h: height,
        }
    }

    /// Intersect this clip rect with another, returning the overlap
    pub fn intersect(&self, other: &ClipRect) -> Option<ClipRect> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.w).min(other.x + other.w);
        let y2 = (self.y + self.h).min(other.y + other.h);

        if x1 < x2 && y1 < y2 {
            Some(ClipRect {
                x: x1,
                y: y1,
                w: x2 - x1,
                h: y2 - y1,
            })
        } else {
            None
        }
    }

    /// Check if a point is inside this clip rect
    #[inline]
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    /// Clamp coordinates to this clip rect
    #[inline]
    pub fn clamp(&self, x: u32, y: u32) -> (u32, u32) {
        (
            x.max(self.x).min(self.x + self.w.saturating_sub(1)),
            y.max(self.y).min(self.y + self.h.saturating_sub(1)),
        )
    }
}

// ============================================================================
// Compositing Layer
// ============================================================================

/// Blend mode for layer compositing
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    /// No blending, source replaces destination
    Opaque,
    /// Standard alpha blending (source over)
    Alpha,
    /// Additive blending
    Additive,
    /// Multiplicative blending
    Multiply,
}

/// A compositing layer with its own buffer
pub struct Layer {
    /// Layer pixel data (ARGB8888)
    pub data: &'static mut [u32],
    /// Layer width
    pub width: u32,
    /// Layer height
    pub height: u32,
    /// X position on screen
    pub x: i32,
    /// Y position on screen
    pub y: i32,
    /// Layer visibility
    pub visible: bool,
    /// Layer opacity (0-255)
    pub opacity: u8,
    /// Blend mode
    pub blend_mode: BlendMode,
}

// ============================================================================
// Bitmap Structure
// ============================================================================

/// A bitmap image for blitting operations
pub struct Bitmap<'a> {
    /// Pixel data (ARGB8888)
    pub data: &'a [u32],
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

impl<'a> Bitmap<'a> {
    pub const fn new(data: &'a [u32], width: u32, height: u32) -> Self {
        Self {
            data,
            width,
            height,
        }
    }

    /// Get pixel at (x, y), returns transparent if out of bounds
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        if x < self.width && y < self.height {
            self.data[(y * self.width + x) as usize]
        } else {
            color::TRANSPARENT
        }
    }
}

// ============================================================================
// Framebuffer Structure
// ============================================================================

/// Framebuffer handle for direct display access with double buffering
pub struct Framebuffer {
    // ---- Public fields (API compatibility) ----
    /// Physical address of current back buffer
    pub addr: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Bytes per row (may include padding)
    pub pitch: u32,

    // ---- Double buffering ----
    /// Buffer addresses [0] and [1]
    buffers: [u32; BUFFER_COUNT],
    /// Which buffer is currently being displayed (front)
    front_buffer: usize,
    /// Which buffer we're drawing to (back) - addr points here
    back_buffer: usize,
    /// Total size of one buffer in bytes
    buffer_size: u32,
    /// Virtual height (physical * buffer_count for page flipping)
    virtual_height: u32,

    // ---- Dirty region tracking ----
    dirty_rects: [DirtyRect; MAX_DIRTY_RECTS],
    dirty_count: usize,
    /// Track full-screen dirty for optimization
    full_dirty: bool,

    // ---- Clipping ----
    clip_stack: [ClipRect; MAX_CLIP_DEPTH],
    clip_depth: usize,

    // ---- Frame timing ----
    frame_count: u64,
    vsync_enabled: bool,
}

impl Framebuffer {
    /// Allocate a new framebuffer via VideoCore mailbox
    pub fn new() -> Option<Self> {
        Self::with_size(SCREEN_WIDTH, SCREEN_HEIGHT)
    }

    /// Allocate a framebuffer with specific dimensions
    pub fn with_size(width: u32, height: u32) -> Option<Self> {
        let back_buffer = 0;
        // For double buffering, we allocate a virtual framebuffer that's
        // twice as tall and flip between top and bottom halves
        let virtual_height = height * (BUFFER_COUNT as u32);

        let mut mbox = MailboxBuffer::new();
        let mut idx = 0;

        // Header
        mbox.data[idx] = 35 * 4;
        idx += 1;
        mbox.data[idx] = 0;
        idx += 1;

        // Set physical size
        mbox.data[idx] = tags::SET_PHYSICAL_SIZE;
        idx += 1;
        mbox.data[idx] = 8;
        idx += 1;
        mbox.data[idx] = 8;
        idx += 1;
        mbox.data[idx] = width;
        idx += 1;
        mbox.data[idx] = height;
        idx += 1;

        // Set virtual size (taller for double buffering)
        mbox.data[idx] = tags::SET_VIRTUAL_SIZE;
        idx += 1;
        mbox.data[idx] = 8;
        idx += 1;
        mbox.data[idx] = 8;
        idx += 1;
        mbox.data[idx] = width;
        idx += 1;
        mbox.data[idx] = virtual_height;
        idx += 1;

        // Set virtual offset (start at 0,0)
        mbox.data[idx] = tags::SET_VIRTUAL_OFFSET;
        idx += 1;
        mbox.data[idx] = 8;
        idx += 1;
        mbox.data[idx] = 8;
        idx += 1;
        mbox.data[idx] = 0;
        idx += 1;
        mbox.data[idx] = 0;
        idx += 1;

        // Set depth
        mbox.data[idx] = tags::SET_DEPTH;
        idx += 1;
        mbox.data[idx] = 4;
        idx += 1;
        mbox.data[idx] = 4;
        idx += 1;
        mbox.data[idx] = BITS_PER_PIXEL;
        idx += 1;

        // Set pixel order (BGR)
        mbox.data[idx] = tags::SET_PIXEL_ORDER;
        idx += 1;
        mbox.data[idx] = 4;
        idx += 1;
        mbox.data[idx] = 4;
        idx += 1;
        mbox.data[idx] = 0;
        idx += 1;

        // Allocate buffer
        mbox.data[idx] = tags::ALLOCATE_BUFFER;
        idx += 1;
        mbox.data[idx] = 8;
        idx += 1;
        mbox.data[idx] = 8;
        idx += 1;
        let fb_addr_idx = idx;
        mbox.data[idx] = 16;
        idx += 1;
        mbox.data[idx] = 0;
        idx += 1;

        // Get pitch
        mbox.data[idx] = tags::GET_PITCH;
        idx += 1;
        mbox.data[idx] = 4;
        idx += 1;
        mbox.data[idx] = 4;
        idx += 1;
        let pitch_idx = idx;
        mbox.data[idx] = 0;
        idx += 1;

        // End tag
        mbox.data[idx] = 0;

        if mailbox_call(&mut mbox, 8) && mbox.data[fb_addr_idx] != 0 {
            let base_addr = mbox.data[fb_addr_idx] & 0x3FFF_FFFF;
            let pitch = mbox.data[pitch_idx];
            let buffer_size = pitch * height;

            // Calculate buffer addresses
            let buffer0 = base_addr;
            // let buffer1 = base_addr + buffer_size;

            // Initialize with single buffering
            let back_buffer = 0;

            Some(Self {
                addr: buffer0,
                width: mbox.data[5],
                height: mbox.data[6],
                pitch,
                buffers: [buffer0],
                front_buffer: 0,
                back_buffer,
                buffer_size,
                virtual_height,
                dirty_rects: [DirtyRect::default(); MAX_DIRTY_RECTS],
                dirty_count: 0,
                full_dirty: true,
                clip_stack: [ClipRect::full(width, height); MAX_CLIP_DEPTH],
                clip_depth: 0,
                frame_count: 0,
                vsync_enabled: true,
            })
        } else {
            None
        }
    }

    /// Get framebuffer size in bytes (single buffer)
    pub fn size(&self) -> usize {
        (self.pitch * self.height) as usize
    }

    /// Get pointer to current back buffer
    pub fn as_ptr(&self) -> *mut u32 {
        self.addr as *mut u32
    }

    /// Get current frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Enable or disable vsync
    pub fn set_vsync(&mut self, enabled: bool) {
        self.vsync_enabled = enabled;
    }

    // ========================================================================
    // Double Buffering
    // ========================================================================

    /// Swap front and back buffers (called by present)
    fn swap_buffers(&mut self) {
        if BUFFER_COUNT < 2 {
            return;
        }

        // Swap indices
        let temp = self.front_buffer;
        self.front_buffer = self.back_buffer;
        self.back_buffer = temp;

        // Update addr to point to new back buffer
        self.addr = self.buffers[self.back_buffer];
    }

    /// Set the virtual offset to display the front buffer
    fn flip_display(&mut self) {
        let y_offset = (self.front_buffer as u32) * self.height;

        let mut mbox = MailboxBuffer::new();
        mbox.data[0] = 8 * 4;
        mbox.data[1] = 0;
        mbox.data[2] = tags::SET_VIRTUAL_OFFSET;
        mbox.data[3] = 8;
        mbox.data[4] = 8;
        mbox.data[5] = 0;
        mbox.data[6] = y_offset;
        mbox.data[7] = 0;

        mailbox_call(&mut mbox, 8);
    }

    /// Wait for vertical sync
    /// Wait for vertical sync using VideoCore mailbox
    fn wait_vsync(&self) {
        let mut mbox = MailboxBuffer::new();
        mbox.data[0] = 8 * 4;           // Total size
        mbox.data[1] = 0;               // Request code
        mbox.data[2] = tags::WAIT_FOR_VSYNC;
        mbox.data[3] = 4;               // Value buffer size
        mbox.data[4] = 4;               // Request size
        mbox.data[5] = 0;               // Dummy value
        mbox.data[6] = 0;               // End tag

        // Call mailbox (channel 8 = tags)
        // If this fails (e.g. not supported), we fall back to a small delay
        if !mailbox_call(&mut mbox, 8) {
            // Fallback: ~16ms delay
            for _ in 0..50000 {
                core::hint::spin_loop();
            }
        }
    }

    // ========================================================================
    // Dirty Region Tracking
    // ========================================================================

    /// Mark a rectangular region as dirty
    pub fn mark_dirty(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if self.full_dirty {
            return;
        }

        let rect = DirtyRect::new(x, y, w, h);

        // Try to merge with existing rects
        for i in 0..self.dirty_count {
            if self.dirty_rects[i].overlaps(&rect) {
                self.dirty_rects[i].merge(&rect);
                return;
            }
        }

        // Add as new rect if space available
        if self.dirty_count < MAX_DIRTY_RECTS {
            self.dirty_rects[self.dirty_count] = rect;
            self.dirty_count += 1;
        } else {
            // Too many rects, mark whole screen dirty
            self.full_dirty = true;
        }
    }

    /// Mark entire screen as dirty
    pub fn mark_all_dirty(&mut self) {
        self.full_dirty = true;
    }

    /// Clear dirty tracking (called after present)
    fn clear_dirty(&mut self) {
        self.dirty_count = 0;
        self.full_dirty = false;
    }

    /// Check if any region is dirty
    pub fn is_dirty(&self) -> bool {
        self.full_dirty || self.dirty_count > 0
    }

    /// Get dirty rectangles for partial update
    pub fn dirty_rects(&self) -> &[DirtyRect] {
        if self.full_dirty {
            // Return a single rect covering the whole screen
            &[]
        } else {
            &self.dirty_rects[..self.dirty_count]
        }
    }

    // ========================================================================
    // Clipping Stack
    // ========================================================================

    /// Push a new clip rectangle onto the stack
    pub fn push_clip(&mut self, x: u32, y: u32, w: u32, h: u32) -> bool {
        if self.clip_depth >= MAX_CLIP_DEPTH - 1 {
            return false;
        }

        let new_clip = ClipRect::new(x, y, w, h);

        // Intersect with current clip
        let current = self.current_clip();
        if let Some(intersected) = current.intersect(&new_clip) {
            self.clip_depth += 1;
            self.clip_stack[self.clip_depth] = intersected;
            true
        } else {
            // No intersection - push an empty clip
            self.clip_depth += 1;
            self.clip_stack[self.clip_depth] = ClipRect::new(0, 0, 0, 0);
            true
        }
    }

    /// Pop the top clip rectangle
    pub fn pop_clip(&mut self) {
        if self.clip_depth > 0 {
            self.clip_depth -= 1;
        }
    }

    /// Reset clip stack to full screen
    pub fn reset_clip(&mut self) {
        self.clip_depth = 0;
        self.clip_stack[0] = ClipRect::full(self.width, self.height);
    }

    /// Get current clip rectangle
    #[inline]
    pub fn current_clip(&self) -> ClipRect {
        self.clip_stack[self.clip_depth]
    }

    /// Check if a point is within the current clip
    #[inline]
    fn is_clipped(&self, x: u32, y: u32) -> bool {
        !self.current_clip().contains(x, y)
    }

    // ========================================================================
    // Alpha Blending
    // ========================================================================

    /// Blend source color over destination using alpha
    #[inline]
    pub fn blend_alpha(src: u32, dst: u32) -> u32 {
        let sa = color::alpha(src) as u32;
        if sa == 0 {
            return dst;
        }
        if sa == 255 {
            return src;
        }

        let inv_sa = 255 - sa;

        let sr = color::red(src) as u32;
        let sg = color::green(src) as u32;
        let sb = color::blue(src) as u32;

        let dr = color::red(dst) as u32;
        let dg = color::green(dst) as u32;
        let db = color::blue(dst) as u32;
        let da = color::alpha(dst) as u32;

        let r = ((sr * sa + dr * inv_sa) / 255) as u8;
        let g = ((sg * sa + dg * inv_sa) / 255) as u8;
        let b = ((sb * sa + db * inv_sa) / 255) as u8;
        let a = ((sa * 255 + da * inv_sa) / 255) as u8;

        color::argb(a, r, g, b)
    }

    /// Additive blend
    #[inline]
    pub fn blend_additive(src: u32, dst: u32) -> u32 {
        let r = (color::red(src) as u16 + color::red(dst) as u16).min(255) as u8;
        let g = (color::green(src) as u16 + color::green(dst) as u16).min(255) as u8;
        let b = (color::blue(src) as u16 + color::blue(dst) as u16).min(255) as u8;
        color::rgb(r, g, b)
    }

    /// Multiply blend
    #[inline]
    pub fn blend_multiply(src: u32, dst: u32) -> u32 {
        let r = ((color::red(src) as u16 * color::red(dst) as u16) / 255) as u8;
        let g = ((color::green(src) as u16 * color::green(dst) as u16) / 255) as u8;
        let b = ((color::blue(src) as u16 * color::blue(dst) as u16) / 255) as u8;
        color::rgb(r, g, b)
    }

    // ========================================================================
    // Basic Drawing Primitives
    // ========================================================================

    /// Set a single pixel (with bounds and clip checking)
    #[inline]
    pub fn put_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x >= self.width || y >= self.height || self.is_clipped(x, y) {
            return;
        }
        let offset = y * self.pitch + x * 4;
        unsafe {
            write_volatile((self.addr + offset) as *mut u32, color);
        }
    }

    /// Set a single pixel with alpha blending
    #[inline]
    pub fn put_pixel_blend(&mut self, x: u32, y: u32, color: u32) {
        if x >= self.width || y >= self.height || self.is_clipped(x, y) {
            return;
        }
        let offset = y * self.pitch + x * 4;
        unsafe {
            let dst = read_volatile((self.addr + offset) as *const u32);
            let blended = Self::blend_alpha(color, dst);
            write_volatile((self.addr + offset) as *mut u32, blended);
        }
    }

    /// Set a single pixel (no bounds checking)
    #[inline]
    pub unsafe fn put_pixel_unchecked(&mut self, x: u32, y: u32, color: u32) {
        let offset = y * self.pitch + x * 4;
        write_volatile((self.addr + offset) as *mut u32, color);
    }

    /// Get a pixel value at (x, y)
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        if x >= self.width || y >= self.height {
            return color::TRANSPARENT;
        }
        let offset = y * self.pitch + x * 4;
        unsafe { read_volatile((self.addr + offset) as *const u32) }
    }

    /// Fill a rectangle with a solid color
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        let clip = self.current_clip();

        // Compute intersection with clip rect
        let x1 = x.max(clip.x);
        let y1 = y.max(clip.y);
        let x2 = (x + w).min(clip.x + clip.w).min(self.width);
        let y2 = (y + h).min(clip.y + clip.h).min(self.height);

        if x1 >= x2 || y1 >= y2 {
            return;
        }

        let base = self.addr as *mut u32;
        let pitch_words = (self.pitch / 4) as usize;

        for row in y1..y2 {
            let row_start = (row as usize) * pitch_words + (x1 as usize);
            for col in 0..(x2 - x1) {
                unsafe {
                    write_volatile(base.add(row_start + col as usize), color);
                }
            }
        }

        self.mark_dirty(x1, y1, x2 - x1, y2 - y1);
    }

    /// Fill a rectangle with alpha blending
    pub fn fill_rect_blend(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        let clip = self.current_clip();

        let x1 = x.max(clip.x);
        let y1 = y.max(clip.y);
        let x2 = (x + w).min(clip.x + clip.w).min(self.width);
        let y2 = (y + h).min(clip.y + clip.h).min(self.height);

        if x1 >= x2 || y1 >= y2 {
            return;
        }

        let base = self.addr as *mut u32;
        let pitch_words = (self.pitch / 4) as usize;

        for row in y1..y2 {
            let row_start = (row as usize) * pitch_words + (x1 as usize);
            for col in 0..(x2 - x1) {
                unsafe {
                    let ptr = base.add(row_start + col as usize);
                    let dst = read_volatile(ptr);
                    write_volatile(ptr, Self::blend_alpha(color, dst));
                }
            }
        }

        self.mark_dirty(x1, y1, x2 - x1, y2 - y1);
    }

    /// Draw a rectangle outline
    pub fn draw_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.draw_hline(x, y, w, color);
        self.draw_hline(x, y + h - 1, w, color);
        self.draw_vline(x, y, h, color);
        self.draw_vline(x + w - 1, y, h, color);
    }

    /// Draw a rounded rectangle
    pub fn draw_rounded_rect(&mut self, x: u32, y: u32, w: u32, h: u32, r: u32, color: u32) {
        if w < r * 2 || h < r * 2 {
            self.draw_rect(x, y, w, h, color);
            return;
        }

        // Horizontal lines (top and bottom, minus corners)
        self.draw_hline(x + r, y, w - 2 * r, color);
        self.draw_hline(x + r, y + h - 1, w - 2 * r, color);

        // Vertical lines (left and right, minus corners)
        self.draw_vline(x, y + r, h - 2 * r, color);
        self.draw_vline(x + w - 1, y + r, h - 2 * r, color);

        // Corner arcs
        self.draw_arc(x + r, y + r, r, 180, 270, color);
        self.draw_arc(x + w - 1 - r, y + r, r, 270, 360, color);
        self.draw_arc(x + r, y + h - 1 - r, r, 90, 180, color);
        self.draw_arc(x + w - 1 - r, y + h - 1 - r, r, 0, 90, color);
    }

    /// Fill a rounded rectangle
    pub fn fill_rounded_rect(&mut self, x: u32, y: u32, w: u32, h: u32, r: u32, color: u32) {
        if w < r * 2 || h < r * 2 {
            self.fill_rect(x, y, w, h, color);
            return;
        }

        // Center rectangle
        self.fill_rect(x, y + r, w, h - 2 * r, color);

        // Top and bottom strips
        self.fill_rect(x + r, y, w - 2 * r, r, color);
        self.fill_rect(x + r, y + h - r, w - 2 * r, r, color);

        // Corner circles
        self.fill_circle(x + r, y + r, r, color);
        self.fill_circle(x + w - 1 - r, y + r, r, color);
        self.fill_circle(x + r, y + h - 1 - r, r, color);
        self.fill_circle(x + w - 1 - r, y + h - 1 - r, r, color);
    }

    /// Clear entire screen to a color
    pub fn clear(&mut self, color: u32) {
        let total_words = (self.pitch / 4 * self.height) as usize;
        let base = self.addr as *mut u32;

        for i in 0..total_words {
            unsafe {
                write_volatile(base.add(i), color);
            }
        }

        self.mark_all_dirty();
    }

    // ========================================================================
    // Line Drawing (Bresenham's Algorithm)
    // ========================================================================

    /// Draw a horizontal line (optimized)
    pub fn draw_hline(&mut self, x: u32, y: u32, len: u32, color: u32) {
        let clip = self.current_clip();
        if y < clip.y || y >= clip.y + clip.h {
            return;
        }

        let x1 = x.max(clip.x);
        let x2 = (x + len).min(clip.x + clip.w).min(self.width);
        if x1 >= x2 {
            return;
        }

        let base = self.addr as *mut u32;
        let pitch_words = (self.pitch / 4) as usize;
        let row_start = (y as usize) * pitch_words;

        for px in x1..x2 {
            unsafe {
                write_volatile(base.add(row_start + px as usize), color);
            }
        }
    }

    /// Draw a vertical line (optimized)
    pub fn draw_vline(&mut self, x: u32, y: u32, len: u32, color: u32) {
        let clip = self.current_clip();
        if x < clip.x || x >= clip.x + clip.w {
            return;
        }

        let y1 = y.max(clip.y);
        let y2 = (y + len).min(clip.y + clip.h).min(self.height);
        if y1 >= y2 {
            return;
        }

        let base = self.addr as *mut u32;
        let pitch_words = (self.pitch / 4) as usize;

        for py in y1..y2 {
            unsafe {
                write_volatile(base.add(py as usize * pitch_words + x as usize), color);
            }
        }
    }

    /// Draw a line using Bresenham's algorithm
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        let mut x = x0;
        let mut y = y0;

        loop {
            if x >= 0 && y >= 0 {
                self.put_pixel(x as u32, y as u32, color);
            }

            if x == x1 && y == y1 {
                break;
            }

            let e2 = 2 * err;
            if e2 >= dy {
                if x == x1 {
                    break;
                }
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                if y == y1 {
                    break;
                }
                err += dx;
                y += sy;
            }
        }
    }

    /// Draw a thick line
    pub fn draw_line_thick(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, thickness: u32, color: u32) {
        if thickness <= 1 {
            self.draw_line(x0, y0, x1, y1, color);
            return;
        }

        let dx = x1 - x0;
        let dy = y1 - y0;

        // Use integer square root approximation
        let len_sq = dx * dx + dy * dy;
        if len_sq == 0 {
            self.fill_circle(x0 as u32, y0 as u32, thickness / 2, color);
            return;
        }

        let len = isqrt(len_sq as u32) as i32;
        if len == 0 {
            self.fill_circle(x0 as u32, y0 as u32, thickness / 2, color);
            return;
        }

        // Perpendicular vector (scaled by thickness/2)
        // px = -dy * thickness / (2 * len)
        // py = dx * thickness / (2 * len)
        let half_thick = thickness as i32 / 2;

        // Draw multiple parallel lines to create thickness
        for offset in -half_thick..=half_thick {
            // Offset perpendicular to line direction
            let ox = (-dy * offset) / len;
            let oy = (dx * offset) / len;
            self.draw_line(x0 + ox, y0 + oy, x1 + ox, y1 + oy, color);
        }
    }

    // ========================================================================
    // Circle Drawing (Midpoint Algorithm)
    // ========================================================================

    /// Draw a circle outline using midpoint algorithm
    pub fn draw_circle(&mut self, cx: u32, cy: u32, r: u32, color: u32) {
        if r == 0 {
            self.put_pixel(cx, cy, color);
            return;
        }

        let mut x = r as i32;
        let mut y = 0i32;
        let mut err = 0i32;

        while x >= y {
            self.put_pixel(cx.wrapping_add(x as u32), cy.wrapping_add(y as u32), color);
            self.put_pixel(cx.wrapping_add(y as u32), cy.wrapping_add(x as u32), color);
            self.put_pixel(cx.wrapping_sub(y as u32), cy.wrapping_add(x as u32), color);
            self.put_pixel(cx.wrapping_sub(x as u32), cy.wrapping_add(y as u32), color);
            self.put_pixel(cx.wrapping_sub(x as u32), cy.wrapping_sub(y as u32), color);
            self.put_pixel(cx.wrapping_sub(y as u32), cy.wrapping_sub(x as u32), color);
            self.put_pixel(cx.wrapping_add(y as u32), cy.wrapping_sub(x as u32), color);
            self.put_pixel(cx.wrapping_add(x as u32), cy.wrapping_sub(y as u32), color);

            y += 1;
            err += 1 + 2 * y;
            if 2 * (err - x) + 1 > 0 {
                x -= 1;
                err += 1 - 2 * x;
            }
        }
    }

    /// Fill a circle using midpoint algorithm
    pub fn fill_circle(&mut self, cx: u32, cy: u32, r: u32, color: u32) {
        if r == 0 {
            self.put_pixel(cx, cy, color);
            return;
        }

        let mut x = r as i32;
        let mut y = 0i32;
        let mut err = 0i32;

        while x >= y {
            // Draw horizontal spans
            let x1 = cx.saturating_sub(x as u32);
            let x2 = cx.saturating_add(x as u32);
            self.draw_hline(x1, cy.saturating_add(y as u32), x2 - x1 + 1, color);
            self.draw_hline(x1, cy.saturating_sub(y as u32), x2 - x1 + 1, color);

            let x1 = cx.saturating_sub(y as u32);
            let x2 = cx.saturating_add(y as u32);
            self.draw_hline(x1, cy.saturating_add(x as u32), x2 - x1 + 1, color);
            self.draw_hline(x1, cy.saturating_sub(x as u32), x2 - x1 + 1, color);

            y += 1;
            err += 1 + 2 * y;
            if 2 * (err - x) + 1 > 0 {
                x -= 1;
                err += 1 - 2 * x;
            }
        }
    }

    /// Draw an arc (portion of a circle outline)
    /// Uses integer math with a sine lookup table
    pub fn draw_arc(&mut self, cx: u32, cy: u32, r: u32, start_deg: u32, end_deg: u32, color: u32) {
        if r == 0 {
            self.put_pixel(cx, cy, color);
            return;
        }

        // Normalize angles
        let start = start_deg % 360;
        let end = if end_deg <= start_deg { end_deg + 360 } else { end_deg };

        // Draw using small angle steps
        let mut prev_x: Option<i32> = None;
        let mut prev_y: Option<i32> = None;

        for deg in start..=end {
            let angle = deg % 360;
            let (sin_val, cos_val) = sin_cos_deg(angle);

            // Scale by radius (sin/cos return values in range -256 to 256)
            let px = cx as i32 + ((cos_val * r as i32) >> 8);
            let py = cy as i32 + ((sin_val * r as i32) >> 8);

            if let (Some(prev_px), Some(prev_py)) = (prev_x, prev_y) {
                if px != prev_px || py != prev_py {
                    self.draw_line(prev_px, prev_py, px, py, color);
                }
            }

            prev_x = Some(px);
            prev_y = Some(py);
        }
    }

    // ========================================================================
    // Triangle Drawing
    // ========================================================================

    /// Draw a triangle outline
    pub fn draw_triangle(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
        self.draw_line(x0, y0, x1, y1, color);
        self.draw_line(x1, y1, x2, y2, color);
        self.draw_line(x2, y2, x0, y0, color);
    }

    /// Fill a triangle using scanline algorithm
    pub fn fill_triangle(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
        // Sort vertices by y coordinate
        let (mut x0, mut y0, mut x1, mut y1, mut x2, mut y2) = (x0, y0, x1, y1, x2, y2);

        if y0 > y1 {
            core::mem::swap(&mut x0, &mut x1);
            core::mem::swap(&mut y0, &mut y1);
        }
        if y1 > y2 {
            core::mem::swap(&mut x1, &mut x2);
            core::mem::swap(&mut y1, &mut y2);
        }
        if y0 > y1 {
            core::mem::swap(&mut x0, &mut x1);
            core::mem::swap(&mut y0, &mut y1);
        }

        if y0 == y2 {
            // Degenerate triangle
            return;
        }

        // Fill bottom flat triangle
        if y1 == y2 {
            self.fill_flat_bottom_triangle(x0, y0, x1, y1, x2, y2, color);
        }
        // Fill top flat triangle
        else if y0 == y1 {
            self.fill_flat_top_triangle(x0, y0, x1, y1, x2, y2, color);
        }
        // General case - split into two triangles
        else {
            let x3 = x0 + ((y1 - y0) * (x2 - x0)) / (y2 - y0);
            let y3 = y1;
            self.fill_flat_bottom_triangle(x0, y0, x1, y1, x3, y3, color);
            self.fill_flat_top_triangle(x1, y1, x3, y3, x2, y2, color);
        }
    }

    fn fill_flat_bottom_triangle(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, x2: i32, _y2: i32, color: u32) {
        let slope1 = (x1 - x0) as f32 / (y1 - y0) as f32;
        let slope2 = (x2 - x0) as f32 / (y1 - y0) as f32;

        let mut cx1 = x0 as f32;
        let mut cx2 = x0 as f32;

        for y in y0..=y1 {
            let x_start = cx1.min(cx2) as i32;
            let x_end = cx1.max(cx2) as i32;
            if y >= 0 {
                self.draw_hline(x_start.max(0) as u32, y as u32, (x_end - x_start + 1).max(0) as u32, color);
            }
            cx1 += slope1;
            cx2 += slope2;
        }
    }

    fn fill_flat_top_triangle(&mut self, x0: i32, y0: i32, x1: i32, _y1: i32, x2: i32, y2: i32, color: u32) {
        let slope1 = (x2 - x0) as f32 / (y2 - y0) as f32;
        let slope2 = (x2 - x1) as f32 / (y2 - y0) as f32;

        let mut cx1 = x2 as f32;
        let mut cx2 = x2 as f32;

        for y in (y0..=y2).rev() {
            let x_start = cx1.min(cx2) as i32;
            let x_end = cx1.max(cx2) as i32;
            if y >= 0 {
                self.draw_hline(x_start.max(0) as u32, y as u32, (x_end - x_start + 1).max(0) as u32, color);
            }
            cx1 -= slope1;
            cx2 -= slope2;
        }
    }

    // ========================================================================
    // Bitmap Blitting
    // ========================================================================

    /// Blit a bitmap at the specified position
    pub fn blit(&mut self, bitmap: &Bitmap, x: i32, y: i32) {
        self.blit_with_blend(bitmap, x, y, BlendMode::Opaque);
    }

    /// Blit a bitmap with alpha blending
    pub fn blit_alpha(&mut self, bitmap: &Bitmap, x: i32, y: i32) {
        self.blit_with_blend(bitmap, x, y, BlendMode::Alpha);
    }

    /// Blit a bitmap with specified blend mode
    pub fn blit_with_blend(&mut self, bitmap: &Bitmap, x: i32, y: i32, blend: BlendMode) {
        let clip = self.current_clip();

        // Calculate visible portion
        let src_x = if x < clip.x as i32 { (clip.x as i32 - x) as u32 } else { 0 };
        let src_y = if y < clip.y as i32 { (clip.y as i32 - y) as u32 } else { 0 };

        let dst_x = (x.max(clip.x as i32)) as u32;
        let dst_y = (y.max(clip.y as i32)) as u32;

        let w = (bitmap.width - src_x)
            .min(clip.x + clip.w - dst_x)
            .min(self.width - dst_x);
        let h = (bitmap.height - src_y)
            .min(clip.y + clip.h - dst_y)
            .min(self.height - dst_y);

        if w == 0 || h == 0 {
            return;
        }

        let base = self.addr as *mut u32;
        let pitch_words = (self.pitch / 4) as usize;

        for row in 0..h {
            let src_row = (src_y + row) * bitmap.width;
            let dst_row = (dst_y + row) as usize * pitch_words + dst_x as usize;

            for col in 0..w {
                let src_pixel = bitmap.data[(src_row + src_x + col) as usize];

                unsafe {
                    let ptr = base.add(dst_row + col as usize);
                    match blend {
                        BlendMode::Opaque => {
                            write_volatile(ptr, src_pixel);
                        }
                        BlendMode::Alpha => {
                            let dst_pixel = read_volatile(ptr);
                            write_volatile(ptr, Self::blend_alpha(src_pixel, dst_pixel));
                        }
                        BlendMode::Additive => {
                            let dst_pixel = read_volatile(ptr);
                            write_volatile(ptr, Self::blend_additive(src_pixel, dst_pixel));
                        }
                        BlendMode::Multiply => {
                            let dst_pixel = read_volatile(ptr);
                            write_volatile(ptr, Self::blend_multiply(src_pixel, dst_pixel));
                        }
                    }
                }
            }
        }

        self.mark_dirty(dst_x, dst_y, w, h);
    }

    /// Blit a bitmap with scaling
    pub fn blit_scaled(&mut self, bitmap: &Bitmap, x: i32, y: i32, scale_x: u32, scale_y: u32) {
        if scale_x == 0 || scale_y == 0 {
            return;
        }

        let dst_w = bitmap.width * scale_x;
        let dst_h = bitmap.height * scale_y;

        for dy in 0..dst_h {
            let src_y = dy / scale_y;
            for dx in 0..dst_w {
                let src_x = dx / scale_x;
                let pixel = bitmap.get_pixel(src_x, src_y);
                let px = x + dx as i32;
                let py = y + dy as i32;
                if px >= 0 && py >= 0 {
                    self.put_pixel(px as u32, py as u32, pixel);
                }
            }
        }
    }

    /// Blit a portion of a bitmap (sprite sheet support)
    pub fn blit_region(
        &mut self,
        bitmap: &Bitmap,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
        dst_x: i32,
        dst_y: i32,
    ) {
        for row in 0..src_h {
            for col in 0..src_w {
                let pixel = bitmap.get_pixel(src_x + col, src_y + row);
                let px = dst_x + col as i32;
                let py = dst_y + row as i32;
                if px >= 0 && py >= 0 && color::alpha(pixel) > 0 {
                    self.put_pixel_blend(px as u32, py as u32, pixel);
                }
            }
        }
    }

    // ========================================================================
    // Text Rendering
    // ========================================================================

    /// Draw a single character at pixel position (8x8 font)
    pub fn draw_char(&mut self, x: u32, y: u32, ch: u8, fg: u32, bg: u32) {
        let index = if ch >= 32 && ch <= 126 {
            (ch - 32) as usize
        } else {
            0
        };

        let glyph = &FONT_8X8[index * 8..(index + 1) * 8];

        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                let color = if (bits >> (7 - col)) & 1 != 0 { fg } else { bg };
                self.put_pixel(x + col, y + row as u32, color);
            }
        }
    }

    /// Draw a character with transparent background
    pub fn draw_char_transparent(&mut self, x: u32, y: u32, ch: u8, fg: u32) {
        let index = if ch >= 32 && ch <= 126 {
            (ch - 32) as usize
        } else {
            0
        };

        let glyph = &FONT_8X8[index * 8..(index + 1) * 8];

        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if (bits >> (7 - col)) & 1 != 0 {
                    self.put_pixel(x + col, y + row as u32, fg);
                }
            }
        }
    }

    /// Draw a single character using 8x16 font
    pub fn draw_char_large(&mut self, x: u32, y: u32, ch: u8, fg: u32, bg: u32) {
        let index = if ch >= 32 && ch <= 126 {
            (ch - 32) as usize
        } else {
            0
        };

        let glyph = &FONT_8X16[index * 16..(index + 1) * 16];

        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                let color = if (bits >> (7 - col)) & 1 != 0 { fg } else { bg };
                self.put_pixel(x + col, y + row as u32, color);
            }
        }
    }

    /// Draw a string at pixel position
    pub fn draw_text(&mut self, x: u32, y: u32, text: &[u8], fg: u32, bg: u32) {
        let mut cx = x;
        for &ch in text {
            if cx + CHAR_WIDTH > self.width {
                break;
            }
            self.draw_char(cx, y, ch, fg, bg);
            cx += CHAR_WIDTH;
        }
    }

    /// Draw text with transparent background
    pub fn draw_text_transparent(&mut self, x: u32, y: u32, text: &[u8], fg: u32) {
        let mut cx = x;
        for &ch in text {
            if cx + CHAR_WIDTH > self.width {
                break;
            }
            self.draw_char_transparent(cx, y, ch, fg);
            cx += CHAR_WIDTH;
        }
    }

    /// Draw a string (from &str)
    pub fn draw_str(&mut self, x: u32, y: u32, text: &str, fg: u32, bg: u32) {
        self.draw_text(x, y, text.as_bytes(), fg, bg);
    }

    /// Draw a string with transparent background
    pub fn draw_str_transparent(&mut self, x: u32, y: u32, text: &str, fg: u32) {
        self.draw_text_transparent(x, y, text.as_bytes(), fg);
    }

    /// Draw text using large font
    pub fn draw_text_large(&mut self, x: u32, y: u32, text: &[u8], fg: u32, bg: u32) {
        let mut cx = x;
        for &ch in text {
            if cx + CHAR_WIDTH_LARGE > self.width {
                break;
            }
            self.draw_char_large(cx, y, ch, fg, bg);
            cx += CHAR_WIDTH_LARGE;
        }
    }

    /// Draw text centered horizontally
    pub fn draw_text_centered(&mut self, y: u32, text: &[u8], fg: u32, bg: u32) {
        let text_width = text.len() as u32 * CHAR_WIDTH;
        let x = if text_width < self.width {
            (self.width - text_width) / 2
        } else {
            0
        };
        self.draw_text(x, y, text, fg, bg);
    }

    /// Draw text centered both horizontally and vertically
    pub fn draw_text_center(&mut self, text: &[u8], fg: u32, bg: u32) {
        let text_width = text.len() as u32 * CHAR_WIDTH;
        let x = if text_width < self.width {
            (self.width - text_width) / 2
        } else {
            0
        };
        let y = (self.height - CHAR_HEIGHT) / 2;
        self.draw_text(x, y, text, fg, bg);
    }

    /// Draw a character with 2x scaling
    pub fn draw_char_scaled(&mut self, x: u32, y: u32, ch: u8, fg: u32, bg: u32, scale: u32) {
        let index = if ch >= 32 && ch <= 126 {
            (ch - 32) as usize
        } else {
            0
        };

        let glyph = &FONT_8X8[index * 8..(index + 1) * 8];

        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8u32 {
                let color = if (bits >> (7 - col)) & 1 != 0 { fg } else { bg };
                // Draw a scale×scale block
                for sy in 0..scale {
                    for sx in 0..scale {
                        self.put_pixel(
                            x + col * scale + sx,
                            y + (row as u32) * scale + sy,
                            color,
                        );
                    }
                }
            }
        }
    }

    /// Draw text with scaling
    pub fn draw_text_scaled(&mut self, x: u32, y: u32, text: &[u8], fg: u32, bg: u32, scale: u32) {
        let mut cx = x;
        let char_w = CHAR_WIDTH * scale;
        for &ch in text {
            if cx + char_w > self.width {
                break;
            }
            self.draw_char_scaled(cx, y, ch, fg, bg, scale);
            cx += char_w;
        }
    }

    /// Get the width of a text string in pixels
    pub fn text_width(&self, text: &[u8]) -> u32 {
        text.len() as u32 * CHAR_WIDTH
    }

    /// Get the width of a text string (large font)
    pub fn text_width_large(&self, text: &[u8]) -> u32 {
        text.len() as u32 * CHAR_WIDTH_LARGE
    }

    // ========================================================================
    // Gradients and Effects
    // ========================================================================

    /// Fill a rectangle with a vertical gradient
    pub fn fill_rect_gradient_v(&mut self, x: u32, y: u32, w: u32, h: u32, top: u32, bottom: u32) {
        for row in 0..h {
            let t = ((row * 255) / h.max(1)) as u8;
            let c = color::lerp(top, bottom, t);
            self.draw_hline(x, y + row, w, c);
        }
    }

    /// Fill a rectangle with a horizontal gradient
    pub fn fill_rect_gradient_h(&mut self, x: u32, y: u32, w: u32, h: u32, left: u32, right: u32) {
        for col in 0..w {
            let t = ((col * 255) / w.max(1)) as u8;
            let c = color::lerp(left, right, t);
            self.draw_vline(x + col, y, h, c);
        }
    }

    /// Apply a fade effect to the entire screen
    pub fn fade(&mut self, amount: u8) {
        let fade_color = color::with_alpha(color::BLACK, amount);
        self.fill_rect_blend(0, 0, self.width, self.height, fade_color);
    }

    // ========================================================================
    // Display Sync
    // ========================================================================

    /// Flush framebuffer to display (swaps buffers with vsync)
    pub fn present(&mut self) {
        dsb();

        // Clean cache for the back buffer we just drew to
        unsafe {
            clean_dcache_range(self.addr as usize, self.size());
        }

        dsb();

        // Swap buffers
        self.swap_buffers();

        // Tell GPU to display the new front buffer
        self.flip_display();

        // Wait for vsync if enabled
        if self.vsync_enabled {
            self.wait_vsync();
        }

        // Clear dirty tracking for next frame
        self.clear_dirty();

        // Increment frame counter
        self.frame_count += 1;
    }

    /// Present without buffer swap (for single-buffered mode or debugging)
    pub fn present_immediate(&mut self) {
        dsb();
        unsafe {
            clean_dcache_range(self.addr as usize, self.size());
        }
        dsb();
    }

    // ========================================================================
    // GameBoy Screen Blitting
    // ========================================================================

    /// Blit GameBoy Color screen data with 2x scaling
    #[inline(always)]
    pub fn blit_gb_screen_gbc(&mut self, rgb_data: &[u8]) {
        let mut scanline = [0u32; GB_WIDTH * GB_SCALE];

        let base = self.addr as *mut u32;
        let pitch_words = (self.pitch / 4) as usize;

        for y in 0..GB_HEIGHT {
            let src_row = y * GB_WIDTH * 3;

            for x in 0..GB_WIDTH {
                let idx = src_row + x * 3;
                let color = 0xFF00_0000
                    | ((rgb_data[idx] as u32) << 16)
                    | ((rgb_data[idx + 1] as u32) << 8)
                    | (rgb_data[idx + 2] as u32);

                scanline[x * 2] = color;
                scanline[x * 2 + 1] = color;
            }

            let dst_y = GB_OFFSET_Y + y * GB_SCALE;
            unsafe {
                let row0 = base.add(dst_y * pitch_words + GB_OFFSET_X);
                let row1 = base.add((dst_y + 1) * pitch_words + GB_OFFSET_X);
                core::ptr::copy_nonoverlapping(scanline.as_ptr(), row0, scanline.len());
                core::ptr::copy_nonoverlapping(scanline.as_ptr(), row1, scanline.len());
            }
        }

        dsb();
    }

    /// Blit original GameBoy (DMG) screen data with 2x scaling
    #[inline(always)]
    pub fn blit_gb_screen_dmg(&mut self, pal_data: &[u8]) {
        let mut scanline = [0u32; GB_WIDTH * GB_SCALE];

        let base = self.addr as *mut u32;
        let pitch_words = (self.pitch / 4) as usize;

        for y in 0..GB_HEIGHT {
            let src_row = y * GB_WIDTH;

            for x in 0..GB_WIDTH {
                let pal_idx = pal_data[src_row + x] as usize;
                let color = if pal_idx < 4 {
                    GB_PALETTE[pal_idx]
                } else {
                    color::BLACK
                };
                scanline[x * 2] = color;
                scanline[x * 2 + 1] = color;
            }

            let dst_y = GB_OFFSET_Y + y * GB_SCALE;
            unsafe {
                let row0 = base.add(dst_y * pitch_words + GB_OFFSET_X);
                let row1 = base.add((dst_y + 1) * pitch_words + GB_OFFSET_X);
                core::ptr::copy_nonoverlapping(scanline.as_ptr(), row0, scanline.len());
                core::ptr::copy_nonoverlapping(scanline.as_ptr(), row1, scanline.len());
            }
        }

        dsb();
    }

    /// Draw a border around the GameBoy screen area
    pub fn draw_gb_border(&mut self, color: u32) {
        let border = 4u32;
        let x = GB_OFFSET_X as u32 - border;
        let y = GB_OFFSET_Y as u32 - border;
        let w = GB_SCALED_W as u32 + border * 2;
        let h = GB_SCALED_H as u32 + border * 2;

        self.fill_rect(x, y, w, border, color);
        self.fill_rect(x, y + h - border, w, border, color);
        self.fill_rect(x, y, border, h, color);
        self.fill_rect(x + w - border, y, border, h, color);
    }

    // ========================================================================
    // Utility Methods
    // ========================================================================

    /// Copy a region of the screen to another location
    pub fn copy_rect(&mut self, src_x: u32, src_y: u32, dst_x: u32, dst_y: u32, w: u32, h: u32) {
        // Handle overlapping regions by copying in the right order
        let base = self.addr as *mut u32;
        let pitch_words = (self.pitch / 4) as usize;

        if src_y < dst_y || (src_y == dst_y && src_x < dst_x) {
            // Copy bottom-to-top, right-to-left
            for row in (0..h).rev() {
                for col in (0..w).rev() {
                    let src_idx = (src_y + row) as usize * pitch_words + (src_x + col) as usize;
                    let dst_idx = (dst_y + row) as usize * pitch_words + (dst_x + col) as usize;
                    unsafe {
                        let pixel = read_volatile(base.add(src_idx));
                        write_volatile(base.add(dst_idx), pixel);
                    }
                }
            }
        } else {
            // Copy top-to-bottom, left-to-right
            for row in 0..h {
                for col in 0..w {
                    let src_idx = (src_y + row) as usize * pitch_words + (src_x + col) as usize;
                    let dst_idx = (dst_y + row) as usize * pitch_words + (dst_x + col) as usize;
                    unsafe {
                        let pixel = read_volatile(base.add(src_idx));
                        write_volatile(base.add(dst_idx), pixel);
                    }
                }
            }
        }

        self.mark_dirty(dst_x, dst_y, w, h);
    }

    /// Scroll the screen content vertically
    pub fn scroll_v(&mut self, pixels: i32, fill_color: u32) {
        if pixels == 0 {
            return;
        }

        let abs_pixels = pixels.unsigned_abs();
        if abs_pixels >= self.height {
            self.clear(fill_color);
            return;
        }

        if pixels > 0 {
            // Scroll down
            self.copy_rect(0, 0, 0, abs_pixels, self.width, self.height - abs_pixels);
            self.fill_rect(0, 0, self.width, abs_pixels, fill_color);
        } else {
            // Scroll up
            self.copy_rect(0, abs_pixels, 0, 0, self.width, self.height - abs_pixels);
            self.fill_rect(0, self.height - abs_pixels, self.width, abs_pixels, fill_color);
        }
    }

    /// Scroll the screen content horizontally
    pub fn scroll_h(&mut self, pixels: i32, fill_color: u32) {
        if pixels == 0 {
            return;
        }

        let abs_pixels = pixels.unsigned_abs();
        if abs_pixels >= self.width {
            self.clear(fill_color);
            return;
        }

        if pixels > 0 {
            // Scroll right
            self.copy_rect(0, 0, abs_pixels, 0, self.width - abs_pixels, self.height);
            self.fill_rect(0, 0, abs_pixels, self.height, fill_color);
        } else {
            // Scroll left
            self.copy_rect(abs_pixels, 0, 0, 0, self.width - abs_pixels, self.height);
            self.fill_rect(self.width - abs_pixels, 0, abs_pixels, self.height, fill_color);
        }
    }
}

// ============================================================================
// ROM Selector Display Trait Implementation
// ============================================================================

impl crate::subsystems::rom_selector::Display for Framebuffer {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn clear(&mut self, color: u32) {
        Framebuffer::clear(self, color);
    }

    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        Framebuffer::fill_rect(self, x, y, w, h, color);
    }

    fn draw_text(&mut self, x: u32, y: u32, text: &[u8], fg: u32, bg: u32) {
        Framebuffer::draw_text(self, x, y, text, fg, bg);
    }

    fn present(&mut self) {
        Framebuffer::present(self);
    }

    fn wait_vblank(&self) {
        if self.vsync_enabled {
            self.wait_vsync();
        } else {
            // Fallback delay if vsync disabled (~16ms)
            crate::platform_core::mmio::delay_us(16000);
        }
    }
}
