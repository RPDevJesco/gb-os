/* Linker script for RP2040 (Cortex-M0+)
 *
 * Memory map:
 * - Boot ROM: 0x00000000 (16KB, read-only)
 * - XIP Flash: 0x10000000 (up to 16MB)
 * - SRAM: 0x20000000 (264KB)
 *
 * Flash layout:
 * - 0x10000000: Stage2 bootloader (256 bytes)
 * - 0x10000100: Vector table + main application
 */

ENTRY(_reset_handler)

MEMORY
{
    /* Stage2 bootloader - must be first 256 bytes of flash */
    BOOT2 (rx) : ORIGIN = 0x10000000, LENGTH = 256

    /* Main flash - after stage2 */
    FLASH (rx) : ORIGIN = 0x10000100, LENGTH = 2M - 256

    /* SRAM - 264KB total in 6 banks */
    RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 264K
}

_stack_size = 0x2000; /* 8KB stack */

SECTIONS
{
    /* Stage2 bootloader - executed by boot ROM */
    .boot2 : {
        __boot2_start__ = .;
        KEEP(*(.boot2))
        __boot2_end__ = .;
    } > BOOT2

    ASSERT(__boot2_end__ - __boot2_start__ == 256,
           "Stage2 must be exactly 256 bytes")

    /* Vector table - must be at start of main flash */
    .vector_table : {
        KEEP(*(.vector_table))
    } > FLASH

    /* Code */
    .text : {
        *(.text._reset_handler)
        *(.text.handlers)
        *(.text .text.*)
    } > FLASH

    .rodata : ALIGN(4) {
        *(.rodata .rodata.*)
    } > FLASH

    /* Data loaded from flash to RAM */
    .data : ALIGN(4) {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    } > RAM AT> FLASH

    __data_load = LOADADDR(.data);

    /* Zero-initialized data */
    .bss : ALIGN(4) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    /* Stack */
    . = ALIGN(8);
    _stack_bottom = .;
    . = . + _stack_size;
    _stack_top = .;

    /* Heap */
    . = ALIGN(4);
    _heap_start = .;
    _heap_end = ORIGIN(RAM) + LENGTH(RAM);

    /DISCARD/ : {
        *(.ARM.*)
        *(.gnu.*)
        *(.note.*)
        *(.comment)
    }
}
