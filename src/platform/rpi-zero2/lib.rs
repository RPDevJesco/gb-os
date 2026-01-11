//! GB-OS for Raspberry Pi Zero 2W / GPi Case 2W
//!
//! A bare-metal GameBoy emulator that boots directly on Raspberry Pi Zero 2W.
//!
//! # Module Organization
//!
//! The platform is organized into layers with clear dependencies:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                    Application                       │
//! │              (main.rs - emulator loop)              │
//! ├─────────────────────────────────────────────────────┤
//! │                    Subsystems                        │
//! │              fat32, input                           │
//! ├─────────────────────────────────────────────────────┤
//! │        Drivers              │        Display         │
//! │      sdhost, usb            │  framebuffer, console  │
//! ├─────────────────────────────────────────────────────┤
//! │                      HAL                             │
//! │                 gpio, mailbox                        │
//! ├─────────────────────────────────────────────────────┤
//! │                      Core                            │
//! │            mmio, cpu, mmu, allocator                │
//! └─────────────────────────────────────────────────────┘
//! ```

#![no_std]
#![allow(dead_code)]
#![allow(unused_variables)]

extern crate alloc;

// ============================================================================
// Module Hierarchy
// ============================================================================

/// Low-level core: MMIO, CPU control, MMU, allocator
pub mod core;

/// Hardware abstraction: GPIO, mailbox
pub mod hal;

/// Display: framebuffer, console, splash screen
pub mod display;

/// Hardware drivers: SD card, USB
pub mod drivers;

/// Subsystems: FAT32 filesystem, input handling
pub mod subsystems;

pub mod viewer;

// ============================================================================
// Re-exports for Convenience
// ============================================================================
//
// These provide backwards-compatible access and convenient imports.
// Users can either:
//   - Use the layered path: `use crate::core::mmio_read`
//   - Use the flat re-export: `use crate::mmio_read`

// Core
pub use platform_core::{mmio_read, mmio_write, delay_ms, delay_us, micros};
pub use platform_core::{dmb, dsb, isb, sev, wfe};
pub use platform_core::{init_mmu, check_caches, get_exception_level};

// HAL
pub use hal::{configure_for_dpi, configure_for_sd};
pub use hal::{mailbox_call, MailboxBuffer, set_power_state};

// Display
pub use display::{Framebuffer, color, GB_PALETTE};
pub use display::{SCREEN_WIDTH, SCREEN_HEIGHT};
pub use display::{GB_WIDTH, GB_HEIGHT, GB_SCALE, GB_OFFSET_X, GB_OFFSET_Y};
pub use display::{Console, StringWriter, draw_char, draw_string, draw_centered};

// Drivers
pub use drivers::SdCard;
pub use drivers::{UsbHost, Xbox360InputReport};

// Subsystems
pub use subsystems::Fat32;
pub use subsystems::{GpiButtonState, GbJoypad, button};
