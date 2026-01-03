//! KickPi K2B Bootloader (Allwinner H618)
//!
//! The H618 is a Cortex-A53 quad-core SoC with a complex boot process:
//!
//! ## Allwinner Boot Chain
//!
//! 1. **BROM** (Boot ROM) - Hardcoded in silicon
//!    - Looks for boot media in order: SD0, SD2, NAND, SPI NOR, eMMC, FEL
//!    - Loads and verifies boot0/SPL from specific offsets
//!
//! 2. **boot0/SPL** (Secondary Program Loader) - This is what we're building
//!    - Loaded at 0x20000 (SRAM)
//!    - Must initialize DRAM
//!    - Loads U-Boot or custom payload
//!
//! 3. **U-Boot/Payload**
//!    - Loaded to DRAM
//!    - Full bootloader functionality
//!
//! ## Boot0 Header Format
//!
//! Allwinner expects a specific header at the start of boot0:
//! - Magic: "eGON.BT0" at offset 4
//! - Checksum at offset 12
//! - Length at offset 16
//! - Various other fields
//!
//! ## SD Card Layout
//!
//! | Offset (sectors) | Size    | Content      |
//! |-----------------|---------|--------------|
//! | 0               | 8KB     | Reserved     |
//! | 16 (8KB)        | 32KB    | boot0/SPL    |
//! | 80 (40KB)       | varies  | U-Boot       |
//!
//! ## Implementation Notes
//!
//! For bare-metal boot0:
//! 1. Entry at SRAM (0x00020000 on H618)
//! 2. Initialize clocks (PLL setup)
//! 3. Initialize DRAM controller
//! 4. Load next stage from SD/eMMC to DRAM
//! 5. Jump to loaded code
//!
//! Alternatively, use FEL mode for USB boot during development.

#![no_std]
#![no_main]

use core::arch::global_asm;

/// H618 SRAM base address (where boot0 runs).
pub const SRAM_BASE: usize = 0x0002_0000;

/// H618 peripheral base.
pub const PERIPHERAL_BASE: usize = 0x0200_0000;

/// CCU (Clock Control Unit) base.
pub const CCU_BASE: usize = 0x0200_1000;

/// UART0 base.
pub const UART0_BASE: usize = 0x0250_0000;

/// eGON boot header for Allwinner compatibility.
/// This must be at the very start of the binary.
#[repr(C)]
pub struct EgonHeader {
    /// Jump instruction (branch to real entry point)
    pub jump: u32,
    /// Magic: "eGON.BT0" = 0x4E4F4765, 0x3054422E
    pub magic: [u8; 8],
    /// Checksum (sum of all 32-bit words, with this field as 0x5F0A6C39)
    pub checksum: u32,
    /// Length of boot0 in bytes (must be multiple of 512)
    pub length: u32,
    /// SPL signature "SPL" for sunxi-tools compatibility
    pub spl_signature: [u8; 4],
    /// Fel flag
    pub fel_script_address: u32,
    /// Fel flag
    pub fel_uenv_length: u32,
    /// DT name offset
    pub dt_name_offset: u32,
    /// Reserved
    pub reserved1: u32,
    /// Boot media
    pub boot_media: u32,
    /// String pool
    pub string_pool: [u32; 13],
}

// Entry point with eGON header
global_asm!(
    r#"
.section .text._start
.global _start

_start:
    // eGON header - must be first 96 bytes
    b       _real_start         // Jump over header (offset 0)
    .ascii  "eGON.BT0"          // Magic (offset 4)
    .word   0x5F0A6C39          // Checksum placeholder (offset 12)
    .word   0x00008000          // Length: 32KB (offset 16)
    .ascii  "SPL\x00"           // SPL signature (offset 20)
    .word   0                   // fel_script_address (offset 24)
    .word   0                   // fel_uenv_length (offset 28)
    .word   0                   // dt_name_offset (offset 32)
    .word   0                   // reserved (offset 36)
    .word   0                   // boot_media (offset 40)
    .space  52                  // String pool (offset 44-95)

.align 4
_real_start:
    // Park secondary cores
    mrs     x0, mpidr_el1
    and     x0, x0, #0xFF
    cbnz    x0, .Lpark

    // Set up stack in SRAM
    ldr     x0, =_stack_top
    mov     sp, x0

    // Clear BSS
    ldr     x0, =__bss_start
    ldr     x1, =__bss_end
.Lclear_bss:
    cmp     x0, x1
    b.ge    .Lbss_done
    str     xzr, [x0], #8
    b       .Lclear_bss
.Lbss_done:

    // Call Rust entry
    bl      boot_main

.Lpark:
    wfe
    b       .Lpark
"#
);

#[unsafe(no_mangle)]
pub extern "C" fn boot_main() -> ! {
    // TODO: Full H618 boot sequence
    // 1. Clock initialization (PLL_CPUX, PLL_PERIPH0, etc.)
    // 2. UART0 initialization for debug output
    // 3. DRAM initialization (complex, SoC-specific)
    // 4. Load next stage from SD card
    // 5. Jump to loaded code

    // For now, just halt
    loop {
        arch_aarch64::wfe();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    bootcore::panic::halt_loop()
}
