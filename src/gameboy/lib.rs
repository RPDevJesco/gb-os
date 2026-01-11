//! # GB-OS Core - Zero-Dependency Game Boy/Color Emulator
//!
//! A high-performance, `no_std` compatible Game Boy and Game Boy Color emulator core.
//! All platform-specific functionality is abstracted through traits that must be
//! implemented by the host platform.
//!
//! ## Platform Traits
//!
//! - [`AudioOutput`] - Audio sample output
//! - [`TimeSource`] - Real-time clock for MBC3 cartridges
//! - [`SerialLink`] - Serial port communication (Game Boy Printer, link cable)
//!
//! ## Usage
//!
//! ```ignore
//! use gameboy::{Emulator, EmulatorConfig, KeypadKey};
//!
//! // Create emulator with ROM data
//! let mut emu = Emulator::new(&rom_data, EmulatorConfig::default()).unwrap();
//!
//! // Main loop
//! loop {
//!     let cycles = emu.step();
//!     
//!     if emu.frame_ready() {
//!         let framebuffer = emu.framebuffer();
//!         // Render framebuffer...
//!     }
//! }
//! ```

#![no_std]
#![allow(clippy::new_without_default)]

// We need alloc for Vec in some places, but no std
extern crate alloc;

pub mod audio;
pub mod cartridge;
pub mod cpu;
pub mod gbmode;
pub mod gpu;
pub mod keypad;
pub mod mmu;
pub mod register;
pub mod serial;
pub mod sound;
pub mod timer;

// Re-exports for convenience
pub use audio::{AudioOutput, AudioResampler, NullAudio};
pub use cartridge::{Cartridge, CartridgeRam, RomInfo};
pub use gbmode::GbMode;
pub use gpu::{SCREEN_H, SCREEN_W};
pub use keypad::KeypadKey;
pub use serial::SerialLink;

use alloc::boxed::Box;
use alloc::vec::Vec;
use cpu::CPU;

/// Error type for emulator operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmulatorError {
    /// ROM data is too small to be valid
    RomTooSmall,
    /// ROM checksum validation failed
    InvalidChecksum,
    /// Unsupported cartridge type
    UnsupportedCartridge,
    /// ROM requires Color mode but Classic was requested
    RequiresColorMode,
    /// Invalid save state data
    InvalidSaveState,
    /// Save state version mismatch
    SaveStateVersionMismatch,
}

/// Configuration for emulator creation
#[derive(Clone, Copy)]
pub struct EmulatorConfig {
    /// Force classic Game Boy mode even for Color-compatible games
    pub force_classic: bool,
    /// Skip ROM checksum validation
    pub skip_checksum: bool,
    /// Enable audio processing
    pub enable_audio: bool,
    /// Audio sample rate (default: 44100)
    pub sample_rate: u32,
}

impl Default for EmulatorConfig {
    fn default() -> Self {
        Self {
            force_classic: false,
            skip_checksum: false,
            enable_audio: true,
            sample_rate: 44100,
        }
    }
}

/// Main emulator struct
pub struct Emulator {
    cpu: CPU,
}

impl Emulator {
    /// Create a new emulator instance from ROM data
    pub fn new(rom: &[u8], config: EmulatorConfig) -> Result<Self, EmulatorError> {
        Self::new_with_time(rom, config, 0)
    }

    /// Create a new emulator instance with a custom time source for RTC support
    pub fn new_with_time(
        rom: &[u8],
        config: EmulatorConfig,
        unix_timestamp: u64,  // Changed to u64 directly
    ) -> Result<Self, EmulatorError> {
        let cartridge = cartridge::load_cartridge(rom, config.skip_checksum, unix_timestamp)?;

        let cpu = if config.force_classic {
            CPU::new(cartridge)?
        } else {
            CPU::new_cgb(cartridge)?
        };

        Ok(Self { cpu })
    }

    /// Execute cycles for multiple frames at once (for speed-up feature)
    ///
    /// # Arguments
    /// * `multiplier` - Number of frames worth of cycles to run (1 = normal, 2 = 2x speed, 4 = 4x speed)
    ///
    /// # Returns
    /// The actual number of cycles executed
    pub fn step_frame_fast(&mut self, multiplier: u32) -> u32 {
        let target_cycles = 70224u32 * multiplier;
        let mut total = 0u32;

        while total < target_cycles {
            total += self.step();
        }

        total
    }

    /// Execute one CPU instruction and return the number of cycles elapsed
    #[inline]
    pub fn step(&mut self) -> u32 {
        self.cpu.do_cycle()
    }

    /// Execute cycles for approximately one frame (~70224 cycles in normal speed)
    /// Returns the actual number of cycles executed
    pub fn step_frame(&mut self) -> u32 {
        let target_cycles = 70224u32;
        let mut total = 0u32;

        while total < target_cycles {
            total += self.step();
        }

        total
    }

    /// Check if a new frame has been rendered since last check
    pub fn frame_ready(&mut self) -> bool {
        let ready = self.cpu.mmu.gpu.updated;
        self.cpu.mmu.gpu.updated = false;
        ready
    }

    /// Get the current framebuffer (RGB888 format, 160x144 pixels)
    #[inline]
    pub fn framebuffer(&self) -> &[u8] {
        &self.cpu.mmu.gpu.data
    }

    /// Get framebuffer dimensions
    #[inline]
    pub const fn framebuffer_size() -> (usize, usize) {
        (SCREEN_W, SCREEN_H)
    }

    /// Handle a key press
    #[inline]
    pub fn key_down(&mut self, key: KeypadKey) {
        self.cpu.mmu.keypad.keydown(key);
    }

    /// Handle a key release
    #[inline]
    pub fn key_up(&mut self, key: KeypadKey) {
        self.cpu.mmu.keypad.keyup(key);
    }

    /// Get the ROM's internal name
    pub fn rom_name(&self) -> &str {
        self.cpu.mmu.mbc.rom_name()
    }

    /// Get current emulator mode
    #[inline]
    pub fn mode(&self) -> GbMode {
        self.cpu.mmu.gbmode
    }

    /// Check if cartridge has battery-backed RAM
    #[inline]
    pub fn has_battery(&self) -> bool {
        self.cpu.mmu.mbc.has_battery()
    }

    /// Export cartridge RAM for save file
    pub fn export_ram(&self) -> Option<Vec<u8>> {
        if self.has_battery() {
            Some(self.cpu.mmu.mbc.export_ram())
        } else {
            None
        }
    }

    /// Import cartridge RAM from save file
    pub fn import_ram(&mut self, data: &[u8]) -> Result<(), EmulatorError> {
        self.cpu
            .mmu
            .mbc
            .import_ram(data)
            .map_err(|_| EmulatorError::InvalidSaveState)
    }

    /// Check if RAM has been modified since last check (for auto-save)
    pub fn ram_modified(&mut self) -> bool {
        self.cpu.mmu.mbc.check_and_reset_ram_updated()
    }

    /// Enable audio with the given output handler
    pub fn enable_audio(&mut self, output: Box<dyn AudioOutput>) {
        let sound = match self.cpu.mmu.gbmode {
            GbMode::Classic => sound::Sound::new_dmg(output),
            GbMode::Color | GbMode::ColorAsClassic => sound::Sound::new_cgb(output),
        };
        self.cpu.mmu.sound = Some(sound);
    }

    /// Disable audio processing
    pub fn disable_audio(&mut self) {
        self.cpu.mmu.sound = None;
    }

    /// Sync audio (call after speed changes to avoid audio glitches)
    pub fn sync_audio(&mut self) {
        if let Some(ref mut sound) = self.cpu.mmu.sound {
            sound.sync();
        }
    }

    /// Attach a serial link handler (for Game Boy Printer, link cable, etc.)
    pub fn attach_serial(&mut self, link: Box<dyn SerialLink>) {
        self.cpu.mmu.serial.set_callback(link);
    }

    /// Detach serial link handler
    pub fn detach_serial(&mut self) {
        self.cpu.mmu.serial.unset_callback();
    }

    /// Read a byte from memory (for debugging)
    #[inline]
    pub fn read_byte(&mut self, address: u16) -> u8 {
        self.cpu.mmu.rb(address)
    }

    /// Write a byte to memory (for debugging/cheats)
    #[inline]
    pub fn write_byte(&mut self, address: u16, value: u8) {
        self.cpu.mmu.wb(address, value);
    }

    /// Get save state size estimate
    pub fn save_state_size(&self) -> usize {
        // Rough estimate: CPU state + VRAM + WRAM + cartridge RAM + misc
        0x10000
    }

    /// Export emulator state for save states
    /// Returns serialized state data
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::with_capacity(self.save_state_size());
        self.cpu.serialize(&mut state);
        state
    }

    /// Import emulator state from save state data
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), EmulatorError> {
        self.cpu
            .deserialize(data)
            .map(|_| ())
            .map_err(|_| EmulatorError::InvalidSaveState)
    }
}
