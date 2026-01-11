/* =============================================================================
 * linker.ld - Raspberry Pi Zero 2W Kernel Linker Script
 * =============================================================================
 *
 * Memory Map for BCM2710 (RPi Zero 2W):
 *
 *   0x00000000 - 0x00080000  : Reserved (GPU, VideoCore)
 *   0x00080000 - ...         : Kernel load address
 *   ...        - 0x3B400000  : Available RAM (depends on gpu_mem split)
 *   0x3F000000 - 0x40000000  : Peripheral registers (memory-mapped I/O)
 *   0x40000000 - 0x40FFFFFF  : Local peripherals (ARM timer, mailbox, etc.)
 *
 * With default gpu_mem=64, we have ~448MB of RAM available.
 * The kernel is loaded at 0x80000 by the RPi bootloader.
 *
 * =============================================================================
 */

/* Entry point - must match _start in boot.S */
ENTRY(_start)

/* Kernel load address */
__kernel_load_addr = 0x80000;

/* Stack size per core (64KB each) */
__stack_size = 0x10000;

/* Total RAM available (conservative: 256MB, adjust based on gpu_mem) */
__ram_size = 256M;

MEMORY
{
    /* Main RAM region for kernel */
    RAM (rwx) : ORIGIN = 0x80000, LENGTH = __ram_size - 0x80000
}

SECTIONS
{
    /* =========================================================================
     * Code Section
     * =========================================================================
     * Boot code must come first at 0x80000
     */
    .text __kernel_load_addr :
    {
        __text_start = .;
        KEEP(*(.text.boot))         /* Boot stub - must be first! */
        KEEP(*(.text.vectors))      /* Exception vector table */
        *(.text .text.*)            /* All other code */
        __text_end = .;
    } > RAM

    /* =========================================================================
     * Read-Only Data
     * =========================================================================
     */
    .rodata : ALIGN(4K)
    {
        __rodata_start = .;
        *(.rodata .rodata.*)
        __rodata_end = .;
    } > RAM

    /* =========================================================================
     * Initialized Data
     * =========================================================================
     */
    .data : ALIGN(4K)
    {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    } > RAM

    /* =========================================================================
     * Uninitialized Data (BSS)
     * =========================================================================
     * Must be zeroed by boot code before entering Rust
     */
    .bss : ALIGN(4K)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    /* =========================================================================
     * Stack Region
     * =========================================================================
     * Stack grows downward from _stack_top
     * We allocate 64KB for the main stack
     */
    .stack (NOLOAD) : ALIGN(16)
    {
        __stack_bottom = .;
        . += __stack_size;
        _stack_top = .;
    } > RAM

    /* =========================================================================
     * Heap Region
     * =========================================================================
     * Everything after the stack until end of RAM is available for heap
     */
    .heap (NOLOAD) : ALIGN(4K)
    {
        __heap_start = .;
        /* Heap extends to end of RAM */
    } > RAM

    /* Calculate end of RAM */
    __heap_end = ORIGIN(RAM) + LENGTH(RAM);

    /* =========================================================================
     * Debug Sections (stripped in release builds)
     * =========================================================================
     */
    .debug_info     0 : { *(.debug_info) }
    .debug_abbrev   0 : { *(.debug_abbrev) }
    .debug_line     0 : { *(.debug_line) }
    .debug_frame    0 : { *(.debug_frame) }
    .debug_str      0 : { *(.debug_str) }
    .debug_ranges   0 : { *(.debug_ranges) }
    .debug_loc      0 : { *(.debug_loc) }

    /* =========================================================================
     * Discarded Sections
     * =========================================================================
     */
    /DISCARD/ :
    {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
        *(.ARM.*)
    }
}

/* =============================================================================
 * Sanity Checks
 * =============================================================================
 */

/* Ensure boot code is at the correct address */
ASSERT(__text_start == __kernel_load_addr, "Boot code must be at 0x80000")

/* Ensure BSS section is properly aligned for 64-bit zeroing */
ASSERT((__bss_start & 7) == 0, "BSS start must be 8-byte aligned")
ASSERT((__bss_end & 7) == 0, "BSS end must be 8-byte aligned")

/* Ensure stack is 16-byte aligned (required by AArch64 ABI) */
ASSERT((_stack_top & 15) == 0, "Stack must be 16-byte aligned")
