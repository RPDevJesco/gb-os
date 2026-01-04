//! Platform abstraction traits.
//!
//! These traits define the interface that each platform must implement
//! to participate in the unified bootloader.

/// Result type for bootloader operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Bootloader error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Error {
    /// Hardware initialization failed
    InitFailed,
    /// Invalid configuration
    InvalidConfig,
    /// Timeout waiting for hardware
    Timeout,
    /// Buffer too small
    BufferTooSmall,
    /// Invalid address
    InvalidAddress,
    /// Device not ready
    NotReady,
    /// Checksum mismatch
    ChecksumError,
    /// Unknown error
    Unknown,
}

/// Serial/UART interface for debug output.
pub trait Serial {
    /// Initialize the serial interface with the given baud rate.
    fn init(&mut self, baud: u32) -> Result<()>;

    /// Write a single byte, blocking until complete.
    fn write_byte(&mut self, byte: u8);

    /// Read a single byte, blocking until available.
    fn read_byte(&mut self) -> u8;

    /// Check if data is available to read.
    fn data_available(&self) -> bool;

    /// Write a byte slice.
    fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_byte(b);
        }
    }

    /// Write a string.
    fn write_str(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
    }

    /// Write a string with newline.
    fn write_line(&mut self, s: &str) {
        self.write_str(s);
        self.write_byte(b'\r');
        self.write_byte(b'\n');
    }
}

/// GPIO pin interface.
pub trait Gpio {
    /// Pin identifier type.
    type Pin: Copy;

    /// Configure pin as output.
    fn set_output(&mut self, pin: Self::Pin);

    /// Configure pin as input.
    fn set_input(&mut self, pin: Self::Pin);

    /// Set pin high.
    fn set_high(&mut self, pin: Self::Pin);

    /// Set pin low.
    fn set_low(&mut self, pin: Self::Pin);

    /// Toggle pin state.
    fn toggle(&mut self, pin: Self::Pin);

    /// Read pin state.
    fn read(&self, pin: Self::Pin) -> bool;
}

/// Timer interface for delays.
pub trait Timer {
    /// Get current tick count (platform-specific resolution).
    fn ticks(&self) -> u64;

    /// Get tick frequency in Hz.
    fn frequency(&self) -> u64;

    /// Delay for specified microseconds.
    fn delay_us(&self, us: u64) {
        let start = self.ticks();
        let ticks_needed = (us * self.frequency()) / 1_000_000;
        while self.ticks().wrapping_sub(start) < ticks_needed {
            core::hint::spin_loop();
        }
    }

    /// Delay for specified milliseconds.
    fn delay_ms(&self, ms: u64) {
        self.delay_us(ms * 1000);
    }
}
