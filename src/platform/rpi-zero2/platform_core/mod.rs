//! Low-Level Core Modules
//!
//! Foundation layer with no dependencies on other platform modules.
//! Provides memory-mapped I/O, CPU control, and memory management.

pub mod mmio;
pub mod cpu;
pub mod mmu;
pub mod allocator;

// Re-exports for convenience
pub use mmio::{mmio_read, mmio_write, delay_ms, delay_us, micros};
pub use mmio::{dmb, dsb, isb, sev, wfe};
pub use mmio::PERIPHERAL_BASE;
pub use cpu::{init_mmu, check_caches, get_exception_level};
pub use cpu::{enable_icache};
