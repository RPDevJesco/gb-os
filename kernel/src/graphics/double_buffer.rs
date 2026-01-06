//! Double Buffering with VSync for Flicker-Free Rendering
//!
//! This module provides a back buffer for off-screen rendering,
//! then copies to VGA memory during vertical retrace for zero flicker.
//!
//! # How It Works
//! 1. All drawing happens to a RAM buffer (back buffer)
//! 2. When frame is complete, wait for VGA vertical retrace
//! 3. Copy entire back buffer to VGA in one fast operation
//! 4. The copy completes before the beam reaches visible area
//!
//! # Memory Layout
//! - Back buffer: 64KB static array in kernel BSS
//! - Front buffer: VGA memory at 0xA0000
#[cfg(target_arch = "x86")]
use crate::graphics::vga_mode13h::{SCREEN_WIDTH, SCREEN_HEIGHT};
use crate::gui::layout::{GB_X, GB_Y, GB_WIDTH, GB_HEIGHT};

/// VGA framebuffer base address
const VGA_BUFFER: *mut u8 = 0xA0000 as *mut u8;

/// VGA Input Status Register 1 (for vsync detection)
const VGA_STATUS_REG: u16 = 0x3DA;

/// Size of the framebuffer in bytes
const BUFFER_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

// =============================================================================
// Back Buffer Storage
// =============================================================================

/// The back buffer - 64KB for 320x200 @ 8bpp
/// This is in kernel BSS, so it's zero-initialized at startup
static mut BACK_BUFFER: [u8; BUFFER_SIZE] = [0u8; BUFFER_SIZE];

/// Flag to track if we've done initial clear
static mut BUFFER_INITIALIZED: bool = false;

// =============================================================================
// Public API
// =============================================================================

/// Initialize the double buffer system
/// Clears both buffers to black
pub fn init() {
    unsafe {
        // Clear back buffer
        BACK_BUFFER.fill(0);

        // Clear VGA buffer
        for i in 0..BUFFER_SIZE {
            core::ptr::write_volatile(VGA_BUFFER.add(i), 0);
        }

        BUFFER_INITIALIZED = true;
    }
}

/// Get mutable reference to the back buffer
/// All drawing should go here, not directly to VGA
#[inline]
pub fn back_buffer() -> &'static mut [u8] {
    unsafe { &mut BACK_BUFFER }
}

/// Get the back buffer as a const slice
#[inline]
pub fn back_buffer_ref() -> &'static [u8] {
    unsafe { &BACK_BUFFER }
}

/// Wait for vertical retrace to begin
/// This is when the electron beam is moving from bottom-right to top-left
/// and not drawing anything visible
#[inline]
fn wait_vsync() {
    unsafe {
        // Wait for any current retrace to end (if we're in one)
        while (inb(VGA_STATUS_REG) & 0x08) != 0 {
            core::hint::spin_loop();
        }

        // Wait for next retrace to start
        while (inb(VGA_STATUS_REG) & 0x08) == 0 {
            core::hint::spin_loop();
        }
    }
}

/// Copy back buffer to VGA (front buffer)
/// This is the "flip" operation
///
/// # Safety
/// Writes directly to VGA memory
#[inline(never)]
fn copy_to_vga() {
    unsafe {
        // Fast copy using rep movsb (or regular copy)
        core::ptr::copy_nonoverlapping(
            BACK_BUFFER.as_ptr(),
            VGA_BUFFER,
            BUFFER_SIZE
        );
    }
}

/// Flip buffers with VSync - the main entry point
/// Waits for vertical retrace, then copies back buffer to VGA
/// This ensures zero visible tearing or flicker
pub fn flip_vsync() {
    wait_vsync();
    copy_to_vga();
}

/// Flip buffers without waiting for VSync
/// Faster but may cause tearing on real hardware
/// Good for when you're already synced to frame timing
pub fn flip_immediate() {
    copy_to_vga();
}

// =============================================================================
// Optimized Partial Updates
// =============================================================================

/// Copy only a rectangular region from back buffer to VGA
/// Useful when you know only part of the screen changed
pub fn flip_region(x: usize, y: usize, width: usize, height: usize) {
    let end_x = (x + width).min(SCREEN_WIDTH);
    let end_y = (y + height).min(SCREEN_HEIGHT);

    unsafe {
        for row in y..end_y {
            let offset = row * SCREEN_WIDTH + x;
            let len = end_x - x;
            core::ptr::copy_nonoverlapping(
                BACK_BUFFER.as_ptr().add(offset),
                VGA_BUFFER.add(offset),
                len
            );
        }
    }
}

/// Copy only the Game Boy screen region (most common update)
pub fn flip_gameboy_region() {
    flip_region(GB_X, GB_Y, GB_WIDTH, GB_HEIGHT);
}

/// Copy only the overlay regions (sidebars + bottom)
pub fn flip_overlay_regions() {
    // Right sidebar
    let right_x = GB_X + GB_WIDTH + 4; // After GB screen + offset
    flip_region(right_x, 0, SCREEN_WIDTH - right_x, SCREEN_HEIGHT);

    // Left sidebar
    flip_region(0, 0, GB_X, SCREEN_HEIGHT);

    // Bottom bar
    let bottom_y = GB_Y + GB_HEIGHT;
    flip_region(GB_X, bottom_y, GB_WIDTH, SCREEN_HEIGHT - bottom_y);
}

// =============================================================================
// Helper: Blit Game Boy Screen to Back Buffer
// =============================================================================

/// Blit the Game Boy's rendered frame to the back buffer
/// This replaces the direct-to-VGA blit
#[inline]
pub fn blit_gb_to_backbuffer(pal_data: &[u8]) {
    let buffer = back_buffer();

    for y in 0..GB_HEIGHT {
        let src_offset = y * GB_WIDTH;
        let dst_offset = (GB_Y + y) * SCREEN_WIDTH + GB_X;

        // Copy one scanline
        buffer[dst_offset..dst_offset + GB_WIDTH]
            .copy_from_slice(&pal_data[src_offset..src_offset + GB_WIDTH]);
    }
}

/// Clear the overlay areas in the back buffer
/// Only needed on first frame or when overlay is toggled
pub fn clear_overlay_areas(color: u8) {
    let buffer = back_buffer();

    // Right sidebar
    let right_x = GB_X + GB_WIDTH + 4;
    for y in 0..SCREEN_HEIGHT {
        let start = y * SCREEN_WIDTH + right_x;
        let end = y * SCREEN_WIDTH + SCREEN_WIDTH;
        if end <= buffer.len() {
            buffer[start..end].fill(color);
        }
    }

    // Left sidebar
    for y in 0..SCREEN_HEIGHT {
        let start = y * SCREEN_WIDTH;
        let end = y * SCREEN_WIDTH + GB_X;
        if end <= buffer.len() {
            buffer[start..end].fill(color);
        }
    }

    // Bottom bar
    let bottom_y = GB_Y + GB_HEIGHT;
    for y in bottom_y..SCREEN_HEIGHT {
        let start = y * SCREEN_WIDTH + GB_X;
        let end = y * SCREEN_WIDTH + GB_X + GB_WIDTH;
        if end <= buffer.len() {
            buffer[start..end].fill(color);
        }
    }
}

/// Fill entire back buffer with a color
pub fn clear_backbuffer(color: u8) {
    back_buffer().fill(color);
}

// =============================================================================
// Low-level I/O
// =============================================================================

/// Read a byte from an I/O port
#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!(
    "in al, dx",
    out("al") value,
    in("dx") port,
    options(nomem, nostack, preserves_flags)
    );
    value
}

// =============================================================================
// Statistics / Debugging
// =============================================================================

/// Counts for performance monitoring
pub struct BufferStats {
    pub flips: u32,
    pub vsync_waits: u32,
}

static mut STATS: BufferStats = BufferStats {
    flips: 0,
    vsync_waits: 0,
};

/// Get buffer statistics
pub fn stats() -> &'static BufferStats {
    unsafe { &STATS }
}

/// Reset statistics
pub fn reset_stats() {
    unsafe {
        STATS.flips = 0;
        STATS.vsync_waits = 0;
    }
}
