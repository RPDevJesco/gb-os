//! Hardware Double-Buffered Framebuffer for Pi Zero 2 W
//!
//! This implementation uses TRUE hardware double buffering via the VideoCore GPU:
//! - Allocates virtual framebuffer with 2x height
//! - Draws to back buffer while front buffer is displayed
//! - Flips buffers instantly via SET_VIRTUAL_OFFSET (no memory copy!)
//!
//! Performance comparison:
//! - Software double buffer: ~1.2MB memcpy per frame (slow!)
//! - Hardware double buffer: Single mailbox call per frame (fast!)

#![allow(dead_code)]

use alloc::vec::Vec;
use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// Constants
// ============================================================================

/// Default display width
pub const SCREEN_WIDTH: u32 = 640;
/// Default display height
pub const SCREEN_HEIGHT: u32 = 480;
/// Bits per pixel
pub const SCREEN_DEPTH: u32 = 32;

/// GameBoy display dimensions
pub const GB_WIDTH: usize = 160;
pub const GB_HEIGHT: usize = 144;

// Mailbox registers
const PERIPHERAL_BASE: usize = 0x3F00_0000;
const MBOX_BASE: usize = PERIPHERAL_BASE + 0x0000_B880;
const MBOX_READ: usize = MBOX_BASE + 0x00;
const MBOX_STATUS: usize = MBOX_BASE + 0x18;
const MBOX_WRITE: usize = MBOX_BASE + 0x20;
const MBOX_FULL: u32 = 0x8000_0000;
const MBOX_EMPTY: u32 = 0x4000_0000;
const MBOX_RESPONSE_SUCCESS: u32 = 0x8000_0000;

// Mailbox tags
const TAG_SET_PHYSICAL_SIZE: u32 = 0x0004_8003;
const TAG_SET_VIRTUAL_SIZE: u32 = 0x0004_8004;
const TAG_SET_VIRTUAL_OFFSET: u32 = 0x0004_8009;
const TAG_SET_DEPTH: u32 = 0x0004_8005;
const TAG_SET_PIXEL_ORDER: u32 = 0x0004_8006;
const TAG_ALLOCATE_BUFFER: u32 = 0x0004_0001;
const TAG_GET_PITCH: u32 = 0x0004_0008;
const TAG_END: u32 = 0;

// Pixel order
const PIXEL_ORDER_BGR: u32 = 0;
const PIXEL_ORDER_RGB: u32 = 1;

// ============================================================================
// Mailbox Buffer
// ============================================================================

#[repr(C, align(16))]
struct MailboxBuffer {
    data: [u32; 64],
}

impl MailboxBuffer {
    const fn new() -> Self {
        Self { data: [0; 64] }
    }
}

#[inline]
fn mmio_read(addr: usize) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}

#[inline]
fn mmio_write(addr: usize, val: u32) {
    unsafe { write_volatile(addr as *mut u32, val) }
}

fn mailbox_call(buffer: &mut MailboxBuffer, channel: u8) -> bool {
    let addr = buffer.data.as_ptr() as u32;

    // Wait for mailbox to be ready
    while (mmio_read(MBOX_STATUS) & MBOX_FULL) != 0 {
        core::hint::spin_loop();
    }

    // Send message
    mmio_write(MBOX_WRITE, (addr & !0xF) | (channel as u32 & 0xF));

    // Wait for response
    loop {
        while (mmio_read(MBOX_STATUS) & MBOX_EMPTY) != 0 {
            core::hint::spin_loop();
        }

        let response = mmio_read(MBOX_READ);
        if (response & 0xF) == channel as u32 {
            return buffer.data[1] == MBOX_RESPONSE_SUCCESS;
        }
    }
}

// ============================================================================
// Hardware Double-Buffered Framebuffer
// ============================================================================

/// Hardware double-buffered framebuffer.
///
/// Uses VideoCore's native page-flipping for zero-copy buffer swaps.
pub struct Framebuffer {
    /// Hardware framebuffer base address
    hw_addr: usize,
    /// Total hardware buffer size (includes both pages)
    hw_size: usize,
    /// Display width in pixels
    width: usize,
    /// Display height in pixels
    height: usize,
    /// Bytes per row
    pitch: usize,
    /// Which buffer is currently being displayed (0 or 1)
    front_buffer: u8,
}

impl Framebuffer {
    /// Initialize framebuffer with hardware double buffering.
    ///
    /// Allocates a virtual framebuffer with 2x the height for page flipping.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        let mut mbox = MailboxBuffer::new();

        // Calculate buffer size - we need space for 35 u32s
        mbox.data[0] = 35 * 4;
        mbox.data[1] = 0;  // Request

        // Set physical (display) size
        mbox.data[2] = TAG_SET_PHYSICAL_SIZE;
        mbox.data[3] = 8;   // Value buffer size
        mbox.data[4] = 8;   // Request size
        mbox.data[5] = width;
        mbox.data[6] = height;

        // Set virtual size (2x height for double buffering)
        mbox.data[7] = TAG_SET_VIRTUAL_SIZE;
        mbox.data[8] = 8;
        mbox.data[9] = 8;
        mbox.data[10] = width;
        mbox.data[11] = height * 2;  // DOUBLE HEIGHT for two buffers!

        // Set depth (bits per pixel)
        mbox.data[12] = TAG_SET_DEPTH;
        mbox.data[13] = 4;
        mbox.data[14] = 4;
        mbox.data[15] = SCREEN_DEPTH;

        // Set pixel order (RGB for this display)
        mbox.data[16] = TAG_SET_PIXEL_ORDER;
        mbox.data[17] = 4;
        mbox.data[18] = 4;
        mbox.data[19] = PIXEL_ORDER_RGB;

        // Allocate buffer
        mbox.data[20] = TAG_ALLOCATE_BUFFER;
        mbox.data[21] = 8;
        mbox.data[22] = 8;
        mbox.data[23] = 16;  // Alignment -> becomes base address
        mbox.data[24] = 0;   // -> becomes size

        // Get pitch (bytes per row)
        mbox.data[25] = TAG_GET_PITCH;
        mbox.data[26] = 4;
        mbox.data[27] = 4;
        mbox.data[28] = 0;   // -> becomes pitch

        // End tag
        mbox.data[29] = TAG_END;

        if !mailbox_call(&mut mbox, 8) {
            return None;
        }

        // Extract results
        let fb_addr = mbox.data[23] & 0x3FFF_FFFF;  // Convert bus addr to ARM physical
        let fb_size = mbox.data[24];
        let pitch = mbox.data[28];

        if fb_addr == 0 || fb_size == 0 {
            return None;
        }

        let mut fb = Self {
            hw_addr: fb_addr as usize,
            hw_size: fb_size as usize,
            width: width as usize,
            height: height as usize,
            pitch: pitch as usize,
            front_buffer: 0,
        };

        // Clear both buffers to black
        fb.clear_buffer(0, 0xFF00_0000);
        fb.clear_buffer(1, 0xFF00_0000);

        // Start displaying buffer 0
        fb.set_display_offset(0);

        Some(fb)
    }

    /// Initialize with default 640x480 resolution.
    pub fn new_default() -> Option<Self> {
        Self::new(SCREEN_WIDTH, SCREEN_HEIGHT)
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    #[inline]
    pub fn width(&self) -> usize { self.width }

    #[inline]
    pub fn height(&self) -> usize { self.height }

    #[inline]
    pub fn pitch(&self) -> usize { self.pitch }

    #[inline]
    fn pitch_words(&self) -> usize { self.pitch / 4 }

    /// Get pointer to start of a buffer (0 = front, 1 = back)
    #[inline]
    fn buffer_ptr(&self, buffer: u8) -> *mut u32 {
        let offset = if buffer == 0 { 0 } else { self.height * self.pitch };
        (self.hw_addr + offset) as *mut u32
    }

    /// Get pointer to the BACK buffer (the one we draw to)
    #[inline]
    fn back_buffer_ptr(&self) -> *mut u32 {
        self.buffer_ptr(1 - self.front_buffer)
    }

    /// Get pointer to the FRONT buffer (the one being displayed)
    #[inline]
    fn front_buffer_ptr(&self) -> *mut u32 {
        self.buffer_ptr(self.front_buffer)
    }

    // ========================================================================
    // Buffer Flipping (THE KEY OPTIMIZATION!)
    // ========================================================================

    /// Set which buffer the display scans out from.
    fn set_display_offset(&mut self, buffer: u8) {
        let mut mbox = MailboxBuffer::new();

        mbox.data[0] = 8 * 4;
        mbox.data[1] = 0;
        mbox.data[2] = TAG_SET_VIRTUAL_OFFSET;
        mbox.data[3] = 8;
        mbox.data[4] = 8;
        mbox.data[5] = 0;  // X offset always 0
        mbox.data[6] = if buffer == 0 { 0 } else { self.height as u32 };
        mbox.data[7] = TAG_END;

        mailbox_call(&mut mbox, 8);
    }

    /// Swap front and back buffers.
    ///
    /// This is INSTANT - just tells the GPU to start scanning from a different offset.
    /// No memory copying required!
    pub fn swap_buffers(&mut self) {
        self.front_buffer = 1 - self.front_buffer;
        self.set_display_offset(self.front_buffer);
    }

    /// Wait for vertical sync (reduces tearing).
    ///
    /// Note: On Pi, vsync is tricky without interrupts. This is a simple frame delay.
    pub fn wait_vsync(&self) {
        // At 60Hz, frame time is ~16.67ms
        // Simple delay - not true vsync but prevents some tearing
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
    }

    // ========================================================================
    // Drawing Operations (to BACK buffer)
    // ========================================================================

    /// Clear a specific buffer to a color.
    fn clear_buffer(&self, buffer: u8, color: u32) {
        let ptr = self.buffer_ptr(buffer);
        let pitch_words = self.pitch_words();

        unsafe {
            for y in 0..self.height {
                let row = ptr.add(y * pitch_words);
                for x in 0..self.width {
                    write_volatile(row.add(x), color);
                }
            }
        }
    }

    /// Clear the back buffer to a color.
    pub fn clear(&mut self, color: u32) {
        self.clear_buffer(1 - self.front_buffer, color);
    }

    /// Set a pixel in the back buffer.
    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            let ptr = self.back_buffer_ptr();
            let offset = y * self.pitch_words() + x;
            unsafe {
                write_volatile(ptr.add(offset), color);
            }
        }
    }

    /// Set a pixel in the FRONT buffer (immediate display, use sparingly).
    #[inline]
    pub fn set_pixel_direct(&self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            let ptr = self.front_buffer_ptr();
            let offset = y * self.pitch_words() + x;
            unsafe {
                write_volatile(ptr.add(offset), color);
            }
        }
    }

    /// Fill a rectangle in the back buffer.
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        if x >= self.width || y >= self.height || w == 0 || h == 0 {
            return;
        }

        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);
        let pitch_words = self.pitch_words();
        let ptr = self.back_buffer_ptr();

        unsafe {
            for py in y..y_end {
                let row = ptr.add(py * pitch_words);
                for px in x..x_end {
                    write_volatile(row.add(px), color);
                }
            }
        }
    }

    /// Draw a horizontal line.
    pub fn draw_hline(&mut self, x: usize, y: usize, w: usize, color: u32) {
        if y >= self.height { return; }
        let x_end = (x + w).min(self.width);
        let pitch_words = self.pitch_words();
        let ptr = self.back_buffer_ptr();

        unsafe {
            let row = ptr.add(y * pitch_words);
            for px in x..x_end {
                write_volatile(row.add(px), color);
            }
        }
    }

    /// Draw a vertical line.
    pub fn draw_vline(&mut self, x: usize, y: usize, h: usize, color: u32) {
        if x >= self.width { return; }
        let y_end = (y + h).min(self.height);
        let pitch_words = self.pitch_words();
        let ptr = self.back_buffer_ptr();

        unsafe {
            for py in y..y_end {
                write_volatile(ptr.add(py * pitch_words + x), color);
            }
        }
    }

    /// Draw a rectangle outline.
    pub fn draw_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        if w == 0 || h == 0 { return; }
        self.draw_hline(x, y, w, color);
        self.draw_hline(x, y + h - 1, w, color);
        self.draw_vline(x, y, h, color);
        self.draw_vline(x + w - 1, y, h, color);
    }

    // ========================================================================
    // GameBoy Screen Blitting
    // ========================================================================

    /// Calculate centered position for GameBoy screen.
    pub fn gb_offset(&self, scale: usize) -> (usize, usize) {
        let scaled_w = GB_WIDTH * scale;
        let scaled_h = GB_HEIGHT * scale;
        let x = (self.width.saturating_sub(scaled_w)) / 2;
        let y = (self.height.saturating_sub(scaled_h)) / 2;
        (x, y)
    }

    /// Blit GameBoy Color screen (RGB888 data) with 2x scaling.
    pub fn blit_gbc(&mut self, rgb_data: &[u8]) {
        let (offset_x, offset_y) = self.gb_offset(2);
        let pitch_words = self.pitch_words();
        let ptr = self.back_buffer_ptr();

        if rgb_data.len() < GB_WIDTH * GB_HEIGHT * 3 {
            return;
        }

        let mut src_idx = 0;

        unsafe {
            for gb_y in 0..GB_HEIGHT {
                let dst_y0 = offset_y + gb_y * 2;
                let dst_y1 = dst_y0 + 1;

                let row0 = ptr.add(dst_y0 * pitch_words + offset_x);
                let row1 = ptr.add(dst_y1 * pitch_words + offset_x);

                for gb_x in 0..GB_WIDTH {
                    let r = rgb_data[src_idx] as u32;
                    let g = rgb_data[src_idx + 1] as u32;
                    let b = rgb_data[src_idx + 2] as u32;
                    // ARGB format (hardware is BGR but we handle that in pixel order setting)
                    let color = 0xFF00_0000 | (r << 16) | (g << 8) | b;
                    src_idx += 3;

                    let dst_x = gb_x * 2;

                    // Write 2x2 block
                    write_volatile(row0.add(dst_x), color);
                    write_volatile(row0.add(dst_x + 1), color);
                    write_volatile(row1.add(dst_x), color);
                    write_volatile(row1.add(dst_x + 1), color);
                }
            }
        }
    }

    /// Blit original GameBoy screen (indexed palette) with 2x scaling.
    pub fn blit_dmg(&mut self, indexed_data: &[u8], palette: &[u32; 4]) {
        let (offset_x, offset_y) = self.gb_offset(2);
        let pitch_words = self.pitch_words();
        let ptr = self.back_buffer_ptr();

        if indexed_data.len() < GB_WIDTH * GB_HEIGHT {
            return;
        }

        let mut src_idx = 0;

        unsafe {
            for gb_y in 0..GB_HEIGHT {
                let dst_y0 = offset_y + gb_y * 2;
                let dst_y1 = dst_y0 + 1;

                let row0 = ptr.add(dst_y0 * pitch_words + offset_x);
                let row1 = ptr.add(dst_y1 * pitch_words + offset_x);

                for gb_x in 0..GB_WIDTH {
                    let pal_idx = indexed_data[src_idx] as usize;
                    src_idx += 1;
                    let color = palette[pal_idx.min(3)];

                    let dst_x = gb_x * 2;

                    // Write 2x2 block
                    write_volatile(row0.add(dst_x), color);
                    write_volatile(row0.add(dst_x + 1), color);
                    write_volatile(row1.add(dst_x), color);
                    write_volatile(row1.add(dst_x + 1), color);
                }
            }
        }
    }

    /// Draw border around GameBoy screen area.
    pub fn draw_gb_border(&mut self, scale: usize, thickness: usize, color: u32) {
        let (ox, oy) = self.gb_offset(scale);
        let w = GB_WIDTH * scale;
        let h = GB_HEIGHT * scale;

        let bx = ox.saturating_sub(thickness);
        let by = oy.saturating_sub(thickness);
        let bw = w + thickness * 2;
        let bh = h + thickness * 2;

        // Top
        self.fill_rect(bx, by, bw, thickness, color);
        // Bottom
        self.fill_rect(bx, by + bh - thickness, bw, thickness, color);
        // Left
        self.fill_rect(bx, by, thickness, bh, color);
        // Right
        self.fill_rect(bx + bw - thickness, by, thickness, bh, color);
    }
}

// ============================================================================
// Colors
// ============================================================================

pub mod color {
    pub const BLACK: u32 = 0xFF00_0000;
    pub const WHITE: u32 = 0xFFFF_FFFF;
    pub const RED: u32 = 0xFFFF_0000;
    pub const GREEN: u32 = 0xFF00_FF00;
    pub const BLUE: u32 = 0xFF00_00FF;
    pub const CYAN: u32 = 0xFF00_FFFF;
    pub const YELLOW: u32 = 0xFFFF_FF00;
    pub const GRAY: u32 = 0xFF80_8080;
    pub const DARK_BLUE: u32 = 0xFF00_0040;
}

/// Classic DMG palette (green tint)
pub const DMG_PALETTE: [u32; 4] = [
    0xFFE0_F8D0,  // Lightest
    0xFF88_C070,  // Light
    0xFF34_6856,  // Dark
    0xFF08_1820,  // Darkest
];
