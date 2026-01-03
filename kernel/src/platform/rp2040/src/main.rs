//! RP2040 Bootloader
//!
//! The RP2040 has a well-documented boot process:
//!
//! ## Boot Sequence
//!
//! 1. **Boot ROM** (16KB, hardcoded)
//!    - Checks boot source (flash, USB, UART)
//!    - Loads 256 bytes from flash (stage2)
//!    - Verifies CRC32
//!    - Jumps to stage2
//!
//! 2. **Stage2** (256 bytes max)
//!    - Configures XIP (Execute In Place) for QSPI flash
//!    - Usually just sets up flash timing
//!    - Jumps to main application
//!
//! 3. **Main Application**
//!    - Your actual bootloader code
//!    - Can be much larger (flash size limited)
//!
//! ## Flash Layout (typical W25Q128)
//!
//! | Offset    | Size     | Content         |
//! |-----------|----------|-----------------|
//! | 0x000     | 256B     | Stage2 + CRC    |
//! | 0x100     | varies   | Main app        |
//!
//! ## Stage2 Requirements
//!
//! - Must be exactly 256 bytes (252 code + 4 byte CRC)
//! - CRC32 of first 252 bytes stored in last 4 bytes
//! - Must configure XIP for the specific flash chip
//! - Must jump to address 0x10000100 (after stage2)
//!
//! ## Memory Map
//!
//! - 0x00000000 - Boot ROM (16KB)
//! - 0x10000000 - XIP flash (up to 16MB)
//! - 0x20000000 - SRAM (264KB in 6 banks)
//! - 0x40000000 - APB peripherals
//! - 0x50000000 - AHB-Lite peripherals
//! - 0xD0000000 - SIO (core-local)
//! - 0xE0000000 - Cortex-M0+ internal

#![no_std]
#![no_main]

mod boot2;

/// XIP flash base.
pub const XIP_BASE: usize = 0x1000_0000;

/// SRAM base.
pub const SRAM_BASE: usize = 0x2000_0000;

/// Peripheral base.
pub const PERIPH_BASE: usize = 0x4000_0000;

/// UART0 base.
pub const UART0_BASE: usize = PERIPH_BASE + 0x0003_4000;

/// UART1 base.
pub const UART1_BASE: usize = PERIPH_BASE + 0x0003_8000;

/// SIO (Single-cycle IO) base.
pub const SIO_BASE: usize = 0xD000_0000;

/// Resets controller.
pub const RESETS_BASE: usize = PERIPH_BASE + 0x000C_000;

/// IO bank 0 (GPIO).
pub const IO_BANK0_BASE: usize = PERIPH_BASE + 0x0001_4000;

/// Pads bank 0.
pub const PADS_BANK0_BASE: usize = PERIPH_BASE + 0x001C_000;

#[unsafe(no_mangle)]
pub extern "C" fn boot_main() -> ! {
    // TODO: RP2040 boot sequence
    // 1. Release peripherals from reset
    // 2. Configure clocks (XOSC, PLLs)
    // 3. Configure GPIO for UART
    // 4. Initialize UART for debug output
    // 5. Do bootloader things (load app, check for update mode, etc.)

    loop {
        arch_armv6m::wfi();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        arch_armv6m::wfi();
    }
}
