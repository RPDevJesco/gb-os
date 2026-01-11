//! GPIO configuration for Raspberry Pi Zero 2W
//!
//! This module handles GPIO pin function selection and pull-up/down
//! configuration for:
//! - DPI display (GPIO 0-27)
//! - SD card via SDHOST (GPIO 48-53)

use crate::platform_core::mmio::{mmio_read, mmio_write, delay_us, PERIPHERAL_BASE};

// ============================================================================
// GPIO Register Addresses
// ============================================================================

const GPIO_BASE: usize = PERIPHERAL_BASE + 0x0020_0000;

/// GPIO Function Select registers (3 bits per pin)
const GPFSEL0: usize = GPIO_BASE + 0x00; // GPIO 0-9
const GPFSEL1: usize = GPIO_BASE + 0x04; // GPIO 10-19
const GPFSEL2: usize = GPIO_BASE + 0x08; // GPIO 20-29
const GPFSEL3: usize = GPIO_BASE + 0x0C; // GPIO 30-39
const GPFSEL4: usize = GPIO_BASE + 0x10; // GPIO 40-49
const GPFSEL5: usize = GPIO_BASE + 0x14; // GPIO 50-53

/// GPIO Pin Output Set registers
const GPSET0: usize = GPIO_BASE + 0x1C;  // GPIO 0-31
const GPSET1: usize = GPIO_BASE + 0x20;  // GPIO 32-53

/// GPIO Pin Output Clear registers
const GPCLR0: usize = GPIO_BASE + 0x28;  // GPIO 0-31
const GPCLR1: usize = GPIO_BASE + 0x2C;  // GPIO 32-53

/// GPIO Pin Level registers
const GPLEV0: usize = GPIO_BASE + 0x34;  // GPIO 0-31
const GPLEV1: usize = GPIO_BASE + 0x38;  // GPIO 32-53

/// GPIO Pull-up/down Enable register
const GPPUD: usize = GPIO_BASE + 0x94;

/// GPIO Pull-up/down Enable Clock registers
const GPPUDCLK0: usize = GPIO_BASE + 0x98; // GPIO 0-31
const GPPUDCLK1: usize = GPIO_BASE + 0x9C; // GPIO 32-53

// ============================================================================
// GPIO Function Select Values
// ============================================================================

/// GPIO function select values (3 bits each)
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum GpioFunction {
    Input  = 0b000,
    Output = 0b001,
    Alt0   = 0b100,
    Alt1   = 0b101,
    Alt2   = 0b110,
    Alt3   = 0b111,
    Alt4   = 0b011,
    Alt5   = 0b010,
}

/// GPIO pull-up/down configuration
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum GpioPull {
    Off  = 0,
    Down = 1,
    Up   = 2,
}

// ============================================================================
// GPIO Operations
// ============================================================================

/// Set the function of a single GPIO pin
pub fn set_pin_function(pin: u32, function: GpioFunction) {
    let reg_offset = (pin / 10) as usize * 4;
    let reg_addr = GPIO_BASE + reg_offset;
    let shift = (pin % 10) * 3;
    let mask = 0b111 << shift;
    
    let mut val = mmio_read(reg_addr);
    val = (val & !mask) | ((function as u32) << shift);
    mmio_write(reg_addr, val);
}

/// Configure pull-up/down for a GPIO pin
pub fn set_pin_pull(pin: u32, pull: GpioPull) {
    // Set the pull type
    mmio_write(GPPUD, pull as u32);
    delay_us(150);
    
    // Clock the configuration to the specific pin
    if pin < 32 {
        mmio_write(GPPUDCLK0, 1 << pin);
    } else {
        mmio_write(GPPUDCLK1, 1 << (pin - 32));
    }
    delay_us(150);
    
    // Clear the configuration
    mmio_write(GPPUD, 0);
    if pin < 32 {
        mmio_write(GPPUDCLK0, 0);
    } else {
        mmio_write(GPPUDCLK1, 0);
    }
}

/// Set a GPIO pin high
pub fn set_pin_high(pin: u32) {
    if pin < 32 {
        mmio_write(GPSET0, 1 << pin);
    } else {
        mmio_write(GPSET1, 1 << (pin - 32));
    }
}

/// Set a GPIO pin low
pub fn set_pin_low(pin: u32) {
    if pin < 32 {
        mmio_write(GPCLR0, 1 << pin);
    } else {
        mmio_write(GPCLR1, 1 << (pin - 32));
    }
}

/// Read the level of a GPIO pin
pub fn read_pin(pin: u32) -> bool {
    let val = if pin < 32 {
        mmio_read(GPLEV0)
    } else {
        mmio_read(GPLEV1)
    };
    let bit = if pin < 32 { pin } else { pin - 32 };
    (val & (1 << bit)) != 0
}

// ============================================================================
// DPI Display Configuration
// ============================================================================

/// Configure GPIO 0-27 for DPI display output (24-bit color)
///
/// DPI uses ALT2 function on GPIO 0-27:
/// - GPIO 0-7:   Data bits (various)
/// - GPIO 8-15:  More data bits
/// - GPIO 16-19: Control signals (HSYNC, VSYNC, etc.)
/// - GPIO 20-27: Red channel [7:0]
pub fn configure_for_dpi() {
    const ALT2: u32 = GpioFunction::Alt2 as u32;

    // GPIO 0-9: All ALT2 for DPI (GPFSEL0)
    let gpfsel0_val: u32 = (ALT2 << 0) | (ALT2 << 3) | (ALT2 << 6) | (ALT2 << 9)
        | (ALT2 << 12) | (ALT2 << 15) | (ALT2 << 18) | (ALT2 << 21)
        | (ALT2 << 24) | (ALT2 << 27);

    // GPIO 10-19: All ALT2 for DPI (GPFSEL1)
    let gpfsel1_val: u32 = (ALT2 << 0) | (ALT2 << 3) | (ALT2 << 6) | (ALT2 << 9)
        | (ALT2 << 12) | (ALT2 << 15) | (ALT2 << 18) | (ALT2 << 21)
        | (ALT2 << 24) | (ALT2 << 27);

    // GPIO 20-27: All ALT2 for DPI (GPFSEL2) - CRITICAL FOR RED CHANNEL!
    let gpfsel2_val: u32 = (ALT2 << 0) | (ALT2 << 3) | (ALT2 << 6) | (ALT2 << 9)
        | (ALT2 << 12) | (ALT2 << 15) | (ALT2 << 18) | (ALT2 << 21);

    mmio_write(GPFSEL0, gpfsel0_val);
    mmio_write(GPFSEL1, gpfsel1_val);
    mmio_write(GPFSEL2, gpfsel2_val);

    // Disable pull-up/down on all DPI pins (GPIO 0-27)
    mmio_write(GPPUD, 0);
    delay_us(150);
    mmio_write(GPPUDCLK0, 0x0FFF_FFFF); // GPIO 0-27
    delay_us(150);
    mmio_write(GPPUD, 0);
    mmio_write(GPPUDCLK0, 0);
}

// ============================================================================
// SD Card Configuration
// ============================================================================

/// Configure GPIO 48-53 for SDHOST controller
///
/// SDHOST uses ALT0 function:
/// - GPIO 48: CLK
/// - GPIO 49: CMD
/// - GPIO 50-53: DAT0-DAT3
pub fn configure_for_sd() {
    const ALT0: u32 = GpioFunction::Alt0 as u32;

    // GPIO 48-49 are in GPFSEL4 (bits 24-29)
    let gpfsel4 = mmio_read(GPFSEL4);
    let gpfsel4_new = (gpfsel4 & 0xC0FF_FFFF) | (ALT0 << 24) | (ALT0 << 27);
    mmio_write(GPFSEL4, gpfsel4_new);

    // GPIO 50-53 are in GPFSEL5 (bits 0-11)
    let gpfsel5 = mmio_read(GPFSEL5);
    let gpfsel5_new = (gpfsel5 & 0xFFFF_F000)
        | (ALT0 << 0)   // GPIO 50
        | (ALT0 << 3)   // GPIO 51
        | (ALT0 << 6)   // GPIO 52
        | (ALT0 << 9);  // GPIO 53
    mmio_write(GPFSEL5, gpfsel5_new);

    // Enable pull-ups on data lines (GPIO 49-53)
    mmio_write(GPPUD, GpioPull::Up as u32);
    delay_us(150);
    // GPIO 49-53 are bits 17-21 in GPPUDCLK1
    mmio_write(GPPUDCLK1, (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 21));
    delay_us(150);
    mmio_write(GPPUD, 0);
    mmio_write(GPPUDCLK1, 0);
}
