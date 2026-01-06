//! GameBoy Emulator Module
//!
//! Integrates rboy GameBoy emulator with Rustacean OS kernel.
//! Uses existing kernel infrastructure (drivers, mm, arch).
//!
//! # Integration Points
//!
//! - **Input**: Uses `drivers::keyboard` for PS/2 input (x86 only)
//! - **Display**: Blits to VESA framebuffer via `gui::Framebuffer` or direct
//! - **Memory**: Uses kernel heap from `mm::heap`
//! - **Timing**: Uses PIT timer from `arch::x86::idt::ticks()`

extern crate alloc;

// Core emulator components (ported from rboy) - platform agnostic
pub mod cpu;
pub mod device;
pub mod gbmode;
pub mod gpu;
pub mod keypad;
pub mod mbc;
pub mod mmu;
pub mod register;
pub mod serial;
pub mod timer;

// Platform-specific integration layers
pub mod display;

// x86-specific keyboard input mapping (only compile on x86)
#[cfg(target_arch = "x86")]
pub mod input;

// Re-exports - core types available on all platforms
pub use device::Device;
pub use gpu::{SCREEN_H, SCREEN_W};
pub use keypad::KeypadKey;

// x86-specific re-exports
#[cfg(target_arch = "x86")]
pub use input::InputState;

/// Result type using static string errors
pub type StrResult<T> = Result<T, &'static str>;

/// Cycles per frame (70224 T-cycles at 4.19 MHz ≈ 59.7 fps)
pub const CYCLES_PER_FRAME: u32 = 70224;
