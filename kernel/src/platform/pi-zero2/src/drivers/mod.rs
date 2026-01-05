//! Hardware Drivers for Pi Zero 2W / GPi Case 2W
//!
//! Platform-specific drivers for BCM2837 peripherals.

pub mod emmc;
pub mod gpio;
pub mod timer;
pub mod dpi;

// Re-exports for convenience
pub use timer::{get_timer, delay_us, delay_ms, micros};
pub use gpio::{set_function, read_pin, write_pin, GpioFunction, GpioPull};
pub use emmc::{get_emmc, init as emmc_init};
