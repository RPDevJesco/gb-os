//! Raspberry Pi 5 Bootloader
//!
//! Entry point for the Pi 5 (BCM2712, Cortex-A76).
//!
//! NOTE: Pi 5 has a significantly different boot flow:
//! - Uses RP1 southbridge for I/O
//! - Different UART base addresses
//! - New device tree handling
//!
//! This is a placeholder - full implementation TBD.

#![no_std]
#![no_main]

/// Pi 5 peripheral base address.
/// BCM2712 uses a different memory map than BCM2710/2711.
pub const PERIPHERAL_BASE: usize = 0x1_0000_0000; // Placeholder - needs verification

#[unsafe(no_mangle)]
pub extern "C" fn boot_main() -> ! {
    // TODO: Implement Pi 5 boot sequence
    // - RP1-based UART initialization
    // - Different GPIO controller
    // - PCIe initialization if needed

    loop {
        arch_aarch64::wfe();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    bootcore::panic::halt_loop()
}
