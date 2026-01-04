//! AArch64 architecture support.
//!
//! Provides CPU utilities for AArch64 platforms (Pi Zero 2 W, Pi 5, KickPi K2B).
//! Entry point assembly is in each platform's code.

#![no_std]

pub mod cpu;

/// Get the current exception level (0-3).
#[inline]
pub fn current_el() -> u8 {
    let el: u64;
    unsafe {
        core::arch::asm!(
            "mrs {}, CurrentEL",
            out(reg) el,
            options(nomem, nostack)
        );
    }
    ((el >> 2) & 0x3) as u8
}

/// Get the current core ID.
#[inline]
pub fn core_id() -> u8 {
    let mpidr: u64;
    unsafe {
        core::arch::asm!(
            "mrs {}, mpidr_el1",
            out(reg) mpidr,
            options(nomem, nostack)
        );
    }
    (mpidr & 0xFF) as u8
}

/// Memory barrier - ensure all previous memory accesses complete.
#[inline]
pub fn dmb() {
    unsafe {
        core::arch::asm!("dmb sy", options(nostack));
    }
}

/// Data synchronization barrier.
#[inline]
pub fn dsb() {
    unsafe {
        core::arch::asm!("dsb sy", options(nostack));
    }
}

/// Instruction synchronization barrier.
#[inline]
pub fn isb() {
    unsafe {
        core::arch::asm!("isb", options(nostack));
    }
}

/// No-op delay loop.
#[inline]
pub fn delay(cycles: u64) {
    for _ in 0..cycles {
        core::hint::spin_loop();
    }
}

/// Wait for interrupt (low power idle).
#[inline]
pub fn wfi() {
    unsafe {
        core::arch::asm!("wfi", options(nomem, nostack));
    }
}

/// Wait for event (low power idle).
#[inline]
pub fn wfe() {
    unsafe {
        core::arch::asm!("wfe", options(nomem, nostack));
    }
}
