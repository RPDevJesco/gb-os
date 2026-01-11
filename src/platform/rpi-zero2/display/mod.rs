//! Display Modules
//!
//! Framebuffer management, text console, and splash screen.
//! Depends on: core, hal

pub mod framebuffer;
pub mod console;
pub mod splash_screen;

// Re-exports for convenience
pub use framebuffer::{Framebuffer, color, GB_PALETTE};
pub use framebuffer::{SCREEN_WIDTH, SCREEN_HEIGHT};
pub use framebuffer::{GB_WIDTH, GB_HEIGHT, GB_SCALE, GB_OFFSET_X, GB_OFFSET_Y};
pub use console::{Console, StringWriter, draw_centered};
