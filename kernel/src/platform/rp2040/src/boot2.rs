//! Stage2 bootloader for RP2040.
//!
//! This is the 256-byte second stage that configures XIP flash.
//! It's loaded by the boot ROM and must set up flash for XIP access.
//!
//! For W25Q-series flash (common on RP2040 boards):
//! - QSPI mode
//! - Continuous read mode for fast XIP

use core::arch::global_asm;

/// Stage2 must be exactly 252 bytes of code + 4 byte CRC.
#[allow(dead_code)]
pub const STAGE2_SIZE: usize = 256;

// Boot2 for W25Q flash chips (W25Q16, W25Q32, W25Q64, W25Q128).
// This configures the SSI for XIP with QSPI.
// Note: Uses only Thumb-1 instructions (ARMv6-M compatible).
global_asm!(
    r#"
.section .boot2, "ax"
.global __boot2_start
.align 2

__boot2_start:
    // Save lr
    push    {{lr}}

    // Set up XIP for W25Q flash
    // r3 = XIP_SSI_BASE
    ldr     r3, =0x18000000

    // 1. Disable SSI: SSIENR = 0
    movs    r1, #0
    str     r1, [r3, #8]

    // 2. Set baud rate (clock divider): BAUDR = 4
    movs    r1, #4
    str     r1, [r3, #0x14]

    // 3. Set up for single-bit SPI: CTRLR0 = 0
    movs    r1, #0
    str     r1, [r3, #0]

    // 4. Enable SSI: SSIENR = 1
    movs    r1, #1
    str     r1, [r3, #8]

    // 5. Disable SSI again for XIP config: SSIENR = 0
    movs    r1, #0
    str     r1, [r3, #8]

    // 6. Configure for XIP with QSPI
    // CTRLR0 = 0x001f0300 (32-bit frames, QSPI mode)
    ldr     r1, =0x001f0300
    str     r1, [r3, #0]

    // 7. Set instruction and address format
    // SPI_CTRLR0 at offset 0xf4 - need to compute address
    // (Thumb-1 str immediate offset must be <= 124 and word-aligned)
    movs    r4, #0xf4
    add     r4, r4, r3          // r4 = XIP_SSI_BASE + 0xf4
    ldr     r1, =0x02000218
    str     r1, [r4, #0]

    // 8. Enable SSI: SSIENR = 1
    movs    r1, #1
    str     r1, [r3, #8]

    // 9. Enable XIP caching
    ldr     r3, =0x14000000     // XIP_CTRL_BASE
    movs    r1, #1
    str     r1, [r3, #0]

    // Jump to main program (immediately after stage2)
    pop     {{r0}}
    ldr     r1, =0x10000101     // Main app start (thumb mode)
    bx      r1

.align 2
.pool

// Padding to get to 252 bytes
.space (252 - (. - __boot2_start))

// CRC32 will be patched here by build script
__boot2_crc:
    .word 0x00000000
"#
);

/// Calculate CRC32 for stage2 (same polynomial as boot ROM uses).
#[allow(dead_code)]
pub fn calc_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}
