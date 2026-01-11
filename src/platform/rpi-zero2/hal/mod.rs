//! Hardware Abstraction Layer
//!
//! Provides GPIO configuration and VideoCore mailbox communication.
//! Depends on: core

pub mod gpio;
pub mod mailbox;

// Re-exports for convenience
pub use gpio::{configure_for_dpi, configure_for_sd};
pub use gpio::{set_pin_function, set_pin_pull, GpioFunction, GpioPull};
pub use mailbox::{mailbox_call, MailboxBuffer, set_power_state, device};
