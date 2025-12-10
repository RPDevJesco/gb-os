/* GameBoy OS Kernel Linker Script */
/* Kernel loads at 1MB (0x100000) */

OUTPUT_FORMAT("elf32-i386")
ENTRY(_start)

SECTIONS
{
    . = 0x100000;

    .text : ALIGN(4K)
    {
        /* CRITICAL: _start must be at the very beginning! */
        *(.text.boot)       /* Match main.rs global_asm section */
        *(.text .text.*)
    }

    .rodata : ALIGN(4K)
    {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4K)
    {
        *(.data .data.*)
    }

    .bss : ALIGN(4K)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    /DISCARD/ :
    {
        *(.eh_frame)
        *(.comment)
    }
}
