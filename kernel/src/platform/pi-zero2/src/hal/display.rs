//! Display Hardware Abstraction
//!
//! Abstracts the differences between:
//! - x86: VGA Mode 13h (320x200, 8-bit palette indexed)
//! - ARM: DPI framebuffer (640x480, 32-bit ARGB)

/// Pixel format for the display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8-bit palette indexed (VGA Mode 13h)
    Indexed8,
    /// 16-bit RGB565
    Rgb565,
    /// 24-bit RGB888
    Rgb888,
    /// 32-bit ARGB8888
    Argb8888,
}

/// Display information
#[derive(Debug, Clone, Copy)]
pub struct DisplayInfo {
    pub width: usize,
    pub height: usize,
    pub pitch: usize,  // Bytes per row
    pub format: PixelFormat,
}

/// Display trait for framebuffer operations
pub trait Display {
    /// Get display information
    fn info(&self) -> DisplayInfo;
    
    /// Get framebuffer as mutable slice
    fn framebuffer(&mut self) -> &mut [u8];
    
    /// Get back buffer (for double buffering)
    fn back_buffer(&mut self) -> &mut [u8];
    
    /// Flip buffers (swap front and back)
    fn flip(&mut self);
    
    /// Wait for vertical sync
    fn vsync(&self);
    
    /// Fill entire screen with color
    fn clear(&mut self, color: u32);
    
    /// Draw a single pixel
    fn draw_pixel(&mut self, x: usize, y: usize, color: u32);
    
    /// Fill a rectangle
    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32);
    
    /// Blit Game Boy frame (160x144) to display with scaling
    /// 
    /// # Arguments
    /// * `gb_pixels` - Game Boy pixel data (160x144, format depends on implementation)
    /// * `scale` - Integer scale factor (1, 2, or 3)
    fn blit_gb_frame(&mut self, gb_pixels: &[u8], scale: usize);
    
    /// Set palette entry (for indexed color modes)
    fn set_palette(&mut self, index: u8, r: u8, g: u8, b: u8);
}

/// Game Boy screen constants
pub const GB_WIDTH: usize = 160;
pub const GB_HEIGHT: usize = 144;

/// Calculate centered position for GB screen on display
pub fn center_gb_screen(display_width: usize, display_height: usize, scale: usize) -> (usize, usize) {
    let scaled_width = GB_WIDTH * scale;
    let scaled_height = GB_HEIGHT * scale;
    let x = (display_width - scaled_width) / 2;
    let y = (display_height - scaled_height) / 2;
    (x, y)
}

/// Color conversion utilities
pub mod colors {
    /// Convert RGB to 32-bit ARGB
    pub const fn rgb_to_argb(r: u8, g: u8, b: u8) -> u32 {
        0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }
    
    /// Convert RGB to RGB565
    pub const fn rgb_to_rgb565(r: u8, g: u8, b: u8) -> u16 {
        ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | ((b as u16) >> 3)
    }
    
    /// Game Boy green palette (classic DMG)
    pub const GB_PALETTE: [u32; 4] = [
        rgb_to_argb(155, 188, 15),   // Lightest
        rgb_to_argb(139, 172, 15),   // Light
        rgb_to_argb(48, 98, 48),     // Dark
        rgb_to_argb(15, 56, 15),     // Darkest
    ];
    
    // Standard UI colors
    pub const BLACK: u32 = rgb_to_argb(0, 0, 0);
    pub const WHITE: u32 = rgb_to_argb(255, 255, 255);
    pub const RED: u32 = rgb_to_argb(255, 0, 0);
    pub const GREEN: u32 = rgb_to_argb(0, 255, 0);
    pub const BLUE: u32 = rgb_to_argb(0, 0, 255);
    pub const DARK_GRAY: u32 = rgb_to_argb(64, 64, 64);
    pub const LIGHT_GRAY: u32 = rgb_to_argb(192, 192, 192);
    pub const YELLOW: u32 = rgb_to_argb(255, 255, 0);
    pub const CYAN: u32 = rgb_to_argb(0, 255, 255);
}
