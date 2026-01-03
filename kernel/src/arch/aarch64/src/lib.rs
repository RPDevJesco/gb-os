//! AArch64 architecture support.
//!
//! Provides the entry point, exception vectors, and CPU utilities
//! for AArch64 platforms (Pi Zero 2 W, Pi 5, KickPi K2B).

#![no_std]

pub mod cpu;
pub mod exceptions;
pub mod mmu;

#[cfg(feature = "default-entry")]
use core::arch::global_asm;

// Entry point assembly.
// The Pi firmware loads kernel8.img at 0x80000 and jumps there.
// We set up the stack and call into Rust.
// Only included when "default-entry" feature is enabled.
#[cfg(feature = "default-entry")]
global_asm!(
    r#"
.section .text._start
.global _start

_start:
    // Park all cores except core 0
    mrs     x0, mpidr_el1
    and     x0, x0, #0xFF
    cbz     x0, .Lprimary_core
    
.Lpark:
    wfe
    b       .Lpark

.Lprimary_core:
    // Set up stack pointer
    // Stack grows down, place it below our code
    ldr     x0, =_stack_top
    mov     sp, x0
    
    // Clear BSS
    ldr     x0, =__bss_start
    ldr     x1, =__bss_end
.Lclear_bss:
    cmp     x0, x1
    b.ge    .Lbss_done
    str     xzr, [x0], #8
    b       .Lclear_bss
.Lbss_done:

    // Call Rust entry point
    bl      boot_main
    
    // If boot_main returns, halt
.Lhalt:
    wfe
    b       .Lhalt
"#
);

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
