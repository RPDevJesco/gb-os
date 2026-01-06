//! Pi Zero 2 W Framebuffer
//!
//! Simple framebuffer implementation using VideoCore mailbox.

use crate::mailbox;

// ============================================================================
// Framebuffer Implementation
// ============================================================================

/// Pi Zero 2 W framebuffer
pub struct Framebuffer {
    /// Display width in pixels
    pub width: usize,
    /// Display height in pixels
    pub height: usize,
    /// Bytes per row
    pub pitch: usize,
    /// Bits per pixel
    pub depth: usize,
    /// Front buffer address (GPU framebuffer)
    addr: usize,
    /// Total framebuffer size
    size: usize,
}

impl Framebuffer {
    /// Initialize framebuffer via VideoCore mailbox.
    pub fn new(width: u32, height: u32, depth: u32) -> Option<Self> {
        let fb_info = mailbox::allocate_framebuffer(width, height, depth)?;

        Some(Self {
            width: fb_info.width as usize,
            height: fb_info.height as usize,
            pitch: fb_info.pitch as usize,
            depth: depth as usize,
            addr: fb_info.addr as usize,
            size: fb_info.size as usize,
        })
    }

    /// Get raw pointer to framebuffer
    #[inline]
    fn ptr(&self) -> *mut u8 {
        self.addr as *mut u8
    }

    /// Get framebuffer as mutable slice
    #[inline]
    pub fn as_slice(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr(), self.size) }
    }

    /// Clear the display to a solid color (ARGB)
    pub fn clear(&mut self, color: u32) {
        let fb = self.as_slice();
        let pixels = fb.len() / 4;

        // Fast 32-bit fill
        let fb32 = unsafe {
            core::slice::from_raw_parts_mut(fb.as_mut_ptr() as *mut u32, pixels)
        };
        fb32.fill(color);
    }

    /// Set a single pixel (ARGB)
    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }

        let offset = y * self.pitch + x * 4;
        unsafe {
            let ptr = self.ptr().add(offset) as *mut u32;
            core::ptr::write_volatile(ptr, color);
        }
    }

    /// Fill a rectangle
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);
        let pitch = self.pitch;

        let fb = self.as_slice();

        for py in y..y_end {
            let row_start = py * pitch + x * 4;
            let row_end = py * pitch + x_end * 4;
            let row = &mut fb[row_start..row_end];

            // Fill row as u32s
            let row32 = unsafe {
                core::slice::from_raw_parts_mut(
                    row.as_mut_ptr() as *mut u32,
                    x_end - x,
                )
            };
            row32.fill(color);
        }
    }

    /// Draw a horizontal line
    pub fn draw_hline(&mut self, x: usize, y: usize, w: usize, color: u32) {
        if y >= self.height {
            return;
        }
        let x_end = (x + w).min(self.width);
        for px in x..x_end {
            self.set_pixel(px, y, color);
        }
    }

    /// Draw a vertical line
    pub fn draw_vline(&mut self, x: usize, y: usize, h: usize, color: u32) {
        if x >= self.width {
            return;
        }
        let y_end = (y + h).min(self.height);
        for py in y..y_end {
            self.set_pixel(x, py, color);
        }
    }

    /// Draw a rectangle outline
    pub fn draw_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        self.draw_hline(x, y, w, color);
        self.draw_hline(x, y + h - 1, w, color);
        self.draw_vline(x, y, h, color);
        self.draw_vline(x + w - 1, y, h, color);
    }
}
