//! CPU register access and control.

/// System control register (SCTLR_EL1) bits.
pub mod sctlr {
    pub const M: u64 = 1 << 0;      // MMU enable
    pub const A: u64 = 1 << 1;      // Alignment check enable
    pub const C: u64 = 1 << 2;      // Data cache enable
    pub const SA: u64 = 1 << 3;     // Stack alignment check
    pub const I: u64 = 1 << 12;     // Instruction cache enable
    pub const WXN: u64 = 1 << 19;   // Write permission implies XN
    pub const EE: u64 = 1 << 25;    // Exception endianness (0 = little)
}

/// Read SCTLR_EL1.
#[inline]
pub fn read_sctlr_el1() -> u64 {
    let val: u64;
    unsafe {
        core::arch::asm!(
            "mrs {}, sctlr_el1",
            out(reg) val,
            options(nomem, nostack)
        );
    }
    val
}

/// Write SCTLR_EL1.
#[inline]
pub unsafe fn write_sctlr_el1(val: u64) {
    unsafe {
        core::arch::asm!(
            "msr sctlr_el1, {}",
            "isb",
            in(reg) val,
            options(nostack)
        );
    }
}

/// Read the counter frequency.
#[inline]
pub fn read_cntfrq_el0() -> u64 {
    let val: u64;
    unsafe {
        core::arch::asm!(
            "mrs {}, cntfrq_el0",
            out(reg) val,
            options(nomem, nostack)
        );
    }
    val
}

/// Read the physical counter.
#[inline]
pub fn read_cntpct_el0() -> u64 {
    let val: u64;
    unsafe {
        core::arch::asm!(
            "mrs {}, cntpct_el0",
            out(reg) val,
            options(nomem, nostack)
        );
    }
    val
}

/// Read the Main ID Register.
#[inline]
pub fn read_midr_el1() -> u64 {
    let val: u64;
    unsafe {
        core::arch::asm!(
            "mrs {}, midr_el1",
            out(reg) val,
            options(nomem, nostack)
        );
    }
    val
}

/// Decoded MIDR fields.
#[derive(Debug, Clone, Copy)]
pub struct Midr {
    pub implementer: u8,
    pub variant: u8,
    pub architecture: u8,
    pub part_num: u16,
    pub revision: u8,
}

impl Midr {
    pub fn read() -> Self {
        let val = read_midr_el1();
        Self {
            implementer: ((val >> 24) & 0xFF) as u8,
            variant: ((val >> 20) & 0xF) as u8,
            architecture: ((val >> 16) & 0xF) as u8,
            part_num: ((val >> 4) & 0xFFF) as u16,
            revision: (val & 0xF) as u8,
        }
    }

    /// Check if this is an ARM Cortex-A53.
    pub fn is_cortex_a53(&self) -> bool {
        self.implementer == 0x41 && self.part_num == 0xD03
    }

    /// Check if this is an ARM Cortex-A76.
    pub fn is_cortex_a76(&self) -> bool {
        self.implementer == 0x41 && self.part_num == 0xD0B
    }
}

/// Invalidate all instruction caches to PoU.
#[inline]
pub unsafe fn invalidate_icache() {
    unsafe {
        core::arch::asm!(
            "ic iallu",
            "isb",
            options(nostack)
        );
    }
}

/// Invalidate data cache line by virtual address.
#[inline]
pub unsafe fn invalidate_dcache_line(addr: usize) {
    unsafe {
        core::arch::asm!(
            "dc ivac, {}",
            in(reg) addr,
            options(nostack)
        );
    }
}

/// Clean data cache line by virtual address.
#[inline]
pub unsafe fn clean_dcache_line(addr: usize) {
    unsafe {
        core::arch::asm!(
            "dc cvac, {}",
            in(reg) addr,
            options(nostack)
        );
    }
}

/// Clean and invalidate data cache line by virtual address.
#[inline]
pub unsafe fn clean_invalidate_dcache_line(addr: usize) {
    unsafe {
        core::arch::asm!(
            "dc civac, {}",
            in(reg) addr,
            options(nostack)
        );
    }
}
