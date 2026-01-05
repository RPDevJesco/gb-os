//! Memory-Mapped I/O (MMIO) Operations
//!
//! Low-level volatile read/write operations for hardware registers.
//! All BCM2710 peripheral access goes through these functions.

use core::ptr::{read_volatile, write_volatile};

/// BCM2710/BCM2837 peripheral base address (Pi Zero 2 W / Pi 3)
pub const PERIPHERAL_BASE: usize = 0x3F00_0000;

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

/// Read a 32-bit value with memory barrier.
#[inline(always)]
pub fn read_barrier(addr: usize) -> u32 {
    let value = read(addr);
    barrier();
    value
}

/// Write a 32-bit value with memory barrier.
#[inline(always)]
pub fn write_barrier(addr: usize, value: u32) {
    barrier();
    write(addr, value);
}

/// Data memory barrier - ensures all memory accesses complete.
#[inline(always)]
pub fn barrier() {
    unsafe {
        core::arch::asm!("dmb sy", options(nostack));
    }
}

/// Data synchronization barrier.
#[inline(always)]
pub fn dsb() {
    unsafe {
        core::arch::asm!("dsb sy", options(nostack));
    }
}

/// Instruction synchronization barrier.
#[inline(always)]
pub fn isb() {
    unsafe {
        core::arch::asm!("isb", options(nostack));
    }
}

/// Spin-loop hint for busy waiting.
#[inline(always)]
pub fn spin_hint() {
    core::hint::spin_loop();
}

/// Delay for approximately the given number of cycles.
#[inline]
pub fn delay_cycles(cycles: u32) {
    for _ in 0..cycles {
        spin_hint();
    }
}
