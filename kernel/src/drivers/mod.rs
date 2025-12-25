//! Hardware Drivers
//!
//! Device drivers for Rustacean OS.
//!
//! Driver initialization is handled through EventChains for fault-tolerant
//! loading with graceful degradation when optional drivers fail.

pub mod armada_e500_hw;
pub mod keyboard;

// Re-export common driver types
pub use armada_e500_hw as hw;