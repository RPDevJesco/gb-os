//! GPIO (General Purpose Input/Output) Driver
//!
//! Provides control over the BCM2710 GPIO pins for:
//! - Button input (GPi Case 2W controls)
//! - DPI display output (GPIO 0-21)
//! - UART pins (GPIO 14-15)
//! - LED control (ACT LED on GPIO 29)

use crate::mmio::{self, PERIPHERAL_BASE};

// ============================================================================
// Register Addresses
// ============================================================================

const GPIO_BASE: usize = PERIPHERAL_BASE + 0x0020_0000;

/// GPIO function select registers (3 bits per pin, 10 pins per register)
pub mod fsel {
    use super::GPIO_BASE;
    pub const GPFSEL0: usize = GPIO_BASE + 0x00;  // GPIO 0-9
    pub const GPFSEL1: usize = GPIO_BASE + 0x04;  // GPIO 10-19
    pub const GPFSEL2: usize = GPIO_BASE + 0x08;  // GPIO 20-29
    pub const GPFSEL3: usize = GPIO_BASE + 0x0C;  // GPIO 30-39
    pub const GPFSEL4: usize = GPIO_BASE + 0x10;  // GPIO 40-49
    pub const GPFSEL5: usize = GPIO_BASE + 0x14;  // GPIO 50-53
}

/// GPIO output set/clear registers
pub mod output {
    use super::GPIO_BASE;
    pub const GPSET0: usize = GPIO_BASE + 0x1C;   // Set GPIO 0-31
    pub const GPSET1: usize = GPIO_BASE + 0x20;   // Set GPIO 32-53
    pub const GPCLR0: usize = GPIO_BASE + 0x28;   // Clear GPIO 0-31
    pub const GPCLR1: usize = GPIO_BASE + 0x2C;   // Clear GPIO 32-53
}

/// GPIO level registers (read current state)
pub mod level {
    use super::GPIO_BASE;
    pub const GPLEV0: usize = GPIO_BASE + 0x34;   // Level GPIO 0-31
    pub const GPLEV1: usize = GPIO_BASE + 0x38;   // Level GPIO 32-53
}

/// GPIO pull-up/down control registers
pub mod pull {
    use super::GPIO_BASE;
    pub const GPPUD: usize = GPIO_BASE + 0x94;     // Pull-up/down enable
    pub const GPPUDCLK0: usize = GPIO_BASE + 0x98; // Clock GPIO 0-31
    pub const GPPUDCLK1: usize = GPIO_BASE + 0x9C; // Clock GPIO 32-53
}

// ============================================================================
// GPIO Function Codes
// ============================================================================

/// GPIO pin function modes
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Function {
    Input  = 0b000,
    Output = 0b001,
    Alt0   = 0b100,
    Alt1   = 0b101,
    Alt2   = 0b110,
    Alt3   = 0b111,
    Alt4   = 0b011,
    Alt5   = 0b010,
}

/// GPIO pull-up/down modes
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pull {
    Off  = 0,
    Down = 1,
    Up   = 2,
}

// ============================================================================
// GPIO Operations
// ============================================================================

/// Set the function of a GPIO pin.
///
/// # Arguments
/// * `pin` - GPIO pin number (0-53)
/// * `function` - Desired function mode
pub fn set_function(pin: u8, function: Function) {
    let reg = match pin {
        0..=9   => fsel::GPFSEL0,
        10..=19 => fsel::GPFSEL1,
        20..=29 => fsel::GPFSEL2,
        30..=39 => fsel::GPFSEL3,
        40..=49 => fsel::GPFSEL4,
        50..=53 => fsel::GPFSEL5,
        _ => return,
    };

    let shift = ((pin % 10) * 3) as u32;
    let mask = 0b111 << shift;
    let value = (function as u32) << shift;

    let current = mmio::read(reg);
    mmio::write(reg, (current & !mask) | value);
}

/// Set the pull-up/down mode for a GPIO pin.
///
/// # Arguments
/// * `pin` - GPIO pin number (0-53)
/// * `pull` - Pull-up/down mode
pub fn set_pull(pin: u8, pull: Pull) {
    // BCM2835/BCM2837 pull-up/down sequence:
    // 1. Write to GPPUD to set the control signal
    // 2. Wait 150 cycles
    // 3. Write to GPPUDCLKn to clock the control signal into the GPIO
    // 4. Wait 150 cycles
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

/// Read the current level of a GPIO pin.
///
/// # Returns
/// `true` if the pin is high, `false` if low.
pub fn read(pin: u8) -> bool {
    let (reg, bit) = if pin < 32 {
        (level::GPLEV0, 1u32 << pin)
    } else {
        (level::GPLEV1, 1u32 << (pin - 32))
    };

    (mmio::read(reg) & bit) != 0
}

/// Set a GPIO output pin high.
pub fn set_high(pin: u8) {
    let (reg, bit) = if pin < 32 {
        (output::GPSET0, 1u32 << pin)
    } else {
        (output::GPSET1, 1u32 << (pin - 32))
    };
    mmio::write(reg, bit);
}

/// Set a GPIO output pin low.
pub fn set_low(pin: u8) {
    let (reg, bit) = if pin < 32 {
        (output::GPCLR0, 1u32 << pin)
    } else {
        (output::GPCLR1, 1u32 << (pin - 32))
    };
    mmio::write(reg, bit);
}

/// Set a GPIO output pin to a specific level.
pub fn write(pin: u8, high: bool) {
    if high {
        set_high(pin);
    } else {
        set_low(pin);
    }
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
pub fn act_led_on() {
    set_low(ACT_LED_PIN);
}

/// Turn the ACT LED off.
pub fn act_led_off() {
    set_high(ACT_LED_PIN);
}

/// Toggle the ACT LED.
pub fn act_led_toggle() {
    if read(ACT_LED_PIN) {
        act_led_on();
    } else {
        act_led_off();
    }
}

// ============================================================================
// DPI Display Configuration (GPIO 0-21)
// ============================================================================

/// Configure GPIO 0-21 for DPI display output (ALT2 function).
///
/// Pin assignment for GPi Case 2W (18-bit BGR666):
/// - GPIO 0: PCLK (pixel clock)
/// - GPIO 1: DE (data enable)
/// - GPIO 2: VSYNC
/// - GPIO 3: HSYNC
/// - GPIO 4-9: Blue [2:7]
/// - GPIO 10-15: Green [2:7]
/// - GPIO 16-21: Red [2:7]
pub fn configure_for_dpi() {
    // Set all GPIO 0-21 to ALT2 function
    for pin in 0..=21 {
        set_function(pin, Function::Alt2);
    }

    // Disable pull-up/down on DPI pins
    for pin in 0..=21 {
        set_pull(pin, Pull::Off);
    }
}

// ============================================================================
// UART Configuration (GPIO 14-15)
// ============================================================================

/// Configure GPIO 14/15 for Mini UART (ALT5 function).
pub fn configure_for_uart() {
    set_function(14, Function::Alt5);  // TXD
    set_function(15, Function::Alt5);  // RXD
    set_pull(14, Pull::Off);
    set_pull(15, Pull::Up);
}
