//! CPU control, cache management, and MMU configuration
//!
//! This module handles:
//! - Exception level detection
//! - Instruction and data cache control
//! - MMU page table setup for enabling D-cache

use crate::platform_core::mmio::{dsb, isb};

// ============================================================================
// Exception Level
// ============================================================================

/// Get current exception level (1, 2, or 3)
pub fn get_exception_level() -> u8 {
    let el: u64;
    unsafe {
        core::arch::asm!("mrs {}, CurrentEL", out(reg) el);
    }
    ((el >> 2) & 0x3) as u8
}

// ============================================================================
// Cache Control
// ============================================================================

/// Check if caches are enabled
/// Returns (icache_enabled, dcache_enabled)
pub fn check_caches() -> (bool, bool) {
    let sctlr: u64;
    unsafe {
        core::arch::asm!("mrs {}, sctlr_el1", out(reg) sctlr);
    }
    let icache = (sctlr & (1 << 12)) != 0;
    let dcache = (sctlr & (1 << 2)) != 0;
    (icache, dcache)
}

/// Enable instruction cache only (safe to use before MMU is set up)
#[inline(never)]
pub fn enable_icache() {
    unsafe {
        let mut sctlr: u64;
        core::arch::asm!("mrs {}, sctlr_el1", out(reg) sctlr);
        sctlr |= 1 << 12;   // Enable I-cache
        sctlr &= !(1 << 2); // Ensure D-cache is OFF (until MMU is on)
        core::arch::asm!("msr sctlr_el1, {}", in(reg) sctlr);
        core::arch::asm!("isb");
    }
}

/// Check if MMU is enabled
pub fn is_mmu_enabled() -> bool {
    let sctlr: u64;
    unsafe {
        core::arch::asm!("mrs {}, sctlr_el1", out(reg) sctlr);
    }
    (sctlr & 1) != 0
}

// ============================================================================
// MMU Configuration
// ============================================================================

// Page table - 512 entries, 4KB aligned
#[repr(C, align(4096))]
struct PageTable {
    entries: [u64; 512],
}

// Static page tables
static mut MMU_L1_TABLE: PageTable = PageTable { entries: [0; 512] };
static mut MMU_L2_TABLE: PageTable = PageTable { entries: [0; 512] };

// MAIR attribute indices
const MAIR_IDX_DEVICE: u64 = 0; // Device-nGnRnE
const MAIR_IDX_NORMAL: u64 = 1; // Normal cacheable

// MAIR register value:
// Attr0 = 0x00: Device-nGnRnE (for MMIO - no gather, no reorder, no early write ack)
// Attr1 = 0xFF: Normal, Inner/Outer Write-Back, Read/Write Allocate
const MAIR_VALUE: u64 = (0xFF << 8) | 0x00;

// Page table entry bits for block descriptors
const PTE_VALID: u64 = 1 << 0;
const PTE_BLOCK: u64 = 0 << 1;     // Block descriptor (not table)
const PTE_TABLE: u64 = 1 << 1;     // Table descriptor
const PTE_ATTR_IDX_SHIFT: u64 = 2;
const PTE_AP_RW: u64 = 0 << 6;     // Read-write at EL1
const PTE_SH_INNER: u64 = 3 << 8;  // Inner shareable
const PTE_AF: u64 = 1 << 10;       // Access flag (must be 1)
const PTE_UXN: u64 = 1 << 54;      // User execute never

// Block size for L2 entries: 2MB
const BLOCK_SIZE_2MB: u64 = 2 * 1024 * 1024;

/// Initialize MMU with identity mapping and enable D-cache
///
/// Memory map:
/// - 0x00000000 - 0x3EFFFFFF: Normal cacheable (RAM)
/// - 0x3F000000 - 0x3FFFFFFF: Device memory (peripherals)
/// - 0x40000000+: Device memory (ARM local peripherals)
///
/// # Safety
/// Must be called exactly once, early in boot, before D-cache is enabled.
/// All code/data must be in the first 1GB of address space.
#[inline(never)]
pub unsafe fn init_mmu() {
    // Calculate where peripheral space starts in terms of 2MB blocks
    // Peripheral base is 0x3F000000 = block 504 (0x3F000000 / 0x200000)
    const PERIPHERAL_BLOCK_START: usize = 504;

    // Fill L2 table with 2MB block descriptors (covers 0 - 1GB)
    for i in 0..512 {
        let block_addr = (i as u64) * BLOCK_SIZE_2MB;

        let entry = if i >= PERIPHERAL_BLOCK_START {
            // Device memory for peripherals (non-cacheable, non-bufferable)
            block_addr
                | PTE_VALID
                | PTE_BLOCK
                | (MAIR_IDX_DEVICE << PTE_ATTR_IDX_SHIFT)
                | PTE_AF
                | PTE_AP_RW
                | PTE_UXN  // Don't execute from device memory
        } else {
            // Normal cacheable memory for RAM
            block_addr
                | PTE_VALID
                | PTE_BLOCK
                | (MAIR_IDX_NORMAL << PTE_ATTR_IDX_SHIFT)
                | PTE_AF
                | PTE_AP_RW
                | PTE_SH_INNER  // Inner shareable for cacheability
        };

        MMU_L2_TABLE.entries[i] = entry;
    }

    // L1 table entry 0 points to L2 table (covers first 1GB)
    let l2_addr = &raw const MMU_L2_TABLE as *const _ as u64;
    MMU_L1_TABLE.entries[0] = l2_addr | PTE_VALID | PTE_TABLE;

    // Entries 1-3 for 1GB-4GB range - mark as device memory (1GB blocks)
    // This covers ARM local peripherals and any additional peripheral ranges
    for i in 1..4 {
        let block_addr = (i as u64) * (1024 * 1024 * 1024); // 1GB per entry
        MMU_L1_TABLE.entries[i] = block_addr
            | PTE_VALID
            | PTE_BLOCK
            | (MAIR_IDX_DEVICE << PTE_ATTR_IDX_SHIFT)
            | PTE_AF
            | PTE_AP_RW
            | PTE_UXN;
    }

    // Data synchronization barrier - ensure page tables are written
    core::arch::asm!("dsb sy");

    // Set MAIR_EL1 (Memory Attribute Indirection Register)
    core::arch::asm!("msr mair_el1, {}", in(reg) MAIR_VALUE);

    // Set TCR_EL1 (Translation Control Register)
    // T0SZ = 32: 32-bit address space (4GB)
    // IRGN0 = 1: Inner write-back, write-allocate
    // ORGN0 = 1: Outer write-back, write-allocate
    // SH0 = 3: Inner shareable
    // TG0 = 0: 4KB granule
    let tcr: u64 = (32 << 0)   // T0SZ = 32
        | (1 << 8)    // IRGN0 = write-back write-allocate
        | (1 << 10)   // ORGN0 = write-back write-allocate
        | (3 << 12)   // SH0 = inner shareable
        | (0 << 14);  // TG0 = 4KB granule
    core::arch::asm!("msr tcr_el1, {}", in(reg) tcr);

    // Set TTBR0_EL1 (Translation Table Base Register)
    let l1_addr = &raw const MMU_L1_TABLE as *const _ as u64;
    core::arch::asm!("msr ttbr0_el1, {}", in(reg) l1_addr);

    // Instruction barrier
    core::arch::asm!("isb");

    // Invalidate all TLB entries
    core::arch::asm!("tlbi vmalle1");
    core::arch::asm!("dsb sy");
    core::arch::asm!("isb");

    // Now enable MMU and D-cache in SCTLR_EL1
    let mut sctlr: u64;
    core::arch::asm!("mrs {}, sctlr_el1", out(reg) sctlr);
    sctlr |= 1 << 0;   // M = MMU enable
    sctlr |= 1 << 2;   // C = D-cache enable
    sctlr |= 1 << 12;  // I = I-cache enable
    core::arch::asm!("msr sctlr_el1, {}", in(reg) sctlr);

    // Final instruction barrier
    core::arch::asm!("isb");
}

/// Disable MMU and caches
///
/// # Safety
/// All cached data must be cleaned before calling this.
/// This is primarily useful for shutdown or before chain-loading.
#[inline(never)]
pub unsafe fn disable_mmu() {
    let mut sctlr: u64;
    core::arch::asm!("mrs {}, sctlr_el1", out(reg) sctlr);
    sctlr &= !(1 << 0);  // M = MMU disable
    sctlr &= !(1 << 2);  // C = D-cache disable
    core::arch::asm!("msr sctlr_el1, {}", in(reg) sctlr);
    core::arch::asm!("isb");

    // Invalidate TLBs
    core::arch::asm!("tlbi vmalle1");
    core::arch::asm!("dsb sy");
    core::arch::asm!("isb");
}
