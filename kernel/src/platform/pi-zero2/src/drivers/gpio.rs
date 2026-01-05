//! GPIO Driver for BCM2835/BCM2837
//!
//! Provides low-level GPIO access for:
//! - DPI display pins (0-21, configured as ALT2)
//! - Button input pins (various, configured as inputs with pull-ups)

// ============================================================================
// BCM2835 GPIO Registers
// ============================================================================

const PERIPHERAL_BASE: usize = 0x3F00_0000;  // BCM2837 on Pi Zero 2W
const GPIO_BASE: usize = PERIPHERAL_BASE + 0x20_0000;

// Register offsets
const GPFSEL0: usize = 0x00;   // Function select 0 (pins 0-9)
const GPFSEL1: usize = 0x04;   // Function select 1 (pins 10-19)
const GPFSEL2: usize = 0x08;   // Function select 2 (pins 20-29)
const GPFSEL3: usize = 0x0C;   // Function select 3 (pins 30-39)
const GPFSEL4: usize = 0x10;   // Function select 4 (pins 40-49)
const GPFSEL5: usize = 0x14;   // Function select 5 (pins 50-53)

const GPSET0: usize = 0x1C;    // Output set 0
const GPSET1: usize = 0x20;    // Output set 1

const GPCLR0: usize = 0x28;    // Output clear 0
const GPCLR1: usize = 0x2C;    // Output clear 1

const GPLEV0: usize = 0x34;    // Pin level 0
const GPLEV1: usize = 0x38;    // Pin level 1

const GPEDS0: usize = 0x40;    // Event detect status 0
const GPEDS1: usize = 0x44;    // Event detect status 1

const GPPUD: usize = 0x94;     // Pull-up/down enable
const GPPUDCLK0: usize = 0x98; // Pull-up/down enable clock 0
const GPPUDCLK1: usize = 0x9C; // Pull-up/down enable clock 1

// ============================================================================
// GPIO Functions
// ============================================================================

/// GPIO pin function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GpioFunction {
    Input = 0b000,
    Output = 0b001,
    Alt0 = 0b100,
    Alt1 = 0b101,
    Alt2 = 0b110,
    Alt3 = 0b111,
    Alt4 = 0b011,
    Alt5 = 0b010,
}

/// GPIO pull-up/down configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GpioPull {
    Off = 0b00,
    Down = 0b01,
    Up = 0b10,
}

// ============================================================================
// Low-level Register Access
// ============================================================================

#[inline]
fn read_reg(offset: usize) -> u32 {
    unsafe {
        core::ptr::read_volatile((GPIO_BASE + offset) as *const u32)
    }
}

#[inline]
fn write_reg(offset: usize, value: u32) {
    unsafe {
        core::ptr::write_volatile((GPIO_BASE + offset) as *mut u32, value);
    }
}

fn delay(count: u32) {
    for _ in 0..count {
        unsafe { core::arch::asm!("nop"); }
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Set GPIO pin function
pub fn set_function(pin: u8, function: GpioFunction) {
    let reg_offset = match pin {
        0..=9 => GPFSEL0,
        10..=19 => GPFSEL1,
        20..=29 => GPFSEL2,
        30..=39 => GPFSEL3,
        40..=49 => GPFSEL4,
        50..=53 => GPFSEL5,
        _ => return,
    };
    
    let bit_offset = ((pin % 10) * 3) as usize;
    let mask = !(0b111 << bit_offset);
    
    let mut value = read_reg(reg_offset);
    value &= mask;
    value |= (function as u32) << bit_offset;
    write_reg(reg_offset, value);
}

/// Set GPIO pull-up/down resistor
pub fn set_pull(pin: u8, pull: GpioPull) {
    // 1. Write to GPPUD to set the required control signal
    write_reg(GPPUD, pull as u32);
    
    // 2. Wait 150 cycles
    delay(150);
    
    // 3. Write to GPPUDCLK0/1 to clock the control signal into the GPIO pads
    let clk_reg = if pin < 32 { GPPUDCLK0 } else { GPPUDCLK1 };
    let bit = 1u32 << (pin % 32);
    write_reg(clk_reg, bit);
    
    // 4. Wait 150 cycles
    delay(150);
    
    // 5. Clear GPPUD and GPPUDCLK
    write_reg(GPPUD, 0);
    write_reg(clk_reg, 0);
}

/// Read GPIO pin level
pub fn read_pin(pin: u8) -> bool {
    let reg = if pin < 32 { GPLEV0 } else { GPLEV1 };
    let bit = 1u32 << (pin % 32);
    (read_reg(reg) & bit) != 0
}

/// Set GPIO output high
pub fn set_high(pin: u8) {
    let reg = if pin < 32 { GPSET0 } else { GPSET1 };
    let bit = 1u32 << (pin % 32);
    write_reg(reg, bit);
}

/// Set GPIO output low
pub fn set_low(pin: u8) {
    let reg = if pin < 32 { GPCLR0 } else { GPCLR1 };
    let bit = 1u32 << (pin % 32);
    write_reg(reg, bit);
}

/// Write GPIO output
pub fn write_pin(pin: u8, high: bool) {
    if high {
        set_high(pin);
    } else {
        set_low(pin);
    }
}

// ============================================================================
// DPI Configuration
// ============================================================================

/// Configure GPIO 0-21 for DPI display output (ALT2 function)
/// 
/// Pin assignments for GPi Case 2W DPI:
/// - GPIO 0:  PCLK (Pixel Clock)
/// - GPIO 1:  DE (Data Enable)  
/// - GPIO 2:  VSYNC
/// - GPIO 3:  HSYNC
/// - GPIO 4-9:   Blue [2:7]
/// - GPIO 10-15: Green [2:7]
/// - GPIO 16-21: Red [2:7]
pub fn configure_dpi() {
    // Set all DPI pins to ALT2
    for pin in 0..=21 {
        set_function(pin, GpioFunction::Alt2);
    }
}

// ============================================================================
// Button Input Configuration
// ============================================================================

/// Configure a GPIO pin as button input with pull-up
pub fn configure_button(pin: u8) {
    set_function(pin, GpioFunction::Input);
    set_pull(pin, GpioPull::Up);
}

/// Read all button pins and return as bitfield
/// Buttons are active-low, so inverted for natural logic
pub fn read_buttons(pins: &[u8]) -> u32 {
    let mut state = 0u32;
    
    for (i, &pin) in pins.iter().enumerate() {
        if !read_pin(pin) {  // Active low, so !pin means pressed
            state |= 1 << i;
        }
    }
    
    state
}
