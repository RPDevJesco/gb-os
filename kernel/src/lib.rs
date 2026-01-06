//! GB-OS Kernel Library
//!
//! Exports shared modules for platform crates.
//! All modules use conditional compilation for platform-specific features.

#![no_std]

extern crate alloc;

/// Game Boy emulator core
pub mod gameboy;

/// GUI components (fonts, layout, framebuffer)
pub mod gui;

/// Pokemon game overlay system
pub mod overlay;
