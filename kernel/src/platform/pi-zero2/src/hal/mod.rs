//! Hardware Abstraction Layer for GB-OS
//!
//! This module provides platform-agnostic traits that abstract the differences
//! between x86 (VGA, ATA, PS/2) and ARM (DPI, EMMC, GPIO) hardware.
//!
//! The existing `gameboy`, `overlay`, `rom_browser`, and `storage` modules
//! use these traits instead of directly accessing hardware.

pub mod display;
pub mod input;
pub mod storage;
pub mod timer;

pub use display::{Display, DisplayInfo, PixelFormat};
pub use input::{InputDevice, ButtonState, GameBoyButton};
pub use storage::{BlockDevice, Filesystem};
pub use timer::Timer;

/// Platform initialization trait
pub trait Platform {
    type Display: Display;
    type Input: InputDevice;
    type Storage: BlockDevice;
    type Timer: Timer;
    
    /// Initialize the platform hardware
    fn init() -> Self;
    
    /// Get display device
    fn display(&mut self) -> &mut Self::Display;
    
    /// Get input device
    fn input(&mut self) -> &mut Self::Input;
    
    /// Get storage device
    fn storage(&mut self) -> &mut Self::Storage;
    
    /// Get timer
    fn timer(&self) -> &Self::Timer;
}
