/*
 * Raspberry Pi Zero 2 W Kernel Linker Script
 * BCM2710A1 (Cortex-A53) - AArch64
 *
 * Kernel loads at 0x80000 in 64-bit mode
 * This is where the GPU firmware places us after processing config.txt
 */

ENTRY(_start)

SECTIONS
{
    /* Kernel starts at 0x80000 (512KB mark) */
    . = 0x80000;

    /* Code section - boot code MUST be first */
    .text : {
        KEEP(*(.text.boot))    /* Boot assembly first */
        *(.text .text.*)
    }

    /* Read-only data */
    .rodata : ALIGN(8) {
        *(.rodata .rodata.*)
        *(.rodata.padding)
    }

    /* Initialized data */
    .data : ALIGN(8) {
        *(.data .data.*)
    }

    /* BSS - zero-initialized data */
    .bss ALIGN(16) : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    /* BSS size in 8-byte units (for clearing loop) */
    __bss_size = (__bss_end - __bss_start) >> 3;

    . = ALIGN(16);
    __end = .;

    /* Discard unwanted sections */
    /DISCARD/ : {
        *(.ARM.*)
        *(.gnu.*)
        *(.note.*)
        *(.comment)
        *(.eh_frame*)
    }
}
