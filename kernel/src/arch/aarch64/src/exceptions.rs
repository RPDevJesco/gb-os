//! Exception handling for AArch64.
//!
//! Sets up the exception vector table and provides default handlers.

use core::arch::global_asm;

/// Exception context saved on entry to exception handler.
#[repr(C)]
pub struct ExceptionContext {
    /// General purpose registers x0-x30.
    pub gpr: [u64; 31],
    /// Exception Link Register.
    pub elr: u64,
    /// Saved Program Status Register.
    pub spsr: u64,
    /// Exception Syndrome Register.
    pub esr: u64,
    /// Fault Address Register.
    pub far: u64,
}

/// Exception type from ESR_EL1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExceptionClass {
    Unknown = 0x00,
    WfxTrap = 0x01,
    Cp15RtTrap = 0x03,
    Cp15RrtTrap = 0x04,
    Cp14RtTrap = 0x05,
    Cp14DttTrap = 0x06,
    SveAsmidTrap = 0x07,
    Cp14RrtTrap = 0x0C,
    IllegalExecution = 0x0E,
    Svc32 = 0x11,
    Svc64 = 0x15,
    SysTrap = 0x18,
    InstrAbortLower = 0x20,
    InstrAbortSame = 0x21,
    PcAlign = 0x22,
    DataAbortLower = 0x24,
    DataAbortSame = 0x25,
    SpAlign = 0x26,
    Fpu32 = 0x28,
    Fpu64 = 0x2C,
    SError = 0x2F,
    BreakpointLower = 0x30,
    BreakpointSame = 0x31,
    StepLower = 0x32,
    StepSame = 0x33,
    WatchpointLower = 0x34,
    WatchpointSame = 0x35,
    Bkpt32 = 0x38,
    Brk64 = 0x3C,
}

impl From<u64> for ExceptionClass {
    fn from(esr: u64) -> Self {
        let ec = ((esr >> 26) & 0x3F) as u8;
        // Safety: We handle unknown values
        match ec {
            0x00 => Self::Unknown,
            0x01 => Self::WfxTrap,
            0x0E => Self::IllegalExecution,
            0x15 => Self::Svc64,
            0x20 => Self::InstrAbortLower,
            0x21 => Self::InstrAbortSame,
            0x22 => Self::PcAlign,
            0x24 => Self::DataAbortLower,
            0x25 => Self::DataAbortSame,
            0x26 => Self::SpAlign,
            0x2F => Self::SError,
            0x3C => Self::Brk64,
            _ => Self::Unknown,
        }
    }
}

/// Install the exception vector table.
///
/// # Safety
/// Must be called from EL1 or higher.
#[inline]
pub unsafe fn install_vector_table() {
    unsafe extern "C" {
        static __exception_vectors: u8;
    }

    unsafe {
        let vectors = &__exception_vectors as *const _ as u64;
        core::arch::asm!(
            "msr vbar_el1, {}",
            "isb",
            in(reg) vectors,
            options(nostack)
        );
    }
}

// Exception vector table.
// Each entry is 128 bytes (0x80), containing up to 32 instructions.
global_asm!(
    r#"
.section .text.vectors
.balign 0x800
.global __exception_vectors
__exception_vectors:

// Current EL with SP0
.balign 0x80
    b       __exception_sync_sp0
.balign 0x80
    b       __exception_irq_sp0
.balign 0x80
    b       __exception_fiq_sp0
.balign 0x80
    b       __exception_serror_sp0

// Current EL with SPx
.balign 0x80
    b       __exception_sync_spx
.balign 0x80
    b       __exception_irq_spx
.balign 0x80
    b       __exception_fiq_spx
.balign 0x80
    b       __exception_serror_spx

// Lower EL using AArch64
.balign 0x80
    b       __exception_sync_lower64
.balign 0x80
    b       __exception_irq_lower64
.balign 0x80
    b       __exception_fiq_lower64
.balign 0x80
    b       __exception_serror_lower64

// Lower EL using AArch32
.balign 0x80
    b       __exception_sync_lower32
.balign 0x80
    b       __exception_irq_lower32
.balign 0x80
    b       __exception_fiq_lower32
.balign 0x80
    b       __exception_serror_lower32

// Default handlers - just hang
__exception_sync_sp0:
__exception_irq_sp0:
__exception_fiq_sp0:
__exception_serror_sp0:
__exception_sync_spx:
__exception_irq_spx:
__exception_fiq_spx:
__exception_serror_spx:
__exception_sync_lower64:
__exception_irq_lower64:
__exception_fiq_lower64:
__exception_serror_lower64:
__exception_sync_lower32:
__exception_irq_lower32:
__exception_fiq_lower32:
__exception_serror_lower32:
    wfe
    b       .
"#
);
