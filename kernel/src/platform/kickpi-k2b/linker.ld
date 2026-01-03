/* Linker script for KickPi K2B (Allwinner H618) boot0/SPL
 *
 * The boot0 runs from SRAM before DRAM is initialized.
 * H618 has 160KB SRAM at 0x00020000.
 *
 * Boot0 must:
 * - Start with eGON header
 * - Fit in 32KB (typical boot0 size)
 * - Initialize DRAM before loading larger payloads
 */

ENTRY(_start)

MEMORY
{
    /* H618 SRAM: 160KB at 0x00020000 */
    SRAM (rwx) : ORIGIN = 0x00020000, LENGTH = 160K
}

/* Boot0 is typically limited to 32KB */
_boot0_max_size = 32K;
_stack_size = 0x4000; /* 16KB stack */

SECTIONS
{
    /* Code must start with eGON header */
    .text : {
        KEEP(*(.text._start))       /* Entry + eGON header */
        KEEP(*(.text.vectors))
        *(.text .text.*)
    } > SRAM

    .rodata : ALIGN(8) {
        *(.rodata .rodata.*)
    } > SRAM

    .data : ALIGN(8) {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    } > SRAM

    .bss : ALIGN(8) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } > SRAM

    /* Stack at end of SRAM */
    . = ALIGN(16);
    _stack_bottom = .;
    . = . + _stack_size;
    _stack_top = .;

    /* Check we fit in boot0 size limit */
    _boot0_end = .;
    ASSERT(_boot0_end - ORIGIN(SRAM) <= _boot0_max_size,
           "boot0 exceeds 32KB size limit")

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
