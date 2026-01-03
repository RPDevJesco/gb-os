//! GPIO driver for BCM2710/BCM2837.
//!
//! Pin functions, pull-up/down configuration, and basic I/O.

use crate::mmio;
use crate::PERIPHERAL_BASE;

/// GPIO register base address.
const GPIO_BASE: usize = PERIPHERAL_BASE + 0x0020_0000;

/// GPIO registers.
mod regs {
    use super::GPIO_BASE;

    /// Function select registers (3 bits per pin, 10 pins per register).
    pub const GPFSEL0: usize = GPIO_BASE + 0x00;
    pub const GPFSEL1: usize = GPIO_BASE + 0x04;
    pub const GPFSEL2: usize = GPIO_BASE + 0x08;
    pub const GPFSEL3: usize = GPIO_BASE + 0x0C;
    pub const GPFSEL4: usize = GPIO_BASE + 0x10;
    pub const GPFSEL5: usize = GPIO_BASE + 0x14;

    /// Pin output set registers.
    pub const GPSET0: usize = GPIO_BASE + 0x1C;
    pub const GPSET1: usize = GPIO_BASE + 0x20;

    /// Pin output clear registers.
    pub const GPCLR0: usize = GPIO_BASE + 0x28;
    pub const GPCLR1: usize = GPIO_BASE + 0x2C;

    /// Pin level registers.
    pub const GPLEV0: usize = GPIO_BASE + 0x34;
    pub const GPLEV1: usize = GPIO_BASE + 0x38;

    /// Pull-up/down control.
    pub const GPPUD: usize = GPIO_BASE + 0x94;
    pub const GPPUDCLK0: usize = GPIO_BASE + 0x98;
    pub const GPPUDCLK1: usize = GPIO_BASE + 0x9C;
}

/// GPIO pin function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
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

/// Pull-up/down configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Pull {
    None = 0b00,
    Down = 0b01,
    Up = 0b10,
}

/// GPIO driver.
pub struct Gpio;

impl Gpio {
    /// Create a new GPIO driver instance.
    pub const fn new() -> Self {
        Self
    }

    /// Set the function of a GPIO pin.
    pub fn set_function(&self, pin: u8, function: Function) {
        assert!(pin < 54);

        let reg = match pin / 10 {
            0 => regs::GPFSEL0,
            1 => regs::GPFSEL1,
            2 => regs::GPFSEL2,
            3 => regs::GPFSEL3,
            4 => regs::GPFSEL4,
            5 => regs::GPFSEL5,
            _ => unreachable!(),
        };

        let shift = (pin % 10) * 3;
        let mask = 0b111 << shift;
        let value = (function as u32) << shift;

        mmio::modify(reg, mask, value);
    }

    /// Set the pull-up/down configuration for a pin.
    pub fn set_pull(&self, pin: u8, pull: Pull) {
        assert!(pin < 54);

        // BCM2835/BCM2710 pull-up/down sequence:
        // 1. Write to GPPUD to set the required control signal
        // 2. Wait 150 cycles
        // 3. Write to GPPUDCLK0/1 to clock the control signal into the pin
        // 4. Wait 150 cycles
        // 5. Write to GPPUD to remove the control signal
        // 6. Write to GPPUDCLK0/1 to remove the clock

        mmio::write(regs::GPPUD, pull as u32);
        mmio::delay(150);

        let clk_reg = if pin < 32 {
            regs::GPPUDCLK0
        } else {
            regs::GPPUDCLK1
        };
        let bit = 1u32 << (pin % 32);

        mmio::write(clk_reg, bit);
        mmio::delay(150);

        mmio::write(regs::GPPUD, 0);
        mmio::write(clk_reg, 0);
    }

    /// Set a pin high.
    pub fn set_high(&self, pin: u8) {
        assert!(pin < 54);

        let reg = if pin < 32 {
            regs::GPSET0
        } else {
            regs::GPSET1
        };
        mmio::write(reg, 1 << (pin % 32));
    }

    /// Set a pin low.
    pub fn set_low(&self, pin: u8) {
        assert!(pin < 54);

        let reg = if pin < 32 {
            regs::GPCLR0
        } else {
            regs::GPCLR1
        };
        mmio::write(reg, 1 << (pin % 32));
    }

    /// Read a pin level.
    pub fn read(&self, pin: u8) -> bool {
        assert!(pin < 54);

        let reg = if pin < 32 {
            regs::GPLEV0
        } else {
            regs::GPLEV1
        };
        (mmio::read(reg) & (1 << (pin % 32))) != 0
    }

    /// Toggle a pin.
    pub fn toggle(&self, pin: u8) {
        if self.read(pin) {
            self.set_low(pin);
        } else {
            self.set_high(pin);
        }
    }
}
