//! Mini UART driver for BCM2710/BCM2837.
//!
//! The Mini UART is simpler than the PL011 and shares pins with
//! Bluetooth on Pi 3/Zero2W. For a bootloader, it's often easier
//! to use since it doesn't require mailbox configuration.
//!
//! Default pins: GPIO14 (TXD), GPIO15 (RXD)

use crate::gpio::{Function, Gpio, Pull};
use crate::mmio;
use crate::PERIPHERAL_BASE;
use bootcore::{Result, Serial};

/// AUX peripheral base (contains Mini UART, SPI1, SPI2).
const AUX_BASE: usize = PERIPHERAL_BASE + 0x0021_5000;

/// Mini UART registers.
mod regs {
    use super::AUX_BASE;

    /// Auxiliary enables.
    pub const AUX_ENABLES: usize = AUX_BASE + 0x04;

    /// Mini UART I/O data.
    pub const AUX_MU_IO: usize = AUX_BASE + 0x40;

    /// Mini UART interrupt enable.
    pub const AUX_MU_IER: usize = AUX_BASE + 0x44;

    /// Mini UART interrupt identify.
    pub const AUX_MU_IIR: usize = AUX_BASE + 0x48;

    /// Mini UART line control.
    pub const AUX_MU_LCR: usize = AUX_BASE + 0x4C;

    /// Mini UART modem control.
    pub const AUX_MU_MCR: usize = AUX_BASE + 0x50;

    /// Mini UART line status.
    pub const AUX_MU_LSR: usize = AUX_BASE + 0x54;

    /// Mini UART modem status.
    pub const AUX_MU_MSR: usize = AUX_BASE + 0x58;

    /// Mini UART scratch.
    pub const AUX_MU_SCRATCH: usize = AUX_BASE + 0x5C;

    /// Mini UART extra control.
    pub const AUX_MU_CNTL: usize = AUX_BASE + 0x60;

    /// Mini UART extra status.
    pub const AUX_MU_STAT: usize = AUX_BASE + 0x64;

    /// Mini UART baud rate.
    pub const AUX_MU_BAUD: usize = AUX_BASE + 0x68;
}

/// Line status register bits.
mod lsr {
    pub const DATA_READY: u32 = 1 << 0;
    pub const TX_EMPTY: u32 = 1 << 5;
}

/// Mini UART pins.
const TX_PIN: u8 = 14;
const RX_PIN: u8 = 15;

/// System clock frequency for Pi Zero 2 W / Pi 3.
/// This is the VPU clock, typically 250 MHz.
const SYSTEM_CLOCK: u32 = 250_000_000;

/// Mini UART driver.
pub struct MiniUart {
    initialized: bool,
}

impl MiniUart {
    /// Create a new Mini UART instance (not yet initialized).
    pub const fn new() -> Self {
        Self { initialized: false }
    }

    /// Calculate baud rate divisor.
    /// Formula: baudrate_reg = (system_clock / (8 * baud)) - 1
    fn calculate_divisor(baud: u32) -> u32 {
        (SYSTEM_CLOCK / (8 * baud)) - 1
    }

    /// Check if transmit FIFO has space.
    fn tx_ready(&self) -> bool {
        (mmio::read(regs::AUX_MU_LSR) & lsr::TX_EMPTY) != 0
    }

    /// Check if receive FIFO has data.
    fn rx_ready(&self) -> bool {
        (mmio::read(regs::AUX_MU_LSR) & lsr::DATA_READY) != 0
    }
}

impl Serial for MiniUart {
    fn init(&mut self, baud: u32) -> Result<()> {
        let gpio = Gpio::new();

        // Enable Mini UART (AUX_ENABLES bit 0)
        mmio::set_bits(regs::AUX_ENABLES, 1);

        // Disable TX and RX while configuring
        mmio::write(regs::AUX_MU_CNTL, 0);

        // Disable interrupts
        mmio::write(regs::AUX_MU_IER, 0);

        // Set 8-bit mode (LCR bits [1:0] = 0b11)
        // Note: BCM2835 manual is wrong, bit 1 enables 8-bit mode
        mmio::write(regs::AUX_MU_LCR, 3);

        // Clear RTS (not used)
        mmio::write(regs::AUX_MU_MCR, 0);

        // Clear FIFOs
        mmio::write(regs::AUX_MU_IIR, 0xC6);

        // Set baud rate
        let divisor = Self::calculate_divisor(baud);
        mmio::write(regs::AUX_MU_BAUD, divisor);

        // Configure GPIO pins
        // GPIO14 = TXD1 (Alt5), GPIO15 = RXD1 (Alt5)
        gpio.set_pull(TX_PIN, Pull::None);
        gpio.set_pull(RX_PIN, Pull::Up); // Pull-up on RX to avoid noise

        gpio.set_function(TX_PIN, Function::Alt5);
        gpio.set_function(RX_PIN, Function::Alt5);

        // Enable TX and RX
        mmio::write(regs::AUX_MU_CNTL, 3);

        self.initialized = true;
        Ok(())
    }

    fn write_byte(&mut self, byte: u8) {
        // Wait for TX FIFO to have space
        while !self.tx_ready() {
            core::hint::spin_loop();
        }
        mmio::write(regs::AUX_MU_IO, byte as u32);
    }

    fn read_byte(&mut self) -> u8 {
        // Wait for RX FIFO to have data
        while !self.rx_ready() {
            core::hint::spin_loop();
        }
        (mmio::read(regs::AUX_MU_IO) & 0xFF) as u8
    }

    fn data_available(&self) -> bool {
        self.rx_ready()
    }
}
