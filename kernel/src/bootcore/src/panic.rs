//! Panic handling utilities.
//!
//! Each platform binary needs to define its own panic handler,
//! but they can use these shared utilities.

/// Panic information that can be captured without allocations.
#[derive(Clone, Copy)]
pub struct PanicInfo<'a> {
    pub message: Option<&'a str>,
    pub file: Option<&'a str>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

impl<'a> PanicInfo<'a> {
    /// Extract info from core::panic::PanicInfo.
    pub fn from_core(info: &'a core::panic::PanicInfo<'a>) -> Self {
        let location = info.location();

        Self {
            message: info.message().as_str(),
            file: location.map(|l| l.file()),
            line: location.map(|l| l.line()),
            column: location.map(|l| l.column()),
        }
    }
}

/// Infinite loop for panic situations.
/// Uses architecture-specific halt instructions when possible.
#[inline(always)]
pub fn halt_loop() -> ! {
    loop {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("wfe", options(nomem, nostack));
        }

        #[cfg(target_arch = "arm")]
        unsafe {
            core::arch::asm!("wfe", options(nomem, nostack));
        }

        #[cfg(not(any(target_arch = "aarch64", target_arch = "arm")))]
        core::hint::spin_loop();
    }
}
