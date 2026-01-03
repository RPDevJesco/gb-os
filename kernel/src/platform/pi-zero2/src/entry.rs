//! AArch64 Entry Point for Pi Zero 2 W
//!
//! Memory Layout (32MB GPU / 10MB Kernel / 470MB Heap):
//!   - Kernel loads at 0x0008_0000
//!   - Stacks at 0x0028_0000 (64KB per core)
//!   - Heap at 0x00A8_0000 (470MB)

#![allow(dead_code)]

use core::arch::global_asm;

global_asm!(
    r#"
.section .text._start
.global _start

_start:
    // Get core ID
    mrs     x0, mpidr_el1
    and     x0, x0, #0xFF

    // Park secondary cores
    cbnz    x0, .Lpark

    // Core 0: Set up stack (stack_top_core0 = 0x0029_0000)
    ldr     x1, =_stack_top_core0
    mov     sp, x1

    // Clear BSS
    ldr     x0, =__bss_start
    ldr     x1, =__bss_end
.Lclear_bss:
    cmp     x0, x1
    b.ge    .Ldone_bss
    str     xzr, [x0], #8
    b       .Lclear_bss
.Ldone_bss:

    // Call Rust
    mov     x0, #0          // core_id = 0
    bl      boot_main

    // Halt if boot_main returns
.Lhalt:
    wfe
    b       .Lhalt

// Secondary cores wait here
.Lpark:
    wfe
    b       .Lpark
"#
);

// Linker symbols - Rust 2024 requires `unsafe` on extern blocks
unsafe extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
    static __bss_start: u8;
    static __bss_end: u8;
    static _stack_top_core0: u8;
    static _stack_top_core1: u8;
    static _stack_top_core2: u8;
    static _stack_top_core3: u8;
    static __dma_start: u8;
    static __dma_end: u8;
    static _mailbox_buffer: u8;
    static _emmc_buffer: u8;
    static __pgtbl_start: u8;
    static _ttbr0: u8;
    static _heap_start: u8;
    static _heap_end: u8;
}

/// Get mailbox buffer address (16-byte aligned, DMA-safe)
pub fn mailbox_buffer() -> usize {
    unsafe { &_mailbox_buffer as *const u8 as usize }
}

/// Get EMMC buffer address
pub fn emmc_buffer() -> usize {
    unsafe { &_emmc_buffer as *const u8 as usize }
}

/// Get heap bounds
pub fn heap_bounds() -> (usize, usize) {
    unsafe {
        (
            &_heap_start as *const u8 as usize,
            &_heap_end as *const u8 as usize,
        )
    }
}

/// Get page table base (TTBR0)
pub fn ttbr0_addr() -> usize {
    unsafe { &_ttbr0 as *const u8 as usize }
}

/// Get stack top for a core
pub fn stack_top(core: usize) -> usize {
    unsafe {
        match core {
            0 => &_stack_top_core0 as *const u8 as usize,
            1 => &_stack_top_core1 as *const u8 as usize,
            2 => &_stack_top_core2 as *const u8 as usize,
            3 => &_stack_top_core3 as *const u8 as usize,
            _ => 0,
        }
    }
}
