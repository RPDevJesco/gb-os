//! CPU register access and control.

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
