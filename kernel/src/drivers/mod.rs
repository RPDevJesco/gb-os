//! Hardware Drivers
//!
//! Device drivers for Rustacean OS.
//!
//! Driver initialization is handled through EventChains for fault-tolerant
//! loading with graceful degradation when optional drivers fail.

pub mod vga;
pub mod keyboard;
pub mod mouse;
pub mod ati_rage;
pub mod synaptics;
pub mod init;

// Re-export common driver types
pub use ati_rage::AtiRage;
pub use synaptics::SynapticsTouchpad;
pub use init::{init_all_drivers, DriverInitResult, gpu_type, input_type};
