//! GameBoy Device
//!
//! High-level wrapper that provides a simple interface to the emulator.
//! This is the main entry point for using the emulator.

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use super::cpu::CPU;
use super::gbmode::GbMode;
use super::keypad::KeypadKey;
use super::mbc;
use super::StrResult;

/// GameBoy device - main emulator interface
pub struct Device {
    cpu: CPU,
}

impl Device {
    /// Create a classic GameBoy from ROM data
    pub fn new(romdata: Vec<u8>, skip_checksum: bool) -> StrResult<Device> {
        let cart = mbc::get_mbc(romdata, skip_checksum)?;
        CPU::new(cart).map(|cpu| Device { cpu })
    }

    /// Create a GameBoy Color from ROM data
    pub fn new_cgb(romdata: Vec<u8>, skip_checksum: bool) -> StrResult<Device> {
        let cart = mbc::get_mbc(romdata, skip_checksum)?;
        CPU::new_cgb(cart).map(|cpu| Device { cpu })
    }

    /// Run one CPU cycle, returns number of cycles executed
    pub fn do_cycle(&mut self) -> u32 {
        self.cpu.do_cycle()
    }

    /// Check if GPU updated and reset flag
    pub fn check_and_reset_gpu_updated(&mut self) -> bool {
        let result = self.cpu.mmu.gpu.updated;
        self.cpu.mmu.gpu.updated = false;
        result
    }

    /// Get GPU framebuffer data (160x144x3 RGB bytes)
    pub fn get_gpu_data(&self) -> &[u8] {
        &self.cpu.mmu.gpu.data
    }

    /// Handle key press
    pub fn keydown(&mut self, key: KeypadKey) {
        self.cpu.mmu.keypad.keydown(key);
    }

    /// Handle key release
    pub fn keyup(&mut self, key: KeypadKey) {
        self.cpu.mmu.keypad.keyup(key);
    }

    /// Get ROM title from cartridge header
    pub fn romname(&self) -> String {
        self.cpu.mmu.mbc.romname()
    }

    /// Load external RAM (for save games)
    pub fn loadram(&mut self, ramdata: &[u8]) -> StrResult<()> {
        self.cpu.mmu.mbc.loadram(ramdata)
    }

    /// Dump external RAM (for save games)
    pub fn dumpram(&self) -> Vec<u8> {
        self.cpu.mmu.mbc.dumpram()
    }

    /// Check if cartridge has battery-backed RAM
    pub fn ram_is_battery_backed(&self) -> bool {
        self.cpu.mmu.mbc.is_battery_backed()
    }

    /// Check if RAM was updated since last check
    pub fn check_and_reset_ram_updated(&mut self) -> bool {
        self.cpu.mmu.mbc.check_and_reset_ram_updated()
    }

    /// Get current hardware mode
    pub fn mode(&self) -> GbMode {
        self.cpu.mmu.gbmode
    }

    /// Read byte from memory (for debugging)
    pub fn read_byte(&mut self, address: u16) -> u8 {
        self.cpu.read_byte(address)
    }

    /// Write byte to memory (for debugging)
    pub fn write_byte(&mut self, address: u16, byte: u8) {
        self.cpu.write_byte(address, byte);
    }
}
