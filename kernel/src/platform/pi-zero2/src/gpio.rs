//! GPIO (General Purpose Input/Output) Driver
//!
//! Provides control over the BCM2710 GPIO pins for:
//! - Button input (GPi Case 2W controls)
//! - DPI display output (GPIO 0-21)
//! - UART pins (GPIO 14-15)
//! - SD card interface (GPIO 48-53)
//! - LED control (ACT LED on GPIO 29)
//!
//! # Pin Numbering
//!
//! This driver uses BCM GPIO numbers (0-53), not physical pin numbers.

use crate::mmio::{self, PERIPHERAL_BASE};

// ============================================================================
// Register Addresses
// ============================================================================

const GPIO_BASE: usize = PERIPHERAL_BASE + 0x0020_0000;

/// GPIO function select registers (3 bits per pin, 10 pins per register)
pub mod fsel {
    use super::GPIO_BASE;
    pub const GPFSEL0: usize = GPIO_BASE + 0x00; // GPIO 0-9
    pub const GPFSEL1: usize = GPIO_BASE + 0x04; // GPIO 10-19
    pub const GPFSEL2: usize = GPIO_BASE + 0x08; // GPIO 20-29
    pub const GPFSEL3: usize = GPIO_BASE + 0x0C; // GPIO 30-39
    pub const GPFSEL4: usize = GPIO_BASE + 0x10; // GPIO 40-49
    pub const GPFSEL5: usize = GPIO_BASE + 0x14; // GPIO 50-53
}

/// GPIO output set/clear registers
pub mod output {
    use super::GPIO_BASE;
    pub const GPSET0: usize = GPIO_BASE + 0x1C; // Set GPIO 0-31
    pub const GPSET1: usize = GPIO_BASE + 0x20; // Set GPIO 32-53
    pub const GPCLR0: usize = GPIO_BASE + 0x28; // Clear GPIO 0-31
    pub const GPCLR1: usize = GPIO_BASE + 0x2C; // Clear GPIO 32-53
}

/// GPIO level registers (read current state)
pub mod level {
    use super::GPIO_BASE;
    pub const GPLEV0: usize = GPIO_BASE + 0x34; // Level GPIO 0-31
    pub const GPLEV1: usize = GPIO_BASE + 0x38; // Level GPIO 32-53
}

/// GPIO pull-up/down control registers (active-low buttons need pull-ups)
pub mod pull {
    use super::GPIO_BASE;
    pub const GPPUD: usize = GPIO_BASE + 0x94;     // Pull-up/down enable
    pub const GPPUDCLK0: usize = GPIO_BASE + 0x98; // Clock GPIO 0-31
    pub const GPPUDCLK1: usize = GPIO_BASE + 0x9C; // Clock GPIO 32-53
}

// ============================================================================
// GPIO Function Codes
// ============================================================================

/// GPIO pin function modes.
///
/// Each GPIO can be configured for input, output, or one of 6 alternate functions.
/// The alternate function mapping varies by pin - see BCM2835 datasheet.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Function {
    Input = 0b000,
    Output = 0b001,
    Alt0 = 0b100,
    Alt1 = 0b101,
    Alt2 = 0b110,
    Alt3 = 0b111,
    Alt4 = 0b011,
    Alt5 = 0b010,
}

/// GPIO pull-up/down modes.
///
/// - `Off`: No pull resistor (floating input)
/// - `Down`: Pull to ground (reads low when not driven)
/// - `Up`: Pull to 3.3V (reads high when not driven)
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pull {
    Off = 0,
    Down = 1,
    Up = 2,
}

// ============================================================================
// Core GPIO Operations
// ============================================================================

/// Get the function select register and bit shift for a pin.
#[inline]
fn fsel_reg_and_shift(pin: u8) -> Option<(usize, u32)> {
    let reg = match pin {
        0..=9 => fsel::GPFSEL0,
        10..=19 => fsel::GPFSEL1,
        20..=29 => fsel::GPFSEL2,
        30..=39 => fsel::GPFSEL3,
        40..=49 => fsel::GPFSEL4,
        50..=53 => fsel::GPFSEL5,
        _ => return None,
    };
    let shift = ((pin % 10) * 3) as u32;
    Some((reg, shift))
}

/// Set the function of a GPIO pin.
///
/// # Arguments
/// * `pin` - GPIO pin number (0-53)
/// * `function` - Desired function mode
pub fn set_function(pin: u8, function: Function) {
    if let Some((reg, shift)) = fsel_reg_and_shift(pin) {
        let mask = 0b111 << shift;
        let value = (function as u32) << shift;
        let current = mmio::read(reg);
        mmio::write(reg, (current & !mask) | value);
    }
}

/// Get the current function of a GPIO pin.
pub fn get_function(pin: u8) -> Option<Function> {
    let (reg, shift) = fsel_reg_and_shift(pin)?;
    let value = (mmio::read(reg) >> shift) & 0b111;
    match value {
        0b000 => Some(Function::Input),
        0b001 => Some(Function::Output),
        0b100 => Some(Function::Alt0),
        0b101 => Some(Function::Alt1),
        0b110 => Some(Function::Alt2),
        0b111 => Some(Function::Alt3),
        0b011 => Some(Function::Alt4),
        0b010 => Some(Function::Alt5),
        _ => None,
    }
}

/// Set the pull-up/down mode for a GPIO pin.
///
/// # Arguments
/// * `pin` - GPIO pin number (0-53)
/// * `pull` - Pull-up/down mode
///
/// # Note
/// The BCM2835/BCM2837 requires a specific sequence with timing delays
/// to configure pull-up/down. This function handles that automatically.
pub fn set_pull(pin: u8, pull: Pull) {
    if pin > 53 {
        return;
    }

    // BCM2835/BCM2837 pull-up/down sequence:
    // 1. Write to GPPUD to set the control signal
    // 2. Wait 150 cycles for signal to settle
    // 3. Write to GPPUDCLKn to clock the control signal into the GPIO
    // 4. Wait 150 cycles for clock to propagate
    // 5. Write to GPPUD to remove the control signal
    // 6. Write to GPPUDCLKn to remove the clock

    mmio::write(pull::GPPUD, pull as u32);
    mmio::delay_cycles(150);

    let (clk_reg, bit) = if pin < 32 {
        (pull::GPPUDCLK0, 1u32 << pin)
    } else {
        (pull::GPPUDCLK1, 1u32 << (pin - 32))
    };

    mmio::write(clk_reg, bit);
    mmio::delay_cycles(150);

    mmio::write(pull::GPPUD, 0);
    mmio::write(clk_reg, 0);
}

/// Set pull-up/down for multiple pins at once (more efficient).
///
/// # Arguments
/// * `pins` - Slice of GPIO pin numbers
/// * `pull` - Pull-up/down mode to apply to all pins
pub fn set_pull_multi(pins: &[u8], pull: Pull) {
    if pins.is_empty() {
        return;
    }

    // Build masks for both clock registers
    let mut clk0_mask: u32 = 0;
    let mut clk1_mask: u32 = 0;

    for &pin in pins {
        if pin < 32 {
            clk0_mask |= 1 << pin;
        } else if pin < 54 {
            clk1_mask |= 1 << (pin - 32);
        }
    }

    // Apply sequence once for all pins
    mmio::write(pull::GPPUD, pull as u32);
    mmio::delay_cycles(150);

    if clk0_mask != 0 {
        mmio::write(pull::GPPUDCLK0, clk0_mask);
    }
    if clk1_mask != 0 {
        mmio::write(pull::GPPUDCLK1, clk1_mask);
    }
    mmio::delay_cycles(150);

    mmio::write(pull::GPPUD, 0);
    mmio::write(pull::GPPUDCLK0, 0);
    mmio::write(pull::GPPUDCLK1, 0);
}

/// Read the current level of a GPIO pin.
///
/// # Returns
/// `true` if the pin is high, `false` if low.
#[inline]
pub fn read(pin: u8) -> bool {
    let (reg, bit) = if pin < 32 {
        (level::GPLEV0, 1u32 << pin)
    } else if pin < 54 {
        (level::GPLEV1, 1u32 << (pin - 32))
    } else {
        return false;
    };

    (mmio::read(reg) & bit) != 0
}

/// Read all GPIO pins 0-31 at once.
#[inline]
pub fn read_all_low() -> u32 {
    mmio::read(level::GPLEV0)
}

/// Read all GPIO pins 32-53 at once.
#[inline]
pub fn read_all_high() -> u32 {
    mmio::read(level::GPLEV1)
}

/// Set a GPIO output pin high.
#[inline]
pub fn set_high(pin: u8) {
    let (reg, bit) = if pin < 32 {
        (output::GPSET0, 1u32 << pin)
    } else if pin < 54 {
        (output::GPSET1, 1u32 << (pin - 32))
    } else {
        return;
    };
    mmio::write(reg, bit);
}

/// Set a GPIO output pin low.
#[inline]
pub fn set_low(pin: u8) {
    let (reg, bit) = if pin < 32 {
        (output::GPCLR0, 1u32 << pin)
    } else if pin < 54 {
        (output::GPCLR1, 1u32 << (pin - 32))
    } else {
        return;
    };
    mmio::write(reg, bit);
}

/// Set a GPIO output pin to a specific level.
#[inline]
pub fn write(pin: u8, high: bool) {
    if high {
        set_high(pin);
    } else {
        set_low(pin);
    }
}

/// Toggle a GPIO output pin.
#[inline]
pub fn toggle(pin: u8) {
    write(pin, !read(pin));
}

// ============================================================================
// ACT LED (GPIO 29 active low on Pi Zero 2 W)
// ============================================================================

const ACT_LED_PIN: u8 = 29;

/// Initialize the ACT LED as an output.
pub fn init_act_led() {
    set_function(ACT_LED_PIN, Function::Output);
}

/// Turn the ACT LED on (active low).
#[inline]
pub fn act_led_on() {
    set_low(ACT_LED_PIN);
}

/// Turn the ACT LED off.
#[inline]
pub fn act_led_off() {
    set_high(ACT_LED_PIN);
}

/// Toggle the ACT LED.
#[inline]
pub fn act_led_toggle() {
    toggle(ACT_LED_PIN);
}

/// Set ACT LED state.
#[inline]
pub fn act_led_set(on: bool) {
    // Active low: on = low, off = high
    write(ACT_LED_PIN, !on);
}

// ============================================================================
// DPI Display Configuration (GPIO 0-21)
// ============================================================================

/// DPI pin assignments for GPi Case 2W (18-bit BGR666).
pub mod dpi {
    pub const PCLK: u8 = 0;   // Pixel clock
    pub const DE: u8 = 1;     // Data enable
    pub const VSYNC: u8 = 2;  // Vertical sync
    pub const HSYNC: u8 = 3;  // Horizontal sync
    // GPIO 4-9: Blue [2:7]
    // GPIO 10-15: Green [2:7]
    // GPIO 16-21: Red [2:7]
}

/// Configure GPIO 0-21 for DPI display output (ALT2 function).
///
/// This sets up all 22 pins needed for DPI video output:
/// - GPIO 0: PCLK (pixel clock)
/// - GPIO 1: DE (data enable)
/// - GPIO 2: VSYNC
/// - GPIO 3: HSYNC
/// - GPIO 4-9: Blue [2:7]
/// - GPIO 10-15: Green [2:7]
/// - GPIO 16-21: Red [2:7]
pub fn configure_for_dpi() {
    // Set all GPIO 0-21 to ALT2 function and disable pulls in one pass
    for pin in 0..=21 {
        set_function(pin, Function::Alt2);
    }

    // Disable pull-up/down on all DPI pins at once
    let dpi_pins: [u8; 22] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
    ];
    set_pull_multi(&dpi_pins, Pull::Off);
}

/// Configure DPI with optimized register writes (slightly faster boot).
///
/// This precomputes the register values instead of setting pins individually.
/// Matches the original working implementation exactly.
pub fn configure_for_dpi_fast() {
    const ALT2: u32 = 0b110;

    // GPFSEL0: GPIO 0-9, all ALT2
    let gpfsel0_val: u32 = (ALT2 << 0)
        | (ALT2 << 3)
        | (ALT2 << 6)
        | (ALT2 << 9)
        | (ALT2 << 12)
        | (ALT2 << 15)
        | (ALT2 << 18)
        | (ALT2 << 21)
        | (ALT2 << 24)
        | (ALT2 << 27);

    // GPFSEL1: GPIO 10-19, all ALT2
    let gpfsel1_val: u32 = (ALT2 << 0)
        | (ALT2 << 3)
        | (ALT2 << 6)
        | (ALT2 << 9)
        | (ALT2 << 12)
        | (ALT2 << 15)
        | (ALT2 << 18)
        | (ALT2 << 21)
        | (ALT2 << 24)
        | (ALT2 << 27);

    // GPFSEL2: GPIO 20-21 = ALT2, preserve GPIO 22-29
    let gpfsel2_current = mmio::read(fsel::GPFSEL2);
    let gpfsel2_val: u32 = (ALT2 << 0) | (ALT2 << 3);

    mmio::write(fsel::GPFSEL0, gpfsel0_val);
    mmio::write(fsel::GPFSEL1, gpfsel1_val);
    mmio::write(fsel::GPFSEL2, (gpfsel2_current & 0xFFFFFFC0) | gpfsel2_val);

    // Disable pulls - NO DELAYS like original
    mmio::write(pull::GPPUD, 0);
    mmio::write(pull::GPPUDCLK0, 0x003F_FFFF);
    mmio::write(pull::GPPUD, 0);
    mmio::write(pull::GPPUDCLK0, 0);
}

// ============================================================================
// SD Card Configuration (SDHOST - GPIO 48-53)
// ============================================================================

/// SD card pin assignments (SDHOST controller).
pub mod sdhost {
    pub const CLK: u8 = 48;  // SD clock
    pub const CMD: u8 = 49;  // SD command
    pub const DAT0: u8 = 50; // SD data 0
    pub const DAT1: u8 = 51; // SD data 1
    pub const DAT2: u8 = 52; // SD data 2
    pub const DAT3: u8 = 53; // SD data 3
}

/// Configure GPIO 48-53 for SD card interface (SDHOST, ALT0 function).
///
/// This configures:
/// - GPIO 48: SD CLK
/// - GPIO 49: SD CMD (needs pull-up)
/// - GPIO 50-53: SD DAT0-DAT3 (need pull-ups)
pub fn configure_for_sd() {
    // Set all SD pins to ALT0
    for pin in 48..=53 {
        set_function(pin, Function::Alt0);
    }

    // CMD and DAT lines need pull-ups for proper operation
    // CLK (GPIO 48) should have no pull
    set_pull(sdhost::CLK, Pull::Off);

    // Apply pull-ups to CMD and all data lines
    let sd_pullup_pins: [u8; 5] = [
        sdhost::CMD,
        sdhost::DAT0,
        sdhost::DAT1,
        sdhost::DAT2,
        sdhost::DAT3,
    ];
    set_pull_multi(&sd_pullup_pins, Pull::Up);
}

/// Configure SD card with optimized register writes.
pub fn configure_for_sd_fast() {
    const ALT0: u32 = 0b100;

    // GPFSEL4: GPIO 48-49 (bits 24-29)
    let gpfsel4 = mmio::read(fsel::GPFSEL4);
    let gpfsel4_new = (gpfsel4 & 0xC0FF_FFFF) | (ALT0 << 24) | (ALT0 << 27);
    mmio::write(fsel::GPFSEL4, gpfsel4_new);

    // GPFSEL5: GPIO 50-53 (bits 0-11)
    let gpfsel5 = mmio::read(fsel::GPFSEL5);
    let gpfsel5_new = (gpfsel5 & 0xFFFF_F000) | (ALT0 << 0) | (ALT0 << 3) | (ALT0 << 6) | (ALT0 << 9);
    mmio::write(fsel::GPFSEL5, gpfsel5_new);

    // Pull-ups on CMD and DAT lines (GPIO 49-53)
    mmio::write(pull::GPPUD, Pull::Up as u32);
    mmio::delay_cycles(150);
    // GPIO 49-53 are bits 17-21 in GPPUDCLK1
    mmio::write(pull::GPPUDCLK1, 0b0011_1110_0000_0000_0000);
    mmio::delay_cycles(150);
    mmio::write(pull::GPPUD, 0);
    mmio::write(pull::GPPUDCLK1, 0);
}

// ============================================================================
// UART Configuration (GPIO 14-15)
// ============================================================================

/// UART pin assignments.
pub mod uart {
    pub const TXD: u8 = 14; // Transmit
    pub const RXD: u8 = 15; // Receive
}

/// Configure GPIO 14/15 for Mini UART (ALT5 function).
pub fn configure_for_uart() {
    set_function(uart::TXD, Function::Alt5);
    set_function(uart::RXD, Function::Alt5);
    set_pull(uart::TXD, Pull::Off);
    set_pull(uart::RXD, Pull::Up); // RXD needs pull-up when idle
}

/// Configure GPIO 14/15 for PL011 UART (ALT0 function).
pub fn configure_for_uart_pl011() {
    set_function(uart::TXD, Function::Alt0);
    set_function(uart::RXD, Function::Alt0);
    set_pull(uart::TXD, Pull::Off);
    set_pull(uart::RXD, Pull::Up);
}

// ============================================================================
// I2C Configuration (GPIO 2-3)
// ============================================================================

/// I2C pin assignments (I2C1).
pub mod i2c {
    pub const SDA: u8 = 2; // Data
    pub const SCL: u8 = 3; // Clock
}

/// Configure GPIO 2/3 for I2C1 (ALT0 function).
pub fn configure_for_i2c() {
    set_function(i2c::SDA, Function::Alt0);
    set_function(i2c::SCL, Function::Alt0);
    // I2C lines need pull-ups (often external, but internal helps)
    set_pull(i2c::SDA, Pull::Up);
    set_pull(i2c::SCL, Pull::Up);
}

// ============================================================================
// SPI Configuration (GPIO 7-11)
// ============================================================================

/// SPI0 pin assignments.
pub mod spi {
    pub const CE1: u8 = 7;   // Chip enable 1
    pub const CE0: u8 = 8;   // Chip enable 0
    pub const MISO: u8 = 9;  // Master in, slave out
    pub const MOSI: u8 = 10; // Master out, slave in
    pub const SCLK: u8 = 11; // Clock
}

/// Configure GPIO 7-11 for SPI0 (ALT0 function).
pub fn configure_for_spi() {
    for &pin in &[spi::CE1, spi::CE0, spi::MISO, spi::MOSI, spi::SCLK] {
        set_function(pin, Function::Alt0);
        set_pull(pin, Pull::Off);
    }
}
