//! Splash screen display
//!
//! Displays the GB-OS logo during boot while loading ROM.

use crate::display::framebuffer::Framebuffer;

/// Raw ARGB pixel data (320x213)
static SPLASH_DATA: &[u8] = include_bytes!("splash_screen.bin");

const SPLASH_WIDTH: usize = 320;
const SPLASH_HEIGHT: usize = 213;
const SCALE: usize = 2;

/// Display splash screen centered on framebuffer with 2x scaling
pub fn show(fb: &Framebuffer) {
    let scaled_w = SPLASH_WIDTH * SCALE;
    let scaled_h = SPLASH_HEIGHT * SCALE;

    // Center on screen
    let offset_x = (fb.width as usize - scaled_w) / 2;
    let offset_y = (fb.height as usize - scaled_h) / 2;

    let fb_base = fb.addr as *mut u32;
    let pitch_pixels = (fb.pitch / 4) as usize;

    // Get splash pixels as u32 slice
    let splash_pixels = unsafe {
        core::slice::from_raw_parts(
            SPLASH_DATA.as_ptr() as *const u32,
            SPLASH_WIDTH * SPLASH_HEIGHT
        )
    };

    // Blit with 2x scaling
    for y in 0..SPLASH_HEIGHT {
        for x in 0..SPLASH_WIDTH {
            let color = splash_pixels[y * SPLASH_WIDTH + x];

            // Write 2x2 block
            let dst_x = offset_x + x * SCALE;
            let dst_y = offset_y + y * SCALE;

            unsafe {
                let row0 = fb_base.add(dst_y * pitch_pixels + dst_x);
                let row1 = fb_base.add((dst_y + 1) * pitch_pixels + dst_x);

                *row0 = color;
                *row0.add(1) = color;
                *row1 = color;
                *row1.add(1) = color;
            }
        }
    }
}
