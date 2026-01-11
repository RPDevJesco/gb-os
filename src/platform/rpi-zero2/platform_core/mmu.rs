//! MMU setup for RPi Zero 2W (BCM2837)
//!
//! Memory map:
//!   0x00000000 - 0x3EFFFFFF: RAM (Normal, cacheable)
//!   0x3F000000 - 0x3FFFFFFF: Peripherals (Device, non-cacheable)
//!   0x40000000 - 0x40FFFFFF: Local peripherals (Device, non-cacheable)

use core::arch::asm;

// 4KB-aligned page tables
#[repr(C, align(4096))]
struct PageTable([u64; 512]);

#[unsafe(link_section = ".bss")]
static mut L1_TABLE: PageTable = PageTable([0; 512]);

#[unsafe(link_section = ".bss")]
static mut L2_TABLE: PageTable = PageTable([0; 512]);

// Descriptor type bits [1:0]
const DESC_INVALID: u64 = 0b00;
const DESC_BLOCK: u64   = 0b01;  // Block entry (L1=1GB, L2=2MB)
const DESC_TABLE: u64   = 0b11;  // Table entry (points to next level)

// Block/Page descriptor attribute bits
const ATTR_IDX_SHIFT: u64 = 2;   // AttrIndx[2:0] at bits [4:2]
const AP_RW_EL1: u64 = 0 << 6;   // AP[2:1]: R/W at EL1, no EL0 access
const SH_INNER: u64 = 3 << 8;    // SH[1:0]: Inner Shareable
const AF: u64 = 1 << 10;         // Access Flag (must be 1)

// MAIR attribute indices
const ATTR_DEVICE: u64 = 0;      // Index 0: Device-nGnRnE
const ATTR_NORMAL: u64 = 1;      // Index 1: Normal, Write-Back

pub unsafe fn init() {
    // Step 1: Set up memory attributes in MAIR_EL1
    // Attr0 = 0x00: Device-nGnRnE (no gathering, no reordering, no early ack)
    // Attr1 = 0xFF: Normal, Write-Back, Read-Allocate, Write-Allocate
    let mair: u64 = (0x00 << (ATTR_DEVICE * 8))
        | (0xFF << (ATTR_NORMAL * 8));
    asm!("msr mair_el1, {}", in(reg) mair);

    // Step 2: Configure translation control
    let tcr: u64 = (25 << 0)      // T0SZ=25: 39-bit VA space (512GB)
        | (0b00 << 14)   // TG0=00: 4KB granule
        | (0b01 << 8)    // IRGN0: Inner Write-Back Cacheable
        | (0b01 << 10)   // ORGN0: Outer Write-Back Cacheable
        | (0b11 << 12);  // SH0: Inner Shareable
    asm!("msr tcr_el1, {}", in(reg) tcr);

    // Step 3: Build page tables
    build_tables();

    // Step 4: Set translation table base
    let ttbr = &raw const L1_TABLE as u64;
    asm!("msr ttbr0_el1, {}", in(reg) ttbr);

    // Step 5: Ensure all writes complete before enabling MMU
    asm!("dsb sy");
    asm!("isb");

    // Step 6: Invalidate TLB
    asm!("tlbi vmalle1is");
    asm!("dsb ish");
    asm!("isb");

    // Step 7: Enable MMU and caches
    let mut sctlr: u64;
    asm!("mrs {}, sctlr_el1", out(reg) sctlr);
    sctlr |= 1 << 0;   // M: MMU enable
    sctlr |= 1 << 2;   // C: Data cache enable
    sctlr |= 1 << 12;  // I: Instruction cache enable
    asm!("msr sctlr_el1, {}", in(reg) sctlr);
    asm!("isb");
}

unsafe fn build_tables() {
    // L1 entry 0: 0x00000000 - 0x3FFFFFFF -> points to L2 table
    let l2_addr = &raw const L2_TABLE as u64;
    L1_TABLE.0[0] = l2_addr | DESC_TABLE;

    // L1 entry 1: 0x40000000 - 0x7FFFFFFF -> 1GB device block (local peripherals)
    L1_TABLE.0[1] = 0x40000000
        | DESC_BLOCK
        | (ATTR_DEVICE << ATTR_IDX_SHIFT)
        | AF;

    // L2 table: 512 entries, each covering 2MB
    // Entries 0-503 (0x00000000 - 0x3EFFFFFF): Normal RAM
    // Entries 504-511 (0x3F000000 - 0x3FFFFFFF): Device peripherals

    let normal_attrs = DESC_BLOCK
        | (ATTR_NORMAL << ATTR_IDX_SHIFT)
        | AP_RW_EL1
        | SH_INNER
        | AF;

    let device_attrs = DESC_BLOCK
        | (ATTR_DEVICE << ATTR_IDX_SHIFT)
        | AF;

    // 0x00000000 - 0x3EFFFFFF: Normal memory (504 * 2MB = 1008MB)
    for i in 0..504 {
        let addr = (i as u64) * 0x200000;  // 2MB per entry
        L2_TABLE.0[i] = addr | normal_attrs;
    }

    // 0x3F000000 - 0x3FFFFFFF: Device memory (8 * 2MB = 16MB)
    for i in 504..512 {
        let addr = (i as u64) * 0x200000;
        L2_TABLE.0[i] = addr | device_attrs;
    }
}
