//! GameBoy Display Adapter
//!
//! Blits GameBoy's 160x144 screen to Rustacean OS's VESA framebuffer.
//! Handles the BGRA pixel format used by Rustacean OS.
//!
//! # Pixel Format
//!
//! Rustacean OS framebuffer uses BGRA format (32-bit):
//! - Byte 0: Blue
//! - Byte 1: Green
//! - Byte 2: Red
//! - Byte 3: Alpha (0xFF)
//!
//! GameBoy GPU outputs RGB:
//! - data[i+0]: Red
//! - data[i+1]: Green
//! - data[i+2]: Blue
//!
//! # Scaling
//!
//! GameBoy: 160x144
//! 4x scale: 640x576
//! Centered on 800x600: offset (80, 12)

use super::gpu::{SCREEN_H, SCREEN_W};

/// Scale factor for display
pub const SCALE: usize = 4;

/// Scaled dimensions
pub const SCALED_W: usize = SCREEN_W * SCALE; // 640
pub const SCALED_H: usize = SCREEN_H * SCALE; // 576

/// Offset to center on 800x600 display
pub const OFFSET_X: usize = (800 - SCALED_W) / 2; // 80
pub const OFFSET_Y: usize = (600 - SCALED_H) / 2; // 12

/// Border color (dark gray) in BGRA format
pub const BORDER_COLOR_B: u8 = 0x20;
pub const BORDER_COLOR_G: u8 = 0x20;
pub const BORDER_COLOR_R: u8 = 0x20;

/// Blit GameBoy screen to framebuffer with 4x scaling
///
/// Handles BGRA pixel format used by Rustacean OS.
/// Processes entire scanlines for better cache behavior.
///
/// # Arguments
/// * `gb_data` - GameBoy framebuffer (160x144x3 RGB bytes)
/// * `fb` - VESA framebuffer pointer
/// * `pitch` - Framebuffer pitch in bytes
/// * `bpp` - Bits per pixel (32, 24, or 16)
///
/// # Safety
/// Caller must ensure `fb` points to valid framebuffer memory with sufficient size.
pub unsafe fn blit_scaled(gb_data: &[u8], fb: *mut u8, pitch: usize, bpp: u32) {
    match bpp {
        32 => blit_scaled_32bpp(gb_data, fb, pitch),
        24 => blit_scaled_24bpp(gb_data, fb, pitch),
        16 => blit_scaled_16bpp(gb_data, fb, pitch),
        _ => blit_scaled_32bpp(gb_data, fb, pitch), // Default to 32bpp
    }
}

/// Blit for 32-bit BGRA framebuffer
unsafe fn blit_scaled_32bpp(gb_data: &[u8], fb: *mut u8, pitch: usize) {
    for y in 0..SCREEN_H {
        let src_row = &gb_data[y * SCREEN_W * 3..(y + 1) * SCREEN_W * 3];
        let base_y = OFFSET_Y + y * SCALE;

        // Draw SCALE rows for each GameBoy row
        for sy in 0..SCALE {
            let row_offset = (base_y + sy) * pitch + OFFSET_X * 4;
            let mut dst = fb.add(row_offset);

            for x in 0..SCREEN_W {
                let src_idx = x * 3;
                // GameBoy GPU outputs RGB
                let r = src_row[src_idx];
                let g = src_row[src_idx + 1];
                let b = src_row[src_idx + 2];

                // Write SCALE pixels horizontally
                for _ in 0..SCALE {
                    // BGRA format: B, G, R, A
                    dst.write_volatile(b);
                    dst.add(1).write_volatile(g);
                    dst.add(2).write_volatile(r);
                    dst.add(3).write_volatile(0xFF);
                    dst = dst.add(4);
                }
            }
        }
    }
}

/// Blit for 24-bit BGR framebuffer
unsafe fn blit_scaled_24bpp(gb_data: &[u8], fb: *mut u8, pitch: usize) {
    for y in 0..SCREEN_H {
        let src_row = &gb_data[y * SCREEN_W * 3..(y + 1) * SCREEN_W * 3];
        let base_y = OFFSET_Y + y * SCALE;

        for sy in 0..SCALE {
            let row_offset = (base_y + sy) * pitch + OFFSET_X * 3;
            let mut dst = fb.add(row_offset);

            for x in 0..SCREEN_W {
                let src_idx = x * 3;
                let r = src_row[src_idx];
                let g = src_row[src_idx + 1];
                let b = src_row[src_idx + 2];

                for _ in 0..SCALE {
                    // BGR format
                    dst.write_volatile(b);
                    dst.add(1).write_volatile(g);
                    dst.add(2).write_volatile(r);
                    dst = dst.add(3);
                }
            }
        }
    }
}

/// Blit for 16-bit RGB565 framebuffer
unsafe fn blit_scaled_16bpp(gb_data: &[u8], fb: *mut u8, pitch: usize) {
    for y in 0..SCREEN_H {
        let src_row = &gb_data[y * SCREEN_W * 3..(y + 1) * SCREEN_W * 3];
        let base_y = OFFSET_Y + y * SCALE;

        for sy in 0..SCALE {
            let row_offset = (base_y + sy) * pitch + OFFSET_X * 2;
            let mut dst = fb.add(row_offset) as *mut u16;

            for x in 0..SCREEN_W {
                let src_idx = x * 3;
                let r = src_row[src_idx] as u16;
                let g = src_row[src_idx + 1] as u16;
                let b = src_row[src_idx + 2] as u16;

                // RGB565: RRRRRGGGGGGBBBBB
                let rgb565 = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3);

                for _ in 0..SCALE {
                    dst.write_volatile(rgb565);
                    dst = dst.add(1);
                }
            }
        }
    }
}

/// Clear border areas around the GameBoy screen
pub unsafe fn clear_borders(fb: *mut u8, pitch: usize, width: usize, height: usize, bpp: u32) {
    match bpp {
        32 => clear_borders_32bpp(fb, pitch, width, height),
        24 => clear_borders_24bpp(fb, pitch, width, height),
        16 => clear_borders_16bpp(fb, pitch, width, height),
        _ => clear_borders_32bpp(fb, pitch, width, height),
    }
}

unsafe fn clear_borders_32bpp(fb: *mut u8, pitch: usize, width: usize, height: usize) {
    // Fill entire screen with border color first
    for y in 0..height {
        let row = fb.add(y * pitch);
        for x in 0..width {
            let pixel = row.add(x * 4);
            pixel.write_volatile(BORDER_COLOR_B);
            pixel.add(1).write_volatile(BORDER_COLOR_G);
            pixel.add(2).write_volatile(BORDER_COLOR_R);
            pixel.add(3).write_volatile(0xFF);
        }
    }
}

unsafe fn clear_borders_24bpp(fb: *mut u8, pitch: usize, width: usize, height: usize) {
    for y in 0..height {
        let row = fb.add(y * pitch);
        for x in 0..width {
            let pixel = row.add(x * 3);
            pixel.write_volatile(BORDER_COLOR_B);
            pixel.add(1).write_volatile(BORDER_COLOR_G);
            pixel.add(2).write_volatile(BORDER_COLOR_R);
        }
    }
}

unsafe fn clear_borders_16bpp(fb: *mut u8, pitch: usize, width: usize, height: usize) {
    let rgb565 = ((BORDER_COLOR_R as u16 >> 3) << 11)
        | ((BORDER_COLOR_G as u16 >> 2) << 5)
        | (BORDER_COLOR_B as u16 >> 3);

    for y in 0..height {
        let row = fb.add(y * pitch) as *mut u16;
        for x in 0..width {
            row.add(x).write_volatile(rgb565);
        }
    }
}

/// Draw "NO GAME" placeholder screen
pub unsafe fn draw_no_game_screen(fb: *mut u8, pitch: usize, width: usize, height: usize, bpp: u32) {
    // Dark blue background - BGRA format
    let bg_b: u8 = 0x40;
    let bg_g: u8 = 0x20;
    let bg_r: u8 = 0x10;

    if bpp == 32 {
        for y in 0..height {
            let row = fb.add(y * pitch);
            for x in 0..width {
                let pixel = row.add(x * 4);
                pixel.write_volatile(bg_b);
                pixel.add(1).write_volatile(bg_g);
                pixel.add(2).write_volatile(bg_r);
                pixel.add(3).write_volatile(0xFF);
            }
        }
    }

    // Draw a centered rectangle
    let rect_w = 400;
    let rect_h = 150;
    let rect_x = (width.saturating_sub(rect_w)) / 2;
    let rect_y = (height.saturating_sub(rect_h)) / 2;

    if bpp == 32 {
        for y in rect_y..(rect_y + rect_h).min(height) {
            let row = fb.add(y * pitch);
            for x in rect_x..(rect_x + rect_w).min(width) {
                let pixel = row.add(x * 4);
                pixel.write_volatile(0x60);      // B
                pixel.add(1).write_volatile(0x40); // G
                pixel.add(2).write_volatile(0x30); // R
                pixel.add(3).write_volatile(0xFF);
            }
        }
    }
}
