//! Double-Buffered GameBoy Display Adapter
//!
//! Renders to a cached RAM backbuffer, then bulk-copies to the framebuffer.
//! This avoids the massive overhead of individual writes to uncached GPU memory.
//!
//! # Performance Strategy
//!
//! 1. Backbuffer lives in normal cached RAM - writes are fast
//! 2. All pixel calculations happen on cached data
//! 3. Single bulk copy to framebuffer once per frame
//! 4. Uses 32-bit writes instead of byte-by-byte

extern crate alloc;

use alloc::vec::Vec;
use super::gpu::{SCREEN_H, SCREEN_W};

/// Scale factor for display
pub const SCALE: usize = 4;

/// Scaled dimensions
pub const SCALED_W: usize = SCREEN_W * SCALE; // 640
pub const SCALED_H: usize = SCREEN_H * SCALE; // 576

/// Display dimensions
pub const DISPLAY_W: usize = 640;
pub const DISPLAY_H: usize = 480;

/// Offset to center on display
pub const OFFSET_X: usize = (DISPLAY_W - SCALED_W) / 2; // 0 for 640 width
pub const OFFSET_Y: usize = 0; // Start at top, GB screen is 576 tall on 480 display

/// For 800x600 display
pub const OFFSET_X_800: usize = (800 - SCALED_W) / 2; // 80
pub const OFFSET_Y_800: usize = (600 - SCALED_H) / 2; // 12

/// Border color in ARGB format (0xAARRGGBB for little-endian as 0xBBGGRRAA)
pub const BORDER_COLOR: u32 = 0xFF202020;

/// Double-buffered display
pub struct DoubleBufferedDisplay {
    /// Cached backbuffer in RAM (BGRA format, 32-bit per pixel)
    backbuffer: Vec<u32>,
    /// Backbuffer width
    width: usize,
    /// Backbuffer height
    height: usize,
    /// Framebuffer pointer
    fb_ptr: *mut u8,
    /// Framebuffer pitch in bytes
    fb_pitch: usize,
    /// Bits per pixel
    bpp: u32,
    /// X offset for centering
    offset_x: usize,
    /// Y offset for centering
    offset_y: usize,
    /// Scaling factor
    scale: usize,
}

impl DoubleBufferedDisplay {
    /// Create a new double-buffered display
    ///
    /// # Arguments
    /// * `fb_ptr` - Pointer to the hardware framebuffer
    /// * `fb_pitch` - Framebuffer pitch in bytes
    /// * `fb_width` - Framebuffer width in pixels
    /// * `fb_height` - Framebuffer height in pixels
    /// * `bpp` - Bits per pixel (must be 32 for now)
    pub fn new(
        fb_ptr: *mut u8,
        fb_pitch: usize,
        fb_width: usize,
        fb_height: usize,
        bpp: u32,
    ) -> Self {
        // Calculate centering offsets based on actual display size
        let scale = SCALE;
        let scaled_w = SCREEN_W * scale;
        let scaled_h = SCREEN_H * scale;

        // Center horizontally, top-align vertically if screen is shorter
        let offset_x = if fb_width > scaled_w {
            (fb_width - scaled_w) / 2
        } else {
            0
        };
        let offset_y = if fb_height > scaled_h {
            (fb_height - scaled_h) / 2
        } else {
            0
        };

        // Allocate backbuffer
        let backbuffer_size = fb_width * fb_height;
        let mut backbuffer = Vec::with_capacity(backbuffer_size);
        backbuffer.resize(backbuffer_size, BORDER_COLOR);

        Self {
            backbuffer,
            width: fb_width,
            height: fb_height,
            fb_ptr,
            fb_pitch,
            bpp,
            offset_x,
            offset_y,
            scale,
        }
    }

    /// Blit GameBoy screen to backbuffer with scaling
    ///
    /// This is fast because it writes to cached RAM.
    #[inline]
    pub fn blit_gb_screen(&mut self, gb_data: &[u8]) {
        let scale = self.scale;
        let offset_x = self.offset_x;
        let offset_y = self.offset_y;
        let width = self.width;
        let backbuffer = &mut self.backbuffer;

        for gb_y in 0..SCREEN_H {
            let src_row_start = gb_y * SCREEN_W * 3;
            let dst_base_y = offset_y + gb_y * scale;

            // Pre-compute all colors for this row
            for gb_x in 0..SCREEN_W {
                let src_idx = src_row_start + gb_x * 3;

                // RGB from GameBoy -> BGRA for framebuffer
                let r = gb_data[src_idx] as u32;
                let g = gb_data[src_idx + 1] as u32;
                let b = gb_data[src_idx + 2] as u32;

                // Pack as BGRA (little-endian: 0xAARRGGBB stored as BB GG RR AA)
                let color = 0xFF000000 | (r << 16) | (g << 8) | b;

                let dst_base_x = offset_x + gb_x * scale;

                // Write scaled block
                for sy in 0..scale {
                    let row_idx = (dst_base_y + sy) * width + dst_base_x;

                    // Bounds check once per row
                    if dst_base_y + sy >= self.height {
                        break;
                    }

                    for sx in 0..scale {
                        if dst_base_x + sx < width {
                            backbuffer[row_idx + sx] = color;
                        }
                    }
                }
            }
        }
    }

    /// Optimized blit for 2x scale - uses 64-bit writes
    #[inline]
    pub fn blit_gb_screen_2x(&mut self, gb_data: &[u8]) {
        let offset_x = self.offset_x;
        let offset_y = self.offset_y;
        let width = self.width;
        let backbuffer = &mut self.backbuffer;

        for gb_y in 0..SCREEN_H {
            let src_row_start = gb_y * SCREEN_W * 3;
            let dst_y0 = offset_y + gb_y * 2;
            let dst_y1 = dst_y0 + 1;

            if dst_y1 >= self.height {
                break;
            }

            let row0_base = dst_y0 * width + offset_x;
            let row1_base = dst_y1 * width + offset_x;

            for gb_x in 0..SCREEN_W {
                let src_idx = src_row_start + gb_x * 3;

                let r = gb_data[src_idx] as u32;
                let g = gb_data[src_idx + 1] as u32;
                let b = gb_data[src_idx + 2] as u32;
                let color = 0xFF000000 | (r << 16) | (g << 8) | b;

                let dst_x = gb_x * 2;
                let idx0 = row0_base + dst_x;
                let idx1 = row1_base + dst_x;

                // Write 2 pixels per row
                backbuffer[idx0] = color;
                backbuffer[idx0 + 1] = color;
                backbuffer[idx1] = color;
                backbuffer[idx1 + 1] = color;
            }
        }
    }

    /// Optimized blit for 4x scale
    #[inline]
    pub fn blit_gb_screen_4x(&mut self, gb_data: &[u8]) {
        let offset_x = self.offset_x;
        let offset_y = self.offset_y;
        let width = self.width;
        let backbuffer = &mut self.backbuffer;

        for gb_y in 0..SCREEN_H {
            let src_row_start = gb_y * SCREEN_W * 3;
            let dst_base_y = offset_y + gb_y * 4;

            // Skip if all 4 rows would be off-screen
            if dst_base_y >= self.height {
                break;
            }

            for gb_x in 0..SCREEN_W {
                let src_idx = src_row_start + gb_x * 3;

                let r = gb_data[src_idx] as u32;
                let g = gb_data[src_idx + 1] as u32;
                let b = gb_data[src_idx + 2] as u32;
                let color = 0xFF000000 | (r << 16) | (g << 8) | b;

                let dst_base_x = offset_x + gb_x * 4;

                // Unrolled 4x4 block write
                for sy in 0..4 {
                    let y = dst_base_y + sy;
                    if y >= self.height {
                        break;
                    }
                    let row_idx = y * width + dst_base_x;
                    backbuffer[row_idx] = color;
                    backbuffer[row_idx + 1] = color;
                    backbuffer[row_idx + 2] = color;
                    backbuffer[row_idx + 3] = color;
                }
            }
        }
    }

    /// Flush backbuffer to hardware framebuffer
    ///
    /// This is the only place we touch the slow uncached framebuffer memory.
    /// Uses bulk copy for maximum throughput.
    #[inline]
    pub fn flush(&self) {
        if self.bpp != 32 {
            // Only 32bpp supported for now
            return;
        }

        let src = self.backbuffer.as_ptr();
        let dst = self.fb_ptr as *mut u32;
        let fb_pitch_words = self.fb_pitch / 4;
        let width = self.width;

        // Only copy the visible portion
        let copy_height = self.height.min(self.fb_pitch * 8 / self.bpp as usize);

        unsafe {
            for y in 0..copy_height {
                let src_row = src.add(y * width);
                let dst_row = dst.add(y * fb_pitch_words);

                // Copy entire row at once
                core::ptr::copy_nonoverlapping(src_row, dst_row, width);
            }
        }
    }

    /// Flush only the GameBoy screen region (faster than full flush)
    #[inline]
    pub fn flush_gb_region(&self) {
        if self.bpp != 32 {
            return;
        }

        let src = self.backbuffer.as_ptr();
        let dst = self.fb_ptr as *mut u32;
        let fb_pitch_words = self.fb_pitch / 4;
        let width = self.width;

        let scale = self.scale;
        let gb_w = SCREEN_W * scale;
        let gb_h = SCREEN_H * scale;

        let start_y = self.offset_y;
        let end_y = (self.offset_y + gb_h).min(self.height);
        let start_x = self.offset_x;

        unsafe {
            for y in start_y..end_y {
                let src_row = src.add(y * width + start_x);
                let dst_row = dst.add(y * fb_pitch_words + start_x);

                // Copy just the GB screen width
                core::ptr::copy_nonoverlapping(src_row, dst_row, gb_w);
            }
        }
    }

    /// Clear backbuffer to border color
    pub fn clear(&mut self) {
        self.backbuffer.fill(BORDER_COLOR);
    }

    /// Draw "NO GAME" placeholder
    pub fn draw_no_game(&mut self) {
        // Dark blue background
        let bg_color: u32 = 0xFF102040; // ARGB

        // Fill with background
        self.backbuffer.fill(bg_color);

        // Draw centered rectangle
        let rect_w = 400;
        let rect_h = 150;
        let rect_x = (self.width.saturating_sub(rect_w)) / 2;
        let rect_y = (self.height.saturating_sub(rect_h)) / 2;

        let rect_color: u32 = 0xFF304060;

        for y in rect_y..(rect_y + rect_h).min(self.height) {
            let row_start = y * self.width;
            for x in rect_x..(rect_x + rect_w).min(self.width) {
                self.backbuffer[row_start + x] = rect_color;
            }
        }
    }

    /// Get backbuffer dimensions
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Get current scale factor
    pub fn scale(&self) -> usize {
        self.scale
    }
}

// Legacy API compatibility - wraps the double-buffered display
// Use these if you need drop-in replacement for old code

/// Blit GameBoy screen to framebuffer with 4x scaling (legacy API)
///
/// Note: For better performance, use DoubleBufferedDisplay directly
/// to avoid allocating a new backbuffer each frame.
pub unsafe fn blit_scaled(gb_data: &[u8], fb: *mut u8, pitch: usize, bpp: u32) {
    // For legacy compatibility, we still do individual writes
    // but use 32-bit writes instead of byte-by-byte
    if bpp == 32 {
        blit_scaled_32bpp_fast(gb_data, fb, pitch);
    }
}

/// Faster 32bpp blit using 32-bit writes
unsafe fn blit_scaled_32bpp_fast(gb_data: &[u8], fb: *mut u8, pitch: usize) {
    let fb32 = fb as *mut u32;
    let pitch_words = pitch / 4;
    let offset_x = OFFSET_X_800;
    let offset_y = OFFSET_Y_800;

    for gb_y in 0..SCREEN_H {
        let src_row_start = gb_y * SCREEN_W * 3;
        let dst_base_y = offset_y + gb_y * SCALE;

        for gb_x in 0..SCREEN_W {
            let src_idx = src_row_start + gb_x * 3;

            let r = gb_data[src_idx] as u32;
            let g = gb_data[src_idx + 1] as u32;
            let b = gb_data[src_idx + 2] as u32;

            // BGRA format for little-endian
            let color = 0xFF000000 | (r << 16) | (g << 8) | b;

            let dst_base_x = offset_x + gb_x * SCALE;

            // Write 4x4 block
            for sy in 0..SCALE {
                let row_ptr = fb32.add((dst_base_y + sy) * pitch_words + dst_base_x);
                for sx in 0..SCALE {
                    row_ptr.add(sx).write_volatile(color);
                }
            }
        }
    }
}

/// Clear border areas (legacy API)
pub unsafe fn clear_borders(fb: *mut u8, pitch: usize, width: usize, height: usize, bpp: u32) {
    if bpp == 32 {
        let fb32 = fb as *mut u32;
        let pitch_words = pitch / 4;

        for y in 0..height {
            let row = fb32.add(y * pitch_words);
            for x in 0..width {
                row.add(x).write_volatile(BORDER_COLOR);
            }
        }
    }
}

/// Draw "NO GAME" placeholder (legacy API)
pub unsafe fn draw_no_game_screen(fb: *mut u8, pitch: usize, width: usize, height: usize, bpp: u32) {
    if bpp != 32 {
        return;
    }

    let fb32 = fb as *mut u32;
    let pitch_words = pitch / 4;
    let bg_color: u32 = 0xFF102040;

    // Fill background
    for y in 0..height {
        let row = fb32.add(y * pitch_words);
        for x in 0..width {
            row.add(x).write_volatile(bg_color);
        }
    }

    // Draw rectangle
    let rect_w = 400;
    let rect_h = 150;
    let rect_x = (width.saturating_sub(rect_w)) / 2;
    let rect_y = (height.saturating_sub(rect_h)) / 2;
    let rect_color: u32 = 0xFF304060;

    for y in rect_y..(rect_y + rect_h).min(height) {
        let row = fb32.add(y * pitch_words);
        for x in rect_x..(rect_x + rect_w).min(width) {
            row.add(x).write_volatile(rect_color);
        }
    }
}
