//! ARMv6-M architecture support.
//!
//! Provides the vector table, entry point, and CPU utilities
//! for Cortex-M0+ platforms (RP2040).

#![no_std]

use core::arch::global_asm;

// Vector table and reset handler.
// The RP2040 boot ROM loads the stage2 bootloader from flash,
// which then sets up flash XIP and jumps to the main vector table.
global_asm!(
    r#"
.section .vector_table, "ax"
.global __vector_table
.align 2

__vector_table:
    .word   _stack_top          // Initial stack pointer
    .word   _reset_handler      // Reset handler
    .word   _nmi_handler        // NMI handler
    .word   _hardfault_handler  // HardFault handler
    .word   0                   // Reserved
    .word   0                   // Reserved
    .word   0                   // Reserved
    .word   0                   // Reserved
    .word   0                   // Reserved
    .word   0                   // Reserved
    .word   0                   // Reserved
    .word   _svc_handler        // SVCall handler
    .word   0                   // Reserved
    .word   0                   // Reserved
    .word   _pendsv_handler     // PendSV handler
    .word   _systick_handler    // SysTick handler

    // External interrupts (IRQ 0-31)
    .rept 32
    .word   _default_handler
    .endr

.section .text._reset_handler
.global _reset_handler
.type _reset_handler, %function
.thumb_func

_reset_handler:
    // Set stack pointer (redundant but safe)
    ldr     r0, =_stack_top
    mov     sp, r0

    // Copy .data from flash to RAM
    ldr     r0, =__data_start
    ldr     r1, =__data_end
    ldr     r2, =__data_load
    b       .Ldata_check
.Ldata_copy:
    ldm     r2!, {{r3}}
    stm     r0!, {{r3}}
.Ldata_check:
    cmp     r0, r1
    blt     .Ldata_copy

    // Clear .bss
    ldr     r0, =__bss_start
    ldr     r1, =__bss_end
    movs    r2, #0
    b       .Lbss_check
.Lbss_clear:
    stm     r0!, {{r2}}
.Lbss_check:
    cmp     r0, r1
    blt     .Lbss_clear

    // Call Rust entry
    bl      boot_main

    // Halt if boot_main returns
.Lhalt:
    wfi
    b       .Lhalt

// Default exception handlers
.section .text.handlers
.weak _nmi_handler
.weak _hardfault_handler
.weak _svc_handler
.weak _pendsv_handler
.weak _systick_handler
.weak _default_handler

.thumb_func
_nmi_handler:
.thumb_func
_hardfault_handler:
.thumb_func
_svc_handler:
.thumb_func
_pendsv_handler:
.thumb_func
_systick_handler:
.thumb_func
_default_handler:
    b       .
"#
);

/// Disable interrupts and return previous state.
#[inline]
pub fn disable_interrupts() -> u32 {
    let primask: u32;
    unsafe {
        core::arch::asm!(
            "mrs {}, PRIMASK",
            "cpsid i",
            out(reg) primask,
            options(nomem, nostack)
        );
    }
    primask
}

/// Enable interrupts.
#[inline]
pub fn enable_interrupts() {
    unsafe {
        core::arch::asm!(
            "cpsie i",
            options(nomem, nostack)
        );
    }
}

/// Restore interrupt state.
#[inline]
pub fn restore_interrupts(primask: u32) {
    unsafe {
        core::arch::asm!(
            "msr PRIMASK, {}",
            in(reg) primask,
            options(nomem, nostack)
        );
    }
}

/// Wait for interrupt.
#[inline]
pub fn wfi() {
    unsafe {
        core::arch::asm!("wfi", options(nomem, nostack));
    }
}

/// Wait for event.
#[inline]
pub fn wfe() {
    unsafe {
        core::arch::asm!("wfe", options(nomem, nostack));
    }
}

/// Send event.
#[inline]
pub fn sev() {
    unsafe {
        core::arch::asm!("sev", options(nomem, nostack));
    }
}

/// No operation.
#[inline]
pub fn nop() {
    unsafe {
        core::arch::asm!("nop", options(nomem, nostack));
    }
}

/// Data memory barrier.
#[inline]
pub fn dmb() {
    unsafe {
        core::arch::asm!("dmb", options(nostack));
    }
}

/// Data synchronization barrier.
#[inline]
pub fn dsb() {
    unsafe {
        core::arch::asm!("dsb", options(nostack));
    }
}

/// Instruction synchronization barrier.
#[inline]
pub fn isb() {
    unsafe {
        core::arch::asm!("isb", options(nostack));
    }
}

/// Delay loop.
#[inline]
pub fn delay(cycles: u32) {
    // Each iteration is roughly 3 cycles
    let mut count = cycles;
    unsafe {
        core::arch::asm!(
            "1:",
            "subs {0}, {0}, #1",
            "bne 1b",
            inout(reg) count,
            options(nomem, nostack)
        );
    }
    let _ = count; // Silence unused warning
}

/// Get the current core ID (0 or 1 on RP2040).
#[inline]
pub fn core_id() -> u32 {
    // SIO CPUID register
    const SIO_CPUID: *const u32 = 0xD000_0000 as *const u32;
    unsafe { core::ptr::read_volatile(SIO_CPUID) }
}
