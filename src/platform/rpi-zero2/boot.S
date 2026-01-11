// =============================================================================
// boot.S - MMU only, enable I-cache later from Rust
// =============================================================================

.section ".text.boot"
.global _start

_start:
    mrs     x1, mpidr_el1
    and     x1, x1, #3
    cbnz    x1, .Lpark

    mov     x19, x0

    mrs     x0, CurrentEL
    lsr     x0, x0, #2
    cmp     x0, #3
    b.eq    .Lfrom_el3
    cmp     x0, #2
    b.eq    .Lfrom_el2
    b       .Lat_el1

.Lfrom_el3:
    mov     x0, #0x5b1
    msr     scr_el3, x0
    adr     x0, .Lfrom_el2
    msr     elr_el3, x0
    mov     x0, #0x3c9
    msr     spsr_el3, x0
    eret

.Lfrom_el2:
    msr     cptr_el2, xzr
    msr     hstr_el2, xzr
    mov     x0, #(1 << 31)
    orr     x0, x0, #(1 << 1)
    msr     hcr_el2, x0
    adr     x0, .Lat_el1
    msr     elr_el2, x0
    mov     x0, #0x3c5
    msr     spsr_el2, x0
    eret

.Lat_el1:
    ldr     x0, =exception_vectors
    msr     vbar_el1, x0
    isb

    ldr     x0, =_stack_top
    mov     sp, x0

    ldr     x0, =__bss_start
    ldr     x1, =__bss_end
.Lclear_bss:
    cmp     x0, x1
    b.ge    .Lbss_done
    stp     xzr, xzr, [x0], #16
    b       .Lclear_bss
.Lbss_done:

    mov     x0, #(3 << 20)
    msr     cpacr_el1, x0
    isb

    // MAIR: Attr0=Device(0x00), Attr1=Normal-WB(0xFF)
    ldr     x0, =0x000000000000FF00
    msr     mair_el1, x0
    isb

    // TCR
    mov     x0, #25
    orr     x0, x0, #(1 << 8)
    orr     x0, x0, #(1 << 10)
    orr     x0, x0, #(3 << 12)
    msr     tcr_el1, x0
    isb

    // Build L1
    ldr     x0, =mmu_l1_table
    ldr     x1, =mmu_l2_table
    orr     x1, x1, #3
    str     x1, [x0, #0]

    mov     x1, #0x40000000
    movk    x1, #0x0401, lsl #0
    str     x1, [x0, #8]

    // Build L2
    ldr     x0, =mmu_l2_table
    mov     x1, #0
    mov     x2, #0
    mov     x4, #0x705              // Normal WB: AttrIdx=1

.Ll2_loop:
    cmp     x2, #504
    b.ge    .Ll2_periph
    orr     x3, x1, x4
    str     x3, [x0, x2, lsl #3]
    b       .Ll2_next

.Ll2_periph:
    cmp     x2, #512
    b.ge    .Ll2_done
    mov     x3, x1
    movk    x3, #0x0401, lsl #0     // Device
    str     x3, [x0, x2, lsl #3]

.Ll2_next:
    add     x1, x1, #0x200000
    add     x2, x2, #1
    b       .Ll2_loop

.Ll2_done:
    dsb     sy

    ldr     x0, =mmu_l1_table
    msr     ttbr0_el1, x0
    isb

    tlbi    vmalle1
    dsb     nsh
    isb

    // MMU only - NO caches
    mrs     x0, sctlr_el1
    orr     x0, x0, #(1 << 0)       // M: MMU
    bic     x0, x0, #(1 << 2)       // C: OFF
    bic     x0, x0, #(1 << 12)      // I: OFF
    msr     sctlr_el1, x0
    isb

    mov     x0, x19
    bl      kernel_main

.Lpark:
    wfe
    b       .Lpark

.section ".text.vectors"
.balign 0x800
.global exception_vectors
exception_vectors:
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       sync_exception_handler
.balign 0x80
    b       irq_handler
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled
.balign 0x80
    b       .Lunhandled

.section ".text"

.macro SAVE_CONTEXT
    sub     sp, sp, #272
    stp     x0, x1, [sp, #0]
    stp     x2, x3, [sp, #16]
    stp     x4, x5, [sp, #32]
    stp     x6, x7, [sp, #48]
    stp     x8, x9, [sp, #64]
    stp     x10, x11, [sp, #80]
    stp     x12, x13, [sp, #96]
    stp     x14, x15, [sp, #112]
    stp     x16, x17, [sp, #128]
    stp     x18, x19, [sp, #144]
    stp     x20, x21, [sp, #160]
    stp     x22, x23, [sp, #176]
    stp     x24, x25, [sp, #192]
    stp     x26, x27, [sp, #208]
    stp     x28, x29, [sp, #224]
    str     x30, [sp, #240]
    mrs     x0, elr_el1
    mrs     x1, spsr_el1
    stp     x0, x1, [sp, #248]
.endm

.macro RESTORE_CONTEXT
    ldp     x0, x1, [sp, #248]
    msr     elr_el1, x0
    msr     spsr_el1, x1
    ldp     x0, x1, [sp, #0]
    ldp     x2, x3, [sp, #16]
    ldp     x4, x5, [sp, #32]
    ldp     x6, x7, [sp, #48]
    ldp     x8, x9, [sp, #64]
    ldp     x10, x11, [sp, #80]
    ldp     x12, x13, [sp, #96]
    ldp     x14, x15, [sp, #112]
    ldp     x16, x17, [sp, #128]
    ldp     x18, x19, [sp, #144]
    ldp     x20, x21, [sp, #160]
    ldp     x22, x23, [sp, #176]
    ldp     x24, x25, [sp, #192]
    ldp     x26, x27, [sp, #208]
    ldp     x28, x29, [sp, #224]
    ldr     x30, [sp, #240]
    add     sp, sp, #272
.endm

sync_exception_handler:
    SAVE_CONTEXT
    mov     x0, sp
    mrs     x1, esr_el1
    mrs     x2, far_el1
    bl      handle_sync_exception
    RESTORE_CONTEXT
    eret

irq_handler:
    SAVE_CONTEXT
    mov     x0, sp
    bl      handle_irq
    RESTORE_CONTEXT
    eret

.Lunhandled:
    SAVE_CONTEXT
    mov     x0, sp
    mrs     x1, esr_el1
    bl      handle_unhandled_exception
1:  wfe
    b       1b

.section ".data"
.balign 8
.global __dtb_ptr
__dtb_ptr:
    .quad 0

.section ".bss"
.balign 4096
.global mmu_l1_table
mmu_l1_table:
    .space 4096

.balign 4096
.global mmu_l2_table
mmu_l2_table:
    .space 4096

// Export function to enable I-cache from Rust
.section ".text"
.global enable_icache
enable_icache:
    ic      iallu
    dsb     nsh
    isb
    mrs     x0, sctlr_el1
    orr     x0, x0, #(1 << 12)
    msr     sctlr_el1, x0
    isb
    ret

.global enable_dcache
enable_dcache:
    mrs     x0, sctlr_el1
    orr     x0, x0, #(1 << 2)
    msr     sctlr_el1, x0
    isb
    ret

// Clean D-cache for a memory range (flush to RAM)
// x0 = start address, x1 = size in bytes
.global clean_dcache_range
clean_dcache_range:
    // Get cache line size
    mrs     x2, ctr_el0
    ubfx    x3, x2, #16, #4         // DminLine field
    mov     x2, #4
    lsl     x2, x2, x3              // Cache line size in bytes

    add     x1, x1, x0              // x1 = end address
    bic     x0, x0, x2              // Align start down
    sub     x0, x0, x2              // Adjust for first iteration
.Lclean_loop:
    add     x0, x0, x2              // Next line
    dc      cvac, x0                // Clean by VA to PoC
    cmp     x0, x1
    b.lo    .Lclean_loop

    dsb     sy
    ret
