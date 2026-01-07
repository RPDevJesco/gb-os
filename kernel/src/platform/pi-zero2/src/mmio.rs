//! Memory-Mapped I/O (MMIO) Operations
//!
//! Low-level volatile read/write operations for hardware registers.
//! All BCM2710 peripheral access goes through these functions.
//!
//! # Memory Barriers
//!
//! ARM requires explicit memory barriers for correct peripheral access:
//! - `dmb()` - Data Memory Barrier: ensures ordering of memory accesses
//! - `dsb()` - Data Synchronization Barrier: ensures completion of memory accesses
//! - `isb()` - Instruction Synchronization Barrier: flushes pipeline

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// Base Addresses
// ============================================================================

/// BCM2710/BCM2837 peripheral base address (Pi Zero 2 W / Pi 3)
pub const PERIPHERAL_BASE: usize = 0x3F00_0000;

/// BCM2711 peripheral base address (Pi 4) - for future compatibility
pub const PERIPHERAL_BASE_PI4: usize = 0xFE00_0000;

// ============================================================================
// Basic MMIO Operations
// ============================================================================

/// Read a 32-bit value from a memory-mapped register.
///
/// # Safety
/// The address must be a valid MMIO register address.
#[inline(always)]
pub fn read(addr: usize) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}

/// Write a 32-bit value to a memory-mapped register.
///
/// # Safety
/// The address must be a valid MMIO register address.
#[inline(always)]
pub fn write(addr: usize, value: u32) {
    unsafe { write_volatile(addr as *mut u32, value) }
}

/// Read-modify-write operation with mask.
///
/// Clears bits specified by `mask`, then sets bits from `value`.
/// Equivalent to: `reg = (reg & !mask) | (value & mask)`
#[inline]
pub fn modify(addr: usize, mask: u32, value: u32) {
    let current = read(addr);
    write(addr, (current & !mask) | (value & mask));
}

/// Set specific bits in a register.
#[inline]
pub fn set_bits(addr: usize, bits: u32) {
    write(addr, read(addr) | bits);
}

/// Clear specific bits in a register.
#[inline]
pub fn clear_bits(addr: usize, bits: u32) {
    write(addr, read(addr) & !bits);
}

// ============================================================================
// Memory Barriers
// ============================================================================

/// Data Memory Barrier - ensures all memory accesses before the barrier
/// are observed before any memory accesses after the barrier.
///
/// Use between writes to different peripherals or when write ordering matters.
#[inline(always)]
pub fn dmb() {
    unsafe {
        core::arch::asm!("dmb sy", options(nostack, preserves_flags));
    }
}

/// Data Synchronization Barrier - ensures all memory accesses complete
/// before continuing execution.
///
/// Stronger than DMB. Use when you need to ensure a write has actually
/// completed (e.g., before disabling a peripheral).
#[inline(always)]
pub fn dsb() {
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

/// Instruction Synchronization Barrier - flushes the instruction pipeline.
///
/// Use after modifying code, page tables, or system registers.
#[inline(always)]
pub fn isb() {
    unsafe {
        core::arch::asm!("isb", options(nostack, preserves_flags));
    }
}

/// Full memory barrier (alias for dmb for compatibility).
#[inline(always)]
pub fn barrier() {
    dmb();
}

// ============================================================================
// Barrier-Protected Operations
// ============================================================================

/// Read with barrier after - ensures read completes before subsequent accesses.
#[inline(always)]
pub fn read_barrier(addr: usize) -> u32 {
    let value = read(addr);
    dmb();
    value
}

/// Write with barrier before - ensures previous accesses complete before write.
#[inline(always)]
pub fn write_barrier(addr: usize, value: u32) {
    dmb();
    write(addr, value);
}

/// Write with barriers before and after - for critical register updates.
#[inline(always)]
pub fn write_sync(addr: usize, value: u32) {
    dmb();
    write(addr, value);
    dsb();
}

// ============================================================================
// Spinning and Delays
// ============================================================================

/// Spin-loop hint for busy waiting.
///
/// Tells the CPU we're in a spin loop, allowing power savings
/// and better performance on simultaneous multithreading.
#[inline(always)]
pub fn spin_hint() {
    core::hint::spin_loop();
}

/// Delay for approximately the given number of CPU cycles.
///
/// This is a rough delay used for GPIO timing sequences that require
/// specific cycle counts. Not suitable for precise timing.
#[inline]
pub fn delay_cycles(cycles: u32) {
    for _ in 0..cycles {
        spin_hint();
    }
}

/// Spin until a condition is met or timeout cycles elapse.
///
/// Returns `true` if condition was met, `false` on timeout.
#[inline]
pub fn spin_until<F>(mut condition: F, timeout_cycles: u32) -> bool
where
    F: FnMut() -> bool,
{
    for _ in 0..timeout_cycles {
        if condition() {
            return true;
        }
        spin_hint();
    }
    false
}

/// Spin while reading a register until masked bits match expected value.
///
/// Returns `true` if matched, `false` on timeout.
#[inline]
pub fn wait_bits(addr: usize, mask: u32, expected: u32, timeout_cycles: u32) -> bool {
    spin_until(|| (read(addr) & mask) == expected, timeout_cycles)
}

/// Spin while reading a register until specific bits are set.
#[inline]
pub fn wait_set(addr: usize, bits: u32, timeout_cycles: u32) -> bool {
    wait_bits(addr, bits, bits, timeout_cycles)
}

/// Spin while reading a register until specific bits are clear.
#[inline]
pub fn wait_clear(addr: usize, bits: u32, timeout_cycles: u32) -> bool {
    wait_bits(addr, bits, 0, timeout_cycles)
}
