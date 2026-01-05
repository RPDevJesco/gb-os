/*
 * Linker Script for GB-OS on Raspberry Pi Zero 2 W
 *
 * Memory Layout:
 *   0x00000000 - 0x0007FFFF  Reserved (GPU vectors, firmware)
 *   0x00080000 - 0x000FFFFF  Kernel code and data (512KB)
 *   0x00100000 - 0x001FFFFF  Stack (1MB, grows down)
 *   0x00200000 - 0x1DFFFFFF  Heap (~470MB)
 *   0x1E000000 - 0x1FFFFFFF  GPU memory (32MB)
 *
 * The kernel is loaded at 0x80000 by the GPU firmware.
 * This matches the default kernel8.img load address.
 */

OUTPUT_FORMAT("elf64-littleaarch64")
OUTPUT_ARCH(aarch64)
ENTRY(_start)

/* Memory regions */
MEMORY
{
    /* Kernel is loaded here by GPU firmware */
    RAM (rwx) : ORIGIN = 0x80000, LENGTH = 512K

    /* Stack region (separate for clarity) */
    STACK (rw) : ORIGIN = 0x100000, LENGTH = 1M

    /* Heap region */
    HEAP (rw) : ORIGIN = 0x200000, LENGTH = 470M
}

SECTIONS
{
    /*
     * Code section - must start with boot code
     * The .text.boot section contains _start and must be first
     */
    .text : ALIGN(4K)
    {
        __text_start = .;

        /* Boot code MUST be first */
        KEEP(*(.text.boot))

        /* Then all other code */
        *(.text .text.*)

        __text_end = .;
    } > RAM

    /*
     * Read-only data
     */
    .rodata : ALIGN(4K)
    {
        __rodata_start = .;
        *(.rodata .rodata.*)
        __rodata_end = .;
    } > RAM

    /*
     * Initialized data
     */
    .data : ALIGN(4K)
    {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    } > RAM

    /*
     * Uninitialized data (zeroed by startup code)
     */
    .bss : ALIGN(16)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    /* End of kernel code/data */
    __kernel_end = .;

    /*
     * Stack section
     * Stack grows downward from top of region
     */
    .stack (NOLOAD) : ALIGN(16)
    {
        __stack_start = .;
        . = . + 64K;  /* 64KB stack for core 0 */
        __stack_end = .;
        _stack_top = .;
    } > STACK

    /*
     * Heap section
     */
    .heap (NOLOAD) : ALIGN(4K)
    {
        __heap_start = .;
        . = ORIGIN(HEAP) + LENGTH(HEAP);
        __heap_end = .;
    } > HEAP

    /*
     * Discard sections we don't need
     */
    /DISCARD/ :
    {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
        *(.ARM.*)
    }
}

/* Provide symbols for startup code */
PROVIDE(__kernel_start = ORIGIN(RAM));
PROVIDE(__kernel_size = __kernel_end - __kernel_start);
PROVIDE(__stack_size = __stack_end - __stack_start);
PROVIDE(__heap_size = __heap_end - __heap_start);
