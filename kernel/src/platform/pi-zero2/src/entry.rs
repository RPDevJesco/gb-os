//! AArch64 Entry Point for Pi Zero 2W
//!
//! Memory Layout:
//!   - Kernel loads at 0x0008_0000
//!   - Stack at 0x0010_0000 (256KB below kernel)
//!   - Heap at 0x0100_0000 (16MB mark, 256MB size)

use core::arch::global_asm;

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
    // Stack grows down from 0x0010_0000 (gives 512KB stack space)
    mov     x1, #0x0010
    lsl     x1, x1, #16        // x1 = 0x0010_0000
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
    bl      boot_main

    // If boot_main returns, halt
.Lhalt:
    wfe
    b       .Lhalt

// Secondary cores wait here forever
.Lpark:
    wfe
    b       .Lpark
"#
);

// Linker-provided symbols
extern "C" {
    static __bss_start: u8;
    static __bss_end: u8;
}
