/*
 * Linker Script for GB-OS on Raspberry Pi Zero 2 W
 *
 * Memory Layout (512MB RAM, 32MB reserved for GPU):
 *
 *   0x00000000 - 0x0007FFFF  Reserved for GPU/firmware (512KB)
 *   0x00080000 - ~0x001FFFFF Kernel code, rodata, data, bss
 *   0x00200000 - 0x002FFFFF  Stack region (1MB total)
 *                            - Core 0 stack: 256KB
 *                            - Core 1-3 stacks: 64KB each (for future multi-core)
 *                            - IRQ stack: 64KB
 *                            - Abort stack: 64KB
 *   0x00300000 - 0x1DFFFFFF  Heap (~475MB)
 *   0x1E000000 - 0x1FFFFFFF  GPU memory (32MB, configured in config.txt)
 *
 * Stack grows DOWNWARD on ARM. Each stack pointer should be initialized
 * to the TOP (highest address) of its stack region.
 *
 * The kernel is loaded at 0x80000 by the GPU firmware (kernel8.img).
 */

OUTPUT_FORMAT("elf64-littleaarch64")
OUTPUT_ARCH(aarch64)
ENTRY(_start)

/* ============================================================================
 * Configuration - Adjust these as needed
 * ============================================================================ */

/* Stack sizes (must be 16-byte aligned for AArch64 ABI) */
__core0_stack_size = 256K;    /* Main core - larger for deep recursion */
__core1_stack_size = 64K;     /* Secondary cores (future) */
__core2_stack_size = 64K;
__core3_stack_size = 64K;
__irq_stack_size   = 64K;     /* Interrupt handler stack */
__abort_stack_size = 64K;     /* Abort/exception stack */

/* Total RAM available (before GPU split) */
__total_ram = 512M;

/* GPU memory reservation (must match gpu_mem in config.txt) */
__gpu_mem = 32M;

/* ============================================================================
 * Memory Regions
 * ============================================================================ */

MEMORY
{
    /* Kernel code and data - loaded here by GPU firmware */
    KERNEL (rwx) : ORIGIN = 0x80000, LENGTH = 1536K  /* 1.5MB max kernel */

    /* Stack region - placed at fixed address for predictable layout */
    STACK (rw) : ORIGIN = 0x200000, LENGTH = 1M

    /* Heap region - from end of stack to GPU memory */
    HEAP (rw) : ORIGIN = 0x300000, LENGTH = 475M
}

/* ============================================================================
 * Sections
 * ============================================================================ */

SECTIONS
{
    /* ========================================================================
     * Code Section
     * Boot code MUST be first - GPU jumps to 0x80000
     * ======================================================================== */
    .text : ALIGN(4K)
    {
        __text_start = .;

        /* Boot code must be at the very beginning */
        KEEP(*(.text.boot))

        /* All other code follows */
        *(.text .text.*)

        __text_end = .;
    } > KERNEL

    /* ========================================================================
     * Read-Only Data
     * ======================================================================== */
    .rodata : ALIGN(4K)
    {
        __rodata_start = .;
        *(.rodata .rodata.*)

        /* Ensure 8-byte alignment for any 64-bit constants */
        . = ALIGN(8);
        __rodata_end = .;
    } > KERNEL

    /* ========================================================================
     * Initialized Data
     * ======================================================================== */
    .data : ALIGN(4K)
    {
        __data_start = .;
        *(.data .data.*)
        . = ALIGN(8);
        __data_end = .;
    } > KERNEL

    /* ========================================================================
     * Uninitialized Data (BSS)
     * Zeroed by startup code before main
     * ======================================================================== */
    .bss (NOLOAD) : ALIGN(16)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    } > KERNEL

    /* End of kernel image */
    . = ALIGN(4K);
    __kernel_end = .;

    /* ========================================================================
     * Stack Section
     *
     * Layout (addresses increase downward in listing, stacks grow up in memory):
     *
     *   0x200000 +------------------+ __stack_start
     *            | Core 0 Stack     | 256KB
     *   0x240000 +------------------+ __core0_stack_top (SP for core 0)
     *            | Core 1 Stack     | 64KB
     *   0x250000 +------------------+ __core1_stack_top
     *            | Core 2 Stack     | 64KB
     *   0x260000 +------------------+ __core2_stack_top
     *            | Core 3 Stack     | 64KB
     *   0x270000 +------------------+ __core3_stack_top
     *            | IRQ Stack        | 64KB
     *   0x280000 +------------------+ __irq_stack_top
     *            | Abort Stack      | 64KB
     *   0x290000 +------------------+ __abort_stack_top
     *            | (unused/guard)   |
     *   0x300000 +------------------+ __stack_end
     *
     * Stack grows DOWN, so SP should be set to the TOP of each region.
     * ======================================================================== */
    .stack (NOLOAD) : ALIGN(16)
    {
        __stack_start = .;

        /* Core 0 stack (main core) - largest for emulator recursion */
        __core0_stack_bottom = .;
        . = . + __core0_stack_size;
        __core0_stack_top = .;

        /* Core 1 stack (future multi-core) */
        __core1_stack_bottom = .;
        . = . + __core1_stack_size;
        __core1_stack_top = .;

        /* Core 2 stack */
        __core2_stack_bottom = .;
        . = . + __core2_stack_size;
        __core2_stack_top = .;

        /* Core 3 stack */
        __core3_stack_bottom = .;
        . = . + __core3_stack_size;
        __core3_stack_top = .;

        /* IRQ stack (for interrupt handlers) */
        __irq_stack_bottom = .;
        . = . + __irq_stack_size;
        __irq_stack_top = .;

        /* Abort stack (for exceptions) */
        __abort_stack_bottom = .;
        . = . + __abort_stack_size;
        __abort_stack_top = .;

        /* Align to page boundary */
        . = ALIGN(4K);
        __stack_end = .;
    } > STACK

    /* ========================================================================
     * Heap Section
     * Managed by the global allocator at runtime
     * ======================================================================== */
    .heap (NOLOAD) : ALIGN(4K)
    {
        __heap_start = .;
        . = ORIGIN(HEAP) + LENGTH(HEAP);
        __heap_end = .;
    } > HEAP

    /* ========================================================================
     * Discarded Sections
     * ======================================================================== */
    /DISCARD/ :
    {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
        *(.ARM.*)
        *(.gnu.hash)
        *(.dynsym)
        *(.dynstr)
        *(.dynamic)
        *(.plt)
        *(.interp)
        *(.gnu*)
    }
}

/* ============================================================================
 * Exported Symbols for Startup Code
 * ============================================================================ */

/* Legacy symbol for simple single-core boot (points to core 0 stack top) */
PROVIDE(_stack_top = __core0_stack_top);

/* Kernel info */
PROVIDE(__kernel_start = ORIGIN(KERNEL));
PROVIDE(__kernel_size = __kernel_end - __kernel_start);

/* Stack info */
PROVIDE(__stack_size = __stack_end - __stack_start);

/* Heap info */
PROVIDE(__heap_size = __heap_end - __heap_start);

/* Memory bounds (useful for MMU setup) */
PROVIDE(__memory_start = 0x0);
PROVIDE(__memory_end = __total_ram - __gpu_mem);

/* ============================================================================
 * Assertions - Catch problems at link time
 * ============================================================================ */

/* Ensure kernel doesn't overflow into stack region */
ASSERT(__kernel_end <= ORIGIN(STACK), "Kernel too large - overlaps stack region!")

/* Ensure stack fits in its region */
ASSERT(__stack_end <= ORIGIN(HEAP), "Stack region overflow!")

/* Ensure we have reasonable heap space */
ASSERT(__heap_end - __heap_start >= 64M, "Heap too small!")

/* Ensure stacks are 16-byte aligned (AArch64 ABI requirement) */
ASSERT((__core0_stack_top & 0xF) == 0, "Core 0 stack not 16-byte aligned!")
ASSERT((__irq_stack_top & 0xF) == 0, "IRQ stack not 16-byte aligned!")
