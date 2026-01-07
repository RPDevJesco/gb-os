//! Multi-core support for Raspberry Pi Zero 2W
//!
//! This module provides:
//! - Core startup infrastructure
//! - Shared atomic state for inter-core communication
//! - Synchronization primitives
//!
//! Core allocation:
//! - Core 0: Main emulation loop (GameBoy CPU) + boot
//! - Core 1: USB HID polling (entry point defined in main.rs)
//! - Core 2: Graphics blitting (entry point defined in main.rs)
//! - Core 3: Reserved for audio / idle

use core::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, AtomicU8, AtomicBool, Ordering};
use core::ptr::write_volatile;

// ============================================================================
// Spin Table Addresses (where GPU firmware parks secondary cores)
//
// On Pi 3/Zero 2W with armstub8.bin:
// - Core 0: 0xD8 (unused, core 0 runs immediately)
// - Core 1: 0xE0
// - Core 2: 0xE8
// - Core 3: 0xF0
//
// The firmware spins reading these addresses, waiting for non-zero value,
// then jumps to that address.
// ============================================================================

const CORE1_SPIN_ADDR: usize = 0xE0;
const CORE2_SPIN_ADDR: usize = 0xE8;
const CORE3_SPIN_ADDR: usize = 0xF0;

// ============================================================================
// Shared Button State (Core 1 writes, Core 0 reads)
// ============================================================================

/// Current button state - written by Core 1, read by Core 0
pub static SHARED_BUTTONS: AtomicU16 = AtomicU16::new(0);

/// Previous button state - for edge detection
pub static SHARED_BUTTONS_PREV: AtomicU16 = AtomicU16::new(0);

/// Flag indicating Core 1 has taken USB ownership
pub static USB_OWNED_BY_CORE1: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Shared Graphics State (Core 0 signals, Core 2 blits)
// ============================================================================

/// Signal from Core 0 to Core 2: "new frame ready to blit"
pub static GFX_FRAME_READY: AtomicBool = AtomicBool::new(false);

/// Signal from Core 2 to Core 0: "blit complete"
pub static GFX_BLIT_DONE: AtomicBool = AtomicBool::new(true);

/// Framebuffer address (set by Core 0, read by Core 2)
pub static FB_ADDR: AtomicU32 = AtomicU32::new(0);
pub static FB_PITCH: AtomicU32 = AtomicU32::new(0);

/// GB screen data pointer (set by Core 0 before signaling Core 2)
pub static GB_SCREEN_PTR: AtomicU32 = AtomicU32::new(0);
pub static GB_SCREEN_IS_COLOR: AtomicBool = AtomicBool::new(true);

// ============================================================================
// Core Status Flags
// ============================================================================

pub static CORE1_RUNNING: AtomicBool = AtomicBool::new(false);
pub static CORE2_RUNNING: AtomicBool = AtomicBool::new(false);
pub static CORE3_RUNNING: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Memory Barriers
// ============================================================================

#[inline(always)]
pub fn dmb() {
    unsafe { core::arch::asm!("dmb sy"); }
}

#[inline(always)]
pub fn dsb() {
    unsafe { core::arch::asm!("dsb sy"); }
}

#[inline(always)]
pub fn sev() {
    unsafe { core::arch::asm!("sev"); }
}

#[inline(always)]
pub fn wfe() {
    unsafe { core::arch::asm!("wfe"); }
}

// ============================================================================
// Core Startup
// ============================================================================

// ============================================================================
// Core Entry Trampolines (set up stack before jumping to Rust code)
//
// CRITICAL: Secondary cores have NO STACK when firmware wakes them!
// We must set up the stack in assembly before calling any Rust code.
// ============================================================================

// External symbols for stack tops (defined in linker script)
extern "C" {
    static __core1_stack_top: u8;
    static __core2_stack_top: u8;
    static __core3_stack_top: u8;
}

// Trampoline entry points - these are what we write to the spin table
core::arch::global_asm!(
    r#"
.section .text
.global core1_trampoline
.global core2_trampoline
.global core3_trampoline

// Core 1 trampoline: set up stack, then call Rust entry
core1_trampoline:
    ldr     x1, =__core1_stack_top
    mov     sp, x1
    b       core1_rust_entry

// Core 2 trampoline: set up stack, then call Rust entry
core2_trampoline:
    ldr     x1, =__core2_stack_top
    mov     sp, x1
    b       core2_rust_entry

// Core 3 trampoline: set up stack, then call Rust entry
core3_trampoline:
    ldr     x1, =__core3_stack_top
    mov     sp, x1
    b       core3_rust_entry
"#
);

// Declare the trampoline symbols
extern "C" {
    fn core1_trampoline() -> !;
    fn core2_trampoline() -> !;
    fn core3_trampoline() -> !;
}

// These are set by main.rs before starting cores
pub static CORE1_ENTRY: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub static CORE2_ENTRY: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub static CORE3_ENTRY: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Core 1 Rust entry - called from trampoline after stack is set up
#[no_mangle]
pub unsafe extern "C" fn core1_rust_entry() -> ! {
    // Signal we're running (stack is now set up!)
    CORE1_RUNNING.store(true, core::sync::atomic::Ordering::Release);

    // Get the actual entry function
    let entry_addr = CORE1_ENTRY.load(core::sync::atomic::Ordering::Acquire);
    if entry_addr != 0 {
        let entry_fn: unsafe extern "C" fn() -> ! = core::mem::transmute(entry_addr);
        entry_fn()
    } else {
        // No entry set, just spin
        loop { wfe(); }
    }
}

/// Core 2 Rust entry - called from trampoline after stack is set up
#[no_mangle]
pub unsafe extern "C" fn core2_rust_entry() -> ! {
    CORE2_RUNNING.store(true, core::sync::atomic::Ordering::Release);

    let entry_addr = CORE2_ENTRY.load(core::sync::atomic::Ordering::Acquire);
    if entry_addr != 0 {
        let entry_fn: unsafe extern "C" fn() -> ! = core::mem::transmute(entry_addr);
        entry_fn()
    } else {
        loop { wfe(); }
    }
}

/// Core 3 Rust entry - called from trampoline after stack is set up
#[no_mangle]
pub unsafe extern "C" fn core3_rust_entry() -> ! {
    CORE3_RUNNING.store(true, core::sync::atomic::Ordering::Release);

    let entry_addr = CORE3_ENTRY.load(core::sync::atomic::Ordering::Acquire);
    if entry_addr != 0 {
        let entry_fn: unsafe extern "C" fn() -> ! = core::mem::transmute(entry_addr);
        entry_fn()
    } else {
        loop { wfe(); }
    }
}

/// Start a secondary core with a given entry point
///
/// This writes the trampoline address to the spin table. The trampoline
/// sets up the stack, then calls the Rust entry which calls your function.
pub unsafe fn start_core(core_id: u8, entry: unsafe extern "C" fn() -> !) {
    let (spin_addr, trampoline, entry_storage) = match core_id {
        1 => (CORE1_SPIN_ADDR, core1_trampoline as u64, &CORE1_ENTRY),
        2 => (CORE2_SPIN_ADDR, core2_trampoline as u64, &CORE2_ENTRY),
        3 => (CORE3_SPIN_ADDR, core3_trampoline as u64, &CORE3_ENTRY),
        _ => return,
    };

    // Store the actual entry function for the Rust entry to call
    entry_storage.store(entry as u64, core::sync::atomic::Ordering::Release);
    dsb();

    // Write the TRAMPOLINE address (not the entry directly) to spin table
    write_volatile(spin_addr as *mut u64, trampoline);

    // Memory barrier
    core::arch::asm!("dmb sy");

    // Clean cache line
    core::arch::asm!(
    "dc civac, {addr}",
    addr = in(reg) spin_addr,
    );
    core::arch::asm!("dsb sy");

    // Wake cores
    core::arch::asm!("sev");

    for _ in 0..1000 { core::hint::spin_loop(); }
    core::arch::asm!("sev");
}

/// Read the current value at a spin table address (for debugging)
pub fn read_spin_table(core_id: u8) -> u64 {
    let spin_addr = match core_id {
        1 => CORE1_SPIN_ADDR,
        2 => CORE2_SPIN_ADDR,
        3 => CORE3_SPIN_ADDR,
        _ => return 0,
    };

    unsafe {
        // Invalidate cache line first to get fresh value from RAM
        core::arch::asm!(
        "dc ivac, {addr}",
        "dsb sy",
        addr = in(reg) spin_addr,
        );
        core::ptr::read_volatile(spin_addr as *const u64)
    }
}

// ============================================================================
// Button State API (for Core 0 to read)
// ============================================================================

/// Update shared button state (called by Core 1)
#[inline(always)]
pub fn set_buttons(current: u16) {
    let prev = SHARED_BUTTONS.load(Ordering::Relaxed);
    SHARED_BUTTONS_PREV.store(prev, Ordering::Relaxed);
    SHARED_BUTTONS.store(current, Ordering::Release);
}

/// Read current buttons (called by Core 0)
#[inline(always)]
pub fn get_buttons() -> u16 {
    SHARED_BUTTONS.load(Ordering::Acquire)
}

/// Read previous buttons (called by Core 0)
#[inline(always)]
pub fn get_buttons_prev() -> u16 {
    SHARED_BUTTONS_PREV.load(Ordering::Acquire)
}

/// Check if button was just pressed this frame
#[inline(always)]
pub fn button_just_pressed(button: u16) -> bool {
    let current = SHARED_BUTTONS.load(Ordering::Acquire);
    let previous = SHARED_BUTTONS_PREV.load(Ordering::Acquire);
    (current & button) != 0 && (previous & button) == 0
}

/// Check if button was just released this frame
#[inline(always)]
pub fn button_just_released(button: u16) -> bool {
    let current = SHARED_BUTTONS.load(Ordering::Acquire);
    let previous = SHARED_BUTTONS_PREV.load(Ordering::Acquire);
    (current & button) == 0 && (previous & button) != 0
}

/// Check if button is currently held
#[inline(always)]
pub fn button_pressed(button: u16) -> bool {
    (SHARED_BUTTONS.load(Ordering::Acquire) & button) != 0
}

// ============================================================================
// Graphics API (for Core 0 to signal Core 2)
// ============================================================================

/// Initialize framebuffer info for Core 2
pub fn init_gfx_core(fb_addr: u32, fb_pitch: u32) {
    FB_ADDR.store(fb_addr, Ordering::Release);
    FB_PITCH.store(fb_pitch, Ordering::Release);
    dsb();
}

/// Signal Core 2 to blit a frame (non-blocking)
/// Returns true if Core 2 accepted the work, false if still busy
pub fn request_blit(screen_ptr: *const u8, is_color: bool) -> bool {
    // Check if Core 2 is done with previous frame
    if !GFX_BLIT_DONE.load(Ordering::Acquire) {
        return false; // Still busy, skip this frame
    }

    // Set up the blit parameters
    GB_SCREEN_PTR.store(screen_ptr as u32, Ordering::Release);
    GB_SCREEN_IS_COLOR.store(is_color, Ordering::Release);

    // Signal Core 2
    GFX_BLIT_DONE.store(false, Ordering::Release);
    GFX_FRAME_READY.store(true, Ordering::Release);
    dsb();
    sev(); // Wake Core 2

    true
}

/// Wait for Core 2 to finish blitting (blocking)
pub fn wait_blit_done() {
    while !GFX_BLIT_DONE.load(Ordering::Acquire) {
        wfe();
    }
}

// ============================================================================
// Graphics Constants (needed by Core 2 blit functions)
// ============================================================================

pub const GB_WIDTH: usize = 160;
pub const GB_HEIGHT: usize = 144;
pub const GB_SCALE: usize = 2;
pub const SCREEN_WIDTH: usize = 640;
pub const SCREEN_HEIGHT: usize = 480;
pub const GB_OFFSET_X: usize = (SCREEN_WIDTH - GB_WIDTH * GB_SCALE) / 2;
pub const GB_OFFSET_Y: usize = (SCREEN_HEIGHT - GB_HEIGHT * GB_SCALE) / 2;

pub const GB_PALETTE: [u32; 4] = [
    0xFFE0F8D0,
    0xFF88C070,
    0xFF346856,
    0xFF081820,
];
