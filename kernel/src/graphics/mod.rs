//! Graphics Module
//!
//! Provides VGA Mode 13h graphics support including:
//! - Low-level drawing primitives (vga_mode13h)
//! - Palette management (vga_palette)
//! - Double buffering with VSync (double_buffer) - NEW

pub mod vga_mode13h;
pub mod vga_palette;
pub mod double_buffer;
