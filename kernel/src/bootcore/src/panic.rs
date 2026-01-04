//! Panic handling utilities.
//!
//! Each platform binary needs to define its own panic handler,
//! but they can use these shared utilities.

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
