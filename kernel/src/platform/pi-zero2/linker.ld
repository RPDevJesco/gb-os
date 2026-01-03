/* Raspberry Pi Zero 2 W Linker Script
 *
 * Memory Split:
 *   GPU:    32MB  (configured via gpu_mem=32 in config.txt)
 *   Kernel: 10MB  (code, stacks, DMA, page tables)
 *   Heap:   470MB (dynamic allocation)
 *
 * Layout:
 *   0x0000_0000 - 0x0007_FFFF    Reserved (512KB)
 *   0x0008_0000 - 0x00A7_FFFF    Kernel region (10MB)
 *   0x00A8_0000 - 0x1DFF_FFFF    Heap (~470MB)
 *   0x1E00_0000 - 0x1FFF_FFFF    GPU (32MB)
 */

ENTRY(_start)

MEMORY
{
    /* Kernel code region: first 2MB */
    CODE (rwx) : ORIGIN = 0x00080000, LENGTH = 2M
    
    /* Stack region: 256KB at fixed address */
    STACK (rw) : ORIGIN = 0x00280000, LENGTH = 256K
    
    /* DMA buffer region: 256KB */
    DMA (rw)   : ORIGIN = 0x002C0000, LENGTH = 256K
    
    /* Page table region: 1MB */
    PGTBL (rw) : ORIGIN = 0x00300000, LENGTH = 1M
    
    /* Reserved kernel space */
    KRESERVED (rw) : ORIGIN = 0x00400000, LENGTH = 6656K
    
    /* Heap region: ~470MB */
    HEAP (rw)  : ORIGIN = 0x00A80000, LENGTH = 470M
}

SECTIONS
{
    /* ================================================================
     * Kernel Code (must start at 0x80000)
     * ================================================================ */
    .text : {
        __kernel_start = .;
        KEEP(*(.text._start))       /* Entry point FIRST */
        KEEP(*(.text.vectors))      /* Exception vectors */
        *(.text .text.*)
    } > CODE

    .rodata : ALIGN(8) {
        *(.rodata .rodata.*)
    } > CODE

    .data : ALIGN(8) {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    } > CODE

    .bss : ALIGN(8) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } > CODE
    
    __code_end = .;

    /* ================================================================
     * Stacks - 256KB total (64KB × 4 cores)
     * Placed in dedicated STACK memory region
     * ================================================================ */
    .stacks (NOLOAD) : {
        __stacks_start = .;
        
        _stack_bottom_core0 = .;
        . += 64K;
        _stack_top_core0 = .;
        
        _stack_bottom_core1 = .;
        . += 64K;
        _stack_top_core1 = .;
        
        _stack_bottom_core2 = .;
        . += 64K;
        _stack_top_core2 = .;
        
        _stack_bottom_core3 = .;
        . += 64K;
        _stack_top_core3 = .;
        
        __stacks_end = .;
    } > STACK
    
    /* Legacy symbols */
    _stack_top = _stack_top_core0;
    _stack_bottom = _stack_bottom_core0;

    /* ================================================================
     * DMA Buffers - 256KB
     * Must be 16-byte aligned for mailbox
     * ================================================================ */
    .dma (NOLOAD) : ALIGN(16) {
        __dma_start = .;
        
        _mailbox_buffer = .;
        . += 256;
        
        . = ALIGN(4K);
        _emmc_buffer = .;
        . += 64K;
        
        _dma_pool = .;
        
        __dma_end = ORIGIN(DMA) + LENGTH(DMA);
    } > DMA

    /* ================================================================
     * Page Tables - 1MB
     * ================================================================ */
    .page_tables (NOLOAD) : ALIGN(4K) {
        __pgtbl_start = .;
        _ttbr0 = .;             /* L1 table */
        . += 4K;
        _l2_tables = .;         /* L2 tables */
        __pgtbl_end = ORIGIN(PGTBL) + LENGTH(PGTBL);
    } > PGTBL

    /* ================================================================
     * Kernel region ends at 0x00A8_0000
     * ================================================================ */
    __kernel_end = 0x00A80000;

    /* ================================================================
     * Heap - 470MB starting at 0x00A8_0000
     * ================================================================ */
    .heap (NOLOAD) : {
        __heap_start = .;
        _heap_start = .;
    } > HEAP
    
    _heap_end = ORIGIN(HEAP) + LENGTH(HEAP);
    __heap_end = _heap_end;

    /* ================================================================
     * Discard
     * ================================================================ */
    /DISCARD/ : {
        *(.ARM.*)
        *(.gnu.*)
        *(.note.*)
        *(.comment)
        *(.eh_frame*)
    }
}

/* ====================================================================
 * Exported Symbols
 * ====================================================================
 *
 * Kernel:     __kernel_start, __kernel_end, __code_end
 * BSS:        __bss_start, __bss_end  
 * Stacks:     _stack_top_core[0-3], _stack_bottom_core[0-3]
 * DMA:        __dma_start, __dma_end, _mailbox_buffer, _emmc_buffer
 * Page Tables: __pgtbl_start, __pgtbl_end, _ttbr0, _l2_tables
 * Heap:       _heap_start, _heap_end
 */

/* Assertions - verify kernel code fits in CODE region */
ASSERT(__code_end <= ORIGIN(STACK), "Kernel code overflows into stack region!")
ASSERT(__code_end <= 0x00280000, "Kernel code too large (max 2MB)")
