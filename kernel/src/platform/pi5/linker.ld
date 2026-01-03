/* Linker script for Raspberry Pi 5 (BCM2712, Cortex-A76)
 *
 * NOTE: Pi 5 memory map differs from earlier Pis.
 * This needs verification against actual hardware.
 */

ENTRY(_start)

MEMORY
{
    /* Pi 5 has 4GB/8GB RAM, kernel loaded at 0x80000 */
    /* Using 1GB as safe limit - actual RAM depends on model */
    RAM (rwx) : ORIGIN = 0x80000, LENGTH = 0x3FF80000
}

_stack_size = 0x10000;

SECTIONS
{
    .text : {
        KEEP(*(.text._start))
        KEEP(*(.text.vectors))
        *(.text .text.*)
    } > RAM

    .rodata : ALIGN(8) {
        *(.rodata .rodata.*)
    } > RAM

    .data : ALIGN(8) {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    } > RAM

    .bss : ALIGN(8) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    . = ALIGN(16);
    _stack_bottom = .;
    . = . + _stack_size;
    _stack_top = .;

    . = ALIGN(8);
    _heap_start = .;
    _heap_end = ORIGIN(RAM) + LENGTH(RAM);

    /DISCARD/ : {
        *(.ARM.*)
        *(.gnu.*)
        *(.note.*)
        *(.comment)
    }
}

__text_start = ADDR(.text);
__text_end = ADDR(.rodata);
__rodata_start = ADDR(.rodata);
__rodata_end = ADDR(.data);
