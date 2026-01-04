ENTRY(_start)
SECTIONS
{
    . = 0x80000;
    .text : { KEEP(*(.text.boot)) *(.text .text.*) }
    .rodata : ALIGN(8) { 
        *(.rodata .rodata.*) 
        *(.rodata.padding)
    }
    .data : ALIGN(8) { *(.data .data.*) }
    .bss ALIGN(16) : { __bss_start = .; *(.bss .bss.*) *(COMMON) __bss_end = .; }
    /DISCARD/ : { *(.ARM.*) *(.gnu.*) *(.note.*) *(.comment) *(.eh_frame*) }
}
