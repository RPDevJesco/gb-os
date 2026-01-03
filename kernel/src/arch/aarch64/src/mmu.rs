//! Memory Management Unit setup for AArch64.
//!
//! Provides page table management and MMU configuration.
//! For a bootloader, we typically run with MMU disabled or
//! with identity mapping.

/// Page size options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSize {
    /// 4KB pages (most common)
    Size4K,
    /// 16KB pages
    Size16K,
    /// 64KB pages
    Size64K,
}

/// Memory attributes for page table entries.
#[derive(Debug, Clone, Copy)]
pub struct MemoryAttributes {
    /// Executable
    pub executable: bool,
    /// Writable
    pub writable: bool,
    /// User accessible (EL0)
    pub user: bool,
    /// Device memory (non-cacheable, non-bufferable)
    pub device: bool,
}

impl MemoryAttributes {
    pub const NORMAL: Self = Self {
        executable: true,
        writable: true,
        user: false,
        device: false,
    };

    pub const DEVICE: Self = Self {
        executable: false,
        writable: true,
        user: false,
        device: true,
    };

    pub const READONLY: Self = Self {
        executable: true,
        writable: false,
        user: false,
        device: false,
    };
}

/// Translation Control Register (TCR_EL1) configuration.
pub mod tcr {
    pub const T0SZ_SHIFT: u64 = 0;
    pub const EPD0: u64 = 1 << 7;
    pub const IRGN0_SHIFT: u64 = 8;
    pub const ORGN0_SHIFT: u64 = 10;
    pub const SH0_SHIFT: u64 = 12;
    pub const TG0_SHIFT: u64 = 14;
    pub const T1SZ_SHIFT: u64 = 16;
    pub const A1: u64 = 1 << 22;
    pub const EPD1: u64 = 1 << 23;
    pub const IRGN1_SHIFT: u64 = 24;
    pub const ORGN1_SHIFT: u64 = 26;
    pub const SH1_SHIFT: u64 = 28;
    pub const TG1_SHIFT: u64 = 30;
    pub const IPS_SHIFT: u64 = 32;

    // Granule size encoding for TG0
    pub const TG0_4K: u64 = 0b00 << TG0_SHIFT;
    pub const TG0_16K: u64 = 0b10 << TG0_SHIFT;
    pub const TG0_64K: u64 = 0b01 << TG0_SHIFT;

    // Inner cacheability
    pub const IRGN_NC: u64 = 0b00;      // Non-cacheable
    pub const IRGN_WBWA: u64 = 0b01;    // Write-back, write-allocate
    pub const IRGN_WT: u64 = 0b10;      // Write-through
    pub const IRGN_WB: u64 = 0b11;      // Write-back, no write-allocate

    // Shareability
    pub const SH_NONE: u64 = 0b00;
    pub const SH_OUTER: u64 = 0b10;
    pub const SH_INNER: u64 = 0b11;
}

/// Memory Attribute Indirection Register (MAIR_EL1) indices.
pub mod mair {
    /// Device-nGnRnE memory
    pub const DEVICE_NGNRNE: u8 = 0x00;
    /// Device-nGnRE memory
    pub const DEVICE_NGNRE: u8 = 0x04;
    /// Normal non-cacheable
    pub const NORMAL_NC: u8 = 0x44;
    /// Normal cacheable (write-back, write-allocate)
    pub const NORMAL_WBWA: u8 = 0xFF;
}

/// Read MAIR_EL1.
#[inline]
pub fn read_mair_el1() -> u64 {
    let val: u64;
    unsafe {
        core::arch::asm!(
            "mrs {}, mair_el1",
            out(reg) val,
            options(nomem, nostack)
        );
    }
    val
}

/// Write MAIR_EL1.
#[inline]
pub unsafe fn write_mair_el1(val: u64) {
    unsafe {
        core::arch::asm!(
            "msr mair_el1, {}",
            in(reg) val,
            options(nostack)
        );
    }
}

/// Read TCR_EL1.
#[inline]
pub fn read_tcr_el1() -> u64 {
    let val: u64;
    unsafe {
        core::arch::asm!(
            "mrs {}, tcr_el1",
            out(reg) val,
            options(nomem, nostack)
        );
    }
    val
}

/// Write TCR_EL1.
#[inline]
pub unsafe fn write_tcr_el1(val: u64) {
    unsafe {
        core::arch::asm!(
            "msr tcr_el1, {}",
            in(reg) val,
            options(nostack)
        );
    }
}

/// Read TTBR0_EL1.
#[inline]
pub fn read_ttbr0_el1() -> u64 {
    let val: u64;
    unsafe {
        core::arch::asm!(
            "mrs {}, ttbr0_el1",
            out(reg) val,
            options(nomem, nostack)
        );
    }
    val
}

/// Write TTBR0_EL1.
#[inline]
pub unsafe fn write_ttbr0_el1(val: u64) {
    unsafe {
        core::arch::asm!(
            "msr ttbr0_el1, {}",
            in(reg) val,
            options(nostack)
        );
    }
}

/// Invalidate all TLB entries.
#[inline]
pub unsafe fn invalidate_tlb() {
    unsafe {
        core::arch::asm!(
            "tlbi vmalle1",
            "dsb sy",
            "isb",
            options(nostack)
        );
    }
}
