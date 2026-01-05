//! AArch64 Entry Point
//!
//! Boot code that runs when the kernel is loaded by the GPU firmware.
//! Sets up the stack and jumps to Rust code.
//!
//! Memory layout at boot:
//! - Kernel loaded at 0x80000 by GPU firmware
//! - Stack at 0x100000 (grows downward)
//! - Heap starts after kernel BSS

use core::arch::global_asm;

// ============================================================================
// Boot Assembly
// ============================================================================

global_asm!(
    r#"
.section .text.boot
.global _start

_start:
    // Get core ID from MPIDR_EL1
    mrs     x0, mpidr_el1
    and     x0, x0, #0xFF

    // Park secondary cores (only core 0 continues)
    cbnz    x0, .Lpark

    // Core 0: Set up stack pointer
    // Stack at 0x100000, grows down (512KB below kernel load address)
    mov     x1, #0x0010
    lsl     x1, x1, #16        // x1 = 0x100000
    mov     sp, x1

    // Clear BSS section
    ldr     x0, =__bss_start
    ldr     x1, =__bss_end
.Lclear_bss:
    cmp     x0, x1
    b.ge    .Ldone_bss
    str     xzr, [x0], #8
    b       .Lclear_bss
.Ldone_bss:

    // Jump to Rust entry point
    mov     x0, #0             // core_id = 0
    bl      kernel_main

    // If kernel_main returns, halt
.Lhalt:
    wfe
    b       .Lhalt

// Secondary cores wait here forever
.Lpark:
    wfe
    b       .Lpark
"#
);

// ============================================================================
// Linker Symbols
// ============================================================================

extern "C" {
    /// Start of BSS section (defined in linker script).
    static __bss_start: u8;
    /// End of BSS section (defined in linker script).
    static __bss_end: u8;
    /// Start of kernel (0x80000).
    static __kernel_start: u8;
    /// End of kernel.
    static __kernel_end: u8;
}

/// Get the BSS section range.
pub fn bss_range() -> (*const u8, *const u8) {
    unsafe { (&__bss_start as *const u8, &__bss_end as *const u8) }
}

/// Get the kernel memory range.
pub fn kernel_range() -> (*const u8, *const u8) {
    unsafe { (&__kernel_start as *const u8, &__kernel_end as *const u8) }
}
