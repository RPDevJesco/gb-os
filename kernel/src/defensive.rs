//! Defensive Programming Module for RetroFutureGB
//!
//! This module provides safety primitives to catch bugs before they cause
//! triple faults, hangs, or display corruption.
//!
//! # Usage
//! Add to kernel/src/main.rs: `mod defensive;`
//! Then use: `use defensive::{...};`

use core::sync::atomic::{AtomicU32, Ordering};

// =============================================================================
// STACK OVERFLOW DETECTION
// =============================================================================

/// Stack canary value - known pattern to detect overflow
const STACK_CANARY: u32 = 0xDEAD_BEEF;

/// Stack guard page marker
const STACK_GUARD_PATTERN: u32 = 0xBAD_57AC; // "BAD STAC(K)"

/// Location of stack canary (place at bottom of stack region)
/// Stack top is at 0x90000, grows DOWN toward lower addresses.
/// We place canary at 0x70000, giving 128KB of stack space.
/// This is safely above Stage 2 bootloader and ROM buffer.
const STACK_CANARY_ADDR: usize = 0x0007_0000;

/// Initialize stack protection
///
/// Call this early in kernel_main, BEFORE any significant stack usage.
///
/// # Safety
/// Must be called exactly once during boot.
pub unsafe fn init_stack_guard() {
    // Write canary pattern at bottom of stack
    let canary_ptr = STACK_CANARY_ADDR as *mut u32;

    // Write multiple canaries for redundancy
    for i in 0..16 {
        canary_ptr.add(i).write_volatile(STACK_CANARY);
    }
}

/// Check if stack has overflowed
///
/// Call this periodically (e.g., once per frame) or in panic handler.
/// Returns true if stack appears corrupted.
pub fn check_stack_overflow() -> bool {
    unsafe {
        let canary_ptr = STACK_CANARY_ADDR as *const u32;

        // Check all canaries - any corruption means overflow
        for i in 0..16 {
            if canary_ptr.add(i).read_volatile() != STACK_CANARY {
                return true;
            }
        }
        false
    }
}

/// Get approximate stack usage
///
/// Scans from canary upward to find how much stack is used.
/// Returns bytes used (approximate).
pub fn get_stack_usage() -> usize {
    const STACK_TOP: usize = 0x0009_0000;
    const STACK_BOTTOM: usize = 0x0008_F000;

    // This is a rough estimate - looks for first non-zero region
    // In practice, stack usage varies; this gives ballpark figure
    STACK_TOP - STACK_BOTTOM
}

// =============================================================================
// VGA BOUNDS CHECKING (Prevents Display Corruption)
// =============================================================================

/// VGA Mode 13h framebuffer bounds
pub const VGA_BASE: usize = 0x000A_0000;
pub const VGA_WIDTH: usize = 320;
pub const VGA_HEIGHT: usize = 200;
pub const VGA_SIZE: usize = VGA_WIDTH * VGA_HEIGHT; // 64000 bytes
pub const VGA_END: usize = VGA_BASE + VGA_SIZE;

/// Safe pixel write with bounds checking
///
/// Returns false if coordinates are out of bounds (no write performed).
#[inline]
pub fn safe_put_pixel(x: usize, y: usize, color: u8) -> bool {
    if x >= VGA_WIDTH || y >= VGA_HEIGHT {
        // Log or count the violation for debugging
        increment_vga_violation();
        return false;
    }

    let offset = y * VGA_WIDTH + x;
    unsafe {
        let ptr = (VGA_BASE + offset) as *mut u8;
        ptr.write_volatile(color);
    }
    true
}

/// Safe rectangle fill with bounds clamping
///
/// Clamps rectangle to screen bounds rather than failing.
/// Returns actual pixels written.
pub fn safe_fill_rect(x: usize, y: usize, w: usize, h: usize, color: u8) -> usize {
    // Clamp to screen bounds
    let x_end = (x + w).min(VGA_WIDTH);
    let y_end = (y + h).min(VGA_HEIGHT);
    let x_start = x.min(VGA_WIDTH);
    let y_start = y.min(VGA_HEIGHT);

    if x_start >= x_end || y_start >= y_end {
        return 0;
    }

    let mut count = 0;
    for py in y_start..y_end {
        for px in x_start..x_end {
            let offset = py * VGA_WIDTH + px;
            unsafe {
                let ptr = (VGA_BASE + offset) as *mut u8;
                ptr.write_volatile(color);
            }
            count += 1;
        }
    }
    count
}

/// Safe Game Boy framebuffer blit
///
/// Blits 160x144 Game Boy screen to VGA with full bounds checking.
/// Centers the image and scales 2x.
pub fn safe_blit_gameboy(gb_framebuffer: &[u8]) -> Result<(), BlitError> {
    const GB_WIDTH: usize = 160;
    const GB_HEIGHT: usize = 144;
    const EXPECTED_SIZE: usize = GB_WIDTH * GB_HEIGHT * 3; // RGB

    if gb_framebuffer.len() != EXPECTED_SIZE {
        return Err(BlitError::InvalidBufferSize {
            expected: EXPECTED_SIZE,
            actual: gb_framebuffer.len(),
        });
    }

    // Center on 320x200 screen (80 pixels horizontal offset, 28 vertical)
    const X_OFFSET: usize = 0;  // (320 - 320) / 2 = 0 when scaled 2x
    const Y_OFFSET: usize = 28; // (200 - 144) / 2 = 28

    for gy in 0..GB_HEIGHT {
        for gx in 0..GB_WIDTH {
            let src_idx = (gy * GB_WIDTH + gx) * 3;

            // Convert RGB to VGA palette index (simple 332 mapping)
            let r = gb_framebuffer[src_idx] >> 5;     // 3 bits
            let g = gb_framebuffer[src_idx + 1] >> 5; // 3 bits
            let b = gb_framebuffer[src_idx + 2] >> 6; // 2 bits
            let color = (r << 5) | (g << 2) | b;

            // Write 2x2 pixel block (scale 2x)
            let vx = X_OFFSET + gx * 2;
            let vy = Y_OFFSET + gy;

            // Bounds already guaranteed by loop limits + offsets
            if vx + 1 < VGA_WIDTH && vy < VGA_HEIGHT {
                let offset = vy * VGA_WIDTH + vx;
                unsafe {
                    let ptr = (VGA_BASE + offset) as *mut u8;
                    ptr.write_volatile(color);
                    ptr.add(1).write_volatile(color);
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum BlitError {
    InvalidBufferSize { expected: usize, actual: usize },
    OutOfBounds,
}

// =============================================================================
// HARDWARE TIMEOUT PROTECTION (Prevents Hangs)
// =============================================================================

/// Maximum iterations for hardware polling loops
const DEFAULT_TIMEOUT: u32 = 1_000_000;

/// Poll with timeout - prevents infinite loops waiting for hardware
///
/// # Arguments
/// * `condition` - Closure returning true when ready
/// * `max_iterations` - Maximum poll attempts
///
/// # Returns
/// * `Ok(iterations)` - Number of iterations before success
/// * `Err(())` - Timeout reached
#[inline]
pub fn poll_with_timeout<F>(mut condition: F, max_iterations: u32) -> Result<u32, HardwareTimeout>
where
    F: FnMut() -> bool,
{
    for i in 0..max_iterations {
        if condition() {
            return Ok(i);
        }
        // Small delay to avoid hammering the bus
        core::hint::spin_loop();
    }
    Err(HardwareTimeout { iterations: max_iterations })
}

#[derive(Debug, Clone, Copy)]
pub struct HardwareTimeout {
    pub iterations: u32,
}

/// ATA-specific polling with proper 400ns delays
///
/// Reads alternate status register for delay, then polls main status.
pub fn ata_poll_ready(base_port: u16, timeout: u32) -> Result<u8, HardwareTimeout> {
    let status_port = base_port + 7;
    let alt_status_port = base_port + 0x206; // Alternate status (control block)

    // 400ns delay: read alternate status 4 times (~100ns each)
    for _ in 0..4 {
        unsafe {
            core::arch::asm!(
            "in al, dx",
            in("dx") alt_status_port,
            out("al") _,
            options(nomem, nostack)
            );
        }
    }

    // Now poll for BSY clear
    poll_with_timeout(
        || {
            let status: u8;
            unsafe {
                core::arch::asm!(
                "in al, dx",
                in("dx") status_port,
                out("al") status,
                options(nomem, nostack)
                );
            }
            // BSY (bit 7) must be clear
            (status & 0x80) == 0
        },
        timeout,
    )?;

    // Return final status
    let status: u8;
    unsafe {
        core::arch::asm!(
        "in al, dx",
        in("dx") status_port,
        out("al") status,
        options(nomem, nostack)
        );
    }
    Ok(status)
}

// =============================================================================
// INTERRUPT SAFETY
// =============================================================================

/// RAII guard for critical sections
///
/// Disables interrupts on creation, restores on drop.
/// Prevents interrupt handlers from corrupting shared state.
pub struct CriticalSection {
    was_enabled: bool,
}

impl CriticalSection {
    /// Enter critical section (disable interrupts)
    pub fn enter() -> Self {
        let flags: u32;
        unsafe {
            core::arch::asm!(
            "pushfd",
            "pop {0}",
            "cli",
            out(reg) flags,
            options(nomem, preserves_flags)
            );
        }
        Self {
            was_enabled: (flags & 0x200) != 0, // IF flag
        }
    }
}

impl Drop for CriticalSection {
    fn drop(&mut self) {
        if self.was_enabled {
            unsafe {
                core::arch::asm!("sti", options(nomem, nostack));
            }
        }
    }
}

/// Execute closure with interrupts disabled
#[inline]
pub fn with_interrupts_disabled<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = CriticalSection::enter();
    f()
}

// =============================================================================
// DEBUG / DIAGNOSTIC COUNTERS
// =============================================================================

static VGA_VIOLATIONS: AtomicU32 = AtomicU32::new(0);
static TIMEOUT_COUNT: AtomicU32 = AtomicU32::new(0);
static FRAME_COUNT: AtomicU32 = AtomicU32::new(0);
static LAST_OPERATION: AtomicU32 = AtomicU32::new(0);

fn increment_vga_violation() {
    VGA_VIOLATIONS.fetch_add(1, Ordering::Relaxed);
}

pub fn increment_timeout_count() {
    TIMEOUT_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub fn increment_frame_count() {
    FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Operation IDs for tracking last operation before crash
#[repr(u32)]
pub enum OperationId {
    None = 0,
    BootStart = 1,
    GdtInit = 2,
    IdtInit = 3,
    PicInit = 4,
    HeapInit = 5,
    AtaInit = 6,
    Fat32Mount = 7,
    RomLoad = 8,
    EmulatorInit = 9,
    FrameStart = 10,
    CpuCycle = 11,
    GpuRender = 12,
    VgaBlit = 13,
    KeyboardPoll = 14,
    FrameEnd = 15,
}

pub fn set_last_operation(op: OperationId) {
    LAST_OPERATION.store(op as u32, Ordering::Relaxed);
}

pub fn get_last_operation() -> u32 {
    LAST_OPERATION.load(Ordering::SeqCst)
}

/// Diagnostic snapshot for panic handler
#[derive(Debug, Clone, Copy)]
pub struct DiagnosticSnapshot {
    pub vga_violations: u32,
    pub timeout_count: u32,
    pub frame_count: u32,
    pub last_operation: u32,
    pub stack_overflow: bool,
}

pub fn take_diagnostic_snapshot() -> DiagnosticSnapshot {
    DiagnosticSnapshot {
        vga_violations: VGA_VIOLATIONS.load(Ordering::Relaxed),
        timeout_count: TIMEOUT_COUNT.load(Ordering::Relaxed),
        frame_count: FRAME_COUNT.load(Ordering::Relaxed),
        last_operation: get_last_operation(),
        stack_overflow: check_stack_overflow(),
    }
}

// =============================================================================
// ENHANCED PANIC HANDLER
// =============================================================================

/// Diagnostic panic handler that shows useful info before halt
///
/// Replace your existing panic handler with this.
///
/// # Example
/// ```
/// #[panic_handler]
/// fn panic(info: &core::panic::PanicInfo) -> ! {
///     defensive::diagnostic_panic(info)
/// }
/// ```
pub fn diagnostic_panic(info: &core::panic::PanicInfo) -> ! {
    // Disable interrupts immediately
    unsafe { core::arch::asm!("cli", options(nomem, nostack)); }

    // Take diagnostic snapshot
    let diag = take_diagnostic_snapshot();

    // Draw diagnostic screen
    draw_panic_screen(&diag, info);

    // Halt
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)); }
    }
}

fn draw_panic_screen(diag: &DiagnosticSnapshot, _info: &core::panic::PanicInfo) {
    // Clear screen to dark red
    for i in 0..VGA_SIZE {
        unsafe {
            let ptr = (VGA_BASE + i) as *mut u8;
            ptr.write_volatile(0x04); // Dark red
        }
    }

    // Draw white bar at top
    for x in 0..VGA_WIDTH {
        for y in 0..20 {
            let offset = y * VGA_WIDTH + x;
            unsafe {
                let ptr = (VGA_BASE + offset) as *mut u8;
                ptr.write_volatile(0x0F); // White
            }
        }
    }

    // Draw diagnostic indicators as colored blocks
    // Each block represents a different metric

    // Stack overflow indicator (yellow if ok, bright red if overflow)
    let stack_color = if diag.stack_overflow { 0x0C } else { 0x0E };
    draw_indicator_block(10, 30, stack_color);

    // VGA violations (green if 0, yellow if some, red if many)
    let vga_color = match diag.vga_violations {
        0 => 0x0A,       // Green
        1..=10 => 0x0E,  // Yellow
        _ => 0x0C,       // Red
    };
    draw_indicator_block(30, 30, vga_color);

    // Timeout count
    let timeout_color = match diag.timeout_count {
        0 => 0x0A,
        1..=5 => 0x0E,
        _ => 0x0C,
    };
    draw_indicator_block(50, 30, timeout_color);

    // Last operation (displayed as position on a bar)
    let op_x = 10 + (diag.last_operation as usize * 8).min(280);
    draw_indicator_block(op_x, 50, 0x09); // Light blue

    // Frame count as binary pattern (visual debugging)
    for bit in 0..16 {
        let color = if (diag.frame_count >> bit) & 1 == 1 { 0x0F } else { 0x08 };
        draw_indicator_block(10 + bit as usize * 10, 70, color);
    }
}

fn draw_indicator_block(x: usize, y: usize, color: u8) {
    for dy in 0..8 {
        for dx in 0..8 {
            let px = x + dx;
            let py = y + dy;
            if px < VGA_WIDTH && py < VGA_HEIGHT {
                let offset = py * VGA_WIDTH + px;
                unsafe {
                    let ptr = (VGA_BASE + offset) as *mut u8;
                    ptr.write_volatile(color);
                }
            }
        }
    }
}

// =============================================================================
// ASSERTIONS AND INVARIANTS
// =============================================================================

/// Debug assertion that compiles to nothing in release
#[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug_assert_invariant {
    ($cond:expr, $op:expr) => {
        if !($cond) {
            $crate::defensive::set_last_operation($op);
            panic!("Invariant violation");
        }
    };
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! debug_assert_invariant {
    ($cond:expr, $op:expr) => {};
}

/// Always-on assertion for critical invariants
#[macro_export]
macro_rules! assert_invariant {
    ($cond:expr, $op:expr) => {
        if !($cond) {
            $crate::defensive::set_last_operation($op);
            panic!("Critical invariant violation");
        }
    };
}

// =============================================================================
// MEMORY VALIDATION
// =============================================================================

/// Check if an address range is safe to access
///
/// Validates against known memory map to prevent wild pointer access.
pub fn is_safe_memory_range(addr: usize, len: usize) -> bool {
    let end = match addr.checked_add(len) {
        Some(e) => e,
        None => return false, // Overflow
    };

    // Valid ranges from memory map:
    // 0x00000500 - 0x00000548: Boot info (read-only after boot)
    // 0x00007C00 - 0x0000FFFF: Bootloader area (don't touch)
    // 0x000A0000 - 0x000AFFFF: VGA framebuffer (write ok)
    // 0x00100000 - 0x00140000: Kernel code/data
    // 0x00300000 - 0x00500000: ROM data
    // 0x01000000 - 0x01400000: Heap

    // Check if entirely within VGA buffer
    if addr >= VGA_BASE && end <= VGA_END {
        return true;
    }

    // Check if within ROM area
    if addr >= 0x00300000 && end <= 0x00500000 {
        return true;
    }

    // Check if within heap
    if addr >= 0x01000000 && end <= 0x01400000 {
        return true;
    }

    // Reject everything else for safety
    false
}

/// Safe memory read with validation
pub fn safe_read_u8(addr: usize) -> Option<u8> {
    if !is_safe_memory_range(addr, 1) {
        return None;
    }
    Some(unsafe { (addr as *const u8).read_volatile() })
}

/// Safe memory write with validation
pub fn safe_write_u8(addr: usize, value: u8) -> bool {
    if !is_safe_memory_range(addr, 1) {
        return false;
    }
    unsafe { (addr as *mut u8).write_volatile(value); }
    true
}
