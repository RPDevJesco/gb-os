//! Raspberry Pi Zero 2 W Bootloader
//!
//! Entry point for the Pi Zero 2 W (BCM2710, Cortex-A53).
//! The GPU firmware loads this as kernel8.img at 0x80000.

#![no_std]
#![no_main]

mod gpio;
mod uart;
mod mmio;

use bootcore::Serial;
use core::fmt::Write;

/// Pi Zero 2 W peripheral base address.
/// BCM2710/BCM2837 uses 0x3F000000 (same as Pi 3).
pub const PERIPHERAL_BASE: usize = 0x3F00_0000;

/// Main boot entry point, called from assembly after stack setup.
#[unsafe(no_mangle)]
pub extern "C" fn boot_main() -> ! {
    // Initialize UART for debug output
    let mut uart = uart::MiniUart::new();
    uart.init(115200).expect("UART init failed");

    // Print banner
    uart.write_line("");
    uart.write_line("=====================================");
    uart.write_line(" rustboot - Pi Zero 2 W");
    uart.write_line("=====================================");
    uart.write_line("");

    // Print CPU info
    let el = arch_aarch64::current_el();
    let core = arch_aarch64::core_id();
    let midr = arch_aarch64::cpu::Midr::read();

    let _ = writeln!(
        bootcore::fmt::SerialWriter(&mut uart),
        "Exception Level: EL{}",
        el
    );
    let _ = writeln!(
        bootcore::fmt::SerialWriter(&mut uart),
        "Core ID: {}",
        core
    );
    let _ = writeln!(
        bootcore::fmt::SerialWriter(&mut uart),
        "MIDR: impl=0x{:02X} part=0x{:03X} rev=r{}p{}",
        midr.implementer,
        midr.part_num,
        midr.variant,
        midr.revision
    );

    if midr.is_cortex_a53() {
        uart.write_line("CPU: ARM Cortex-A53");
    }

    uart.write_line("");
    uart.write_line("Boot complete. Entering echo mode...");
    uart.write_line("Type characters to test UART:");
    uart.write_line("");

    // Simple echo loop
    loop {
        let byte = uart.read_byte();

        // Echo back
        uart.write_byte(byte);

        // If Enter, also send newline
        if byte == b'\r' {
            uart.write_byte(b'\n');
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Try to print panic info over UART
    let mut uart = uart::MiniUart::new();
    let _ = uart.init(115200);

    uart.write_line("");
    uart.write_line("!!! PANIC !!!");

    if let Some(location) = info.location() {
        let _ = writeln!(
            bootcore::fmt::SerialWriter(&mut uart),
            "at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    }

    if let Some(msg) = info.message().as_str() {
        uart.write_str("message: ");
        uart.write_line(msg);
    }

    bootcore::panic::halt_loop()
}
