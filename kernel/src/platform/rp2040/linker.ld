ENTRY(_reset_handler)
MEMORY {
    FLASH (rx) : ORIGIN = 0x10000000, LENGTH = 2M
    RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 264K
}
SECTIONS {
    .text : { *(.text .text.*) } > FLASH
    .rodata : { *(.rodata .rodata.*) } > FLASH
    .data : { *(.data .data.*) } > RAM AT> FLASH
    .bss : { *(.bss .bss.*) } > RAM
}
