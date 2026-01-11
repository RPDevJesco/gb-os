//! Low-level MMIO, timing, and memory barrier primitives
//!
//! This module provides the foundational memory-mapped I/O operations
//! and timing utilities needed by all hardware drivers.

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// Hardware Base Addresses
// ============================================================================

/// BCM2837 peripheral base address (Pi Zero 2W / Pi 3)
pub const PERIPHERAL_BASE: usize = 0x3F00_0000;

/// ARM Local peripherals base (for multicore)
pub const ARM_LOCAL_BASE: usize = 0x4000_0000;

// ============================================================================
// System Timer Registers
// ============================================================================

const SYSTIMER_BASE: usize = PERIPHERAL_BASE + 0x0000_3000;

/// System timer counter low 32 bits (1MHz)
const SYSTIMER_CLO: usize = SYSTIMER_BASE + 0x04;

// ============================================================================
// MMIO Access Functions
// ============================================================================

/// Read a 32-bit value from an MMIO address
#[inline(always)]
pub fn mmio_read(addr: usize) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}

/// Write a 32-bit value to an MMIO address
#[inline(always)]
pub fn mmio_write(addr: usize, val: u32) {
    unsafe { write_volatile(addr as *mut u32, val) }
}

// ============================================================================
// Memory Barriers
// ============================================================================

/// Data Memory Barrier - ensures all data accesses before this complete
/// before any data accesses after it
#[inline(always)]
pub fn dmb() {
    unsafe { core::arch::asm!("dmb sy"); }
}

/// Data Synchronization Barrier - ensures all memory accesses complete
/// before the next instruction executes
#[inline(always)]
pub fn dsb() {
    unsafe { core::arch::asm!("dsb sy"); }
}

/// Instruction Synchronization Barrier - flushes the pipeline
#[inline(always)]
pub fn isb() {
    unsafe { core::arch::asm!("isb"); }
}

/// Send Event - wakes cores waiting in WFE
#[inline(always)]
pub fn sev() {
    unsafe { core::arch::asm!("sev"); }
}

/// Wait For Event - low-power wait until event
#[inline(always)]
pub fn wfe() {
    unsafe { core::arch::asm!("wfe"); }
}

// ============================================================================
// Timing Functions
// ============================================================================

/// Get current system timer value in microseconds
#[inline]
pub fn micros() -> u32 {
    mmio_read(SYSTIMER_CLO)
}

/// Delay for specified number of microseconds
pub fn delay_us(us: u32) {
    let start = micros();
    while micros().wrapping_sub(start) < us {
        core::hint::spin_loop();
    }
}

/// Delay for specified number of milliseconds
pub fn delay_ms(ms: u32) {
    delay_us(ms * 1000);
}

// ============================================================================
// Cache Line Operations
// ============================================================================

/// ARM Cortex-A53 cache line size
pub const CACHE_LINE_SIZE: usize = 64;

/// Clean D-cache for a memory range (flush dirty lines to RAM)
/// This ensures the GPU can see data written by the CPU
///
/// # Safety
/// The caller must ensure the address range is valid
#[inline(never)]
pub unsafe fn clean_dcache_range(start: usize, size: usize) {
    let mut addr = start & !(CACHE_LINE_SIZE - 1); // Align to cache line
    let end = start + size;

    while addr < end {
        // DC CVAC - Clean by VA to Point of Coherency
        core::arch::asm!("dc cvac, {}", in(reg) addr);
        addr += CACHE_LINE_SIZE;
    }

    // Data synchronization barrier
    core::arch::asm!("dsb sy");
}

/// Invalidate D-cache for a memory range
///
/// # Safety
/// The caller must ensure the address range is valid and no dirty
/// data will be lost
#[inline(never)]
pub unsafe fn invalidate_dcache_range(start: usize, size: usize) {
    let mut addr = start & !(CACHE_LINE_SIZE - 1);
    let end = start + size;

    while addr < end {
        // DC IVAC - Invalidate by VA to Point of Coherency
        core::arch::asm!("dc ivac, {}", in(reg) addr);
        addr += CACHE_LINE_SIZE;
    }

    core::arch::asm!("dsb sy");
}

/// Clean and invalidate D-cache for a memory range
///
/// # Safety
/// The caller must ensure the address range is valid
#[inline(never)]
pub unsafe fn flush_dcache_range(start: usize, size: usize) {
    let mut addr = start & !(CACHE_LINE_SIZE - 1);
    let end = start + size;

    while addr < end {
        // DC CIVAC - Clean and Invalidate by VA to Point of Coherency
        core::arch::asm!("dc civac, {}", in(reg) addr);
        addr += CACHE_LINE_SIZE;
    }

    core::arch::asm!("dsb sy");
}
