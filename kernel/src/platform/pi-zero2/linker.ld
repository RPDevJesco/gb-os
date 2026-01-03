/* Linker script for Raspberry Pi Zero 2 W (BCM2710, Cortex-A53)
 *
 * Memory map:
 * - 0x00000000 - VideoCore GPU
 * - 0x00080000 - Kernel load address (where firmware loads kernel8.img)
 * - 0x3F000000 - Peripheral base (Pi 2/3/Zero2)
 */

ENTRY(_start)

MEMORY
{
    /* Pi Zero 2 W has 512MB RAM, kernel loaded at 0x80000 */
    RAM (rwx) : ORIGIN = 0x80000, LENGTH = 512M - 0x80000
}

/* Stack size: 64KB per core, we only use core 0 for now */
_stack_size = 0x10000;

SECTIONS
{
    /* Code and read-only data */
    .text : {
        KEEP(*(.text._start))       /* Entry point first */
        KEEP(*(.text.vectors))      /* Exception vectors */
        *(.text .text.*)
    } > RAM

    .rodata : ALIGN(8) {
        *(.rodata .rodata.*)
    } > RAM

    /* Read-write data */
    .data : ALIGN(8) {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    } > RAM

    /* Uninitialized data */
    .bss : ALIGN(8) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    /* Stack (grows downward) */
    . = ALIGN(16);
    _stack_bottom = .;
    . = . + _stack_size;
    _stack_top = .;

    /* Heap starts after stack */
    . = ALIGN(8);
    _heap_start = .;
    _heap_end = ORIGIN(RAM) + LENGTH(RAM);

    /* Discard unwanted sections */
    /DISCARD/ : {
        *(.ARM.*)
        *(.gnu.*)
        *(.note.*)
        *(.comment)
    }
}

/* Symbols for runtime */
__text_start = ADDR(.text);
__text_end = ADDR(.rodata);
__rodata_start = ADDR(.rodata);
__rodata_end = ADDR(.data);
