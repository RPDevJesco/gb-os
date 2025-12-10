//! Memory Bank Controller (MBC) Implementations
//!
//! GameBoy cartridges use various MBC chips to provide more than 32KB ROM
//! and optional battery-backed RAM.
//!
//! Converted to no_std:
//! - Removed serde/typetag (no serialization)
//! - Removed file-backed MBC (ROMs come from memory)
//! - Uses alloc::vec::Vec

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use crate::gameboy::StrResult;

mod mbc0;
mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;

/// Memory Bank Controller trait
pub trait MBC: Send {
    /// Read from ROM address space (0x0000-0x7FFF)
    fn readrom(&self, addr: u16) -> u8;
    
    /// Read from external RAM (0xA000-0xBFFF)
    fn readram(&self, addr: u16) -> u8;
    
    /// Write to ROM address space (bank switching)
    fn writerom(&mut self, addr: u16, value: u8);
    
    /// Write to external RAM
    fn writeram(&mut self, addr: u16, value: u8);
    
    /// Check if RAM was updated (for save detection)
    fn check_and_reset_ram_updated(&mut self) -> bool;
    
    /// Is this cartridge battery-backed?
    fn is_battery_backed(&self) -> bool;
    
    /// Load RAM contents (for saves)
    fn loadram(&mut self, ramdata: &[u8]) -> StrResult<()>;
    
    /// Dump RAM contents (for saves)
    fn dumpram(&self) -> Vec<u8>;
    
    /// Get ROM title from header
    fn romname(&self) -> String {
        const TITLE_START: u16 = 0x134;
        const CGB_FLAG: u16 = 0x143;

        let title_size = match self.readrom(CGB_FLAG) & 0x80 {
            0x80 => 11,
            _ => 16,
        };

        let mut result = String::with_capacity(title_size as usize);

        for i in 0..title_size {
            match self.readrom(TITLE_START + i) {
                0 => break,
                v => result.push(v as char),
            }
        }

        result
    }
}

/// Create appropriate MBC from ROM data
pub fn get_mbc(data: Vec<u8>, skip_checksum: bool) -> StrResult<Box<dyn MBC + 'static>> {
    if data.len() < 0x150 {
        return Err("ROM size too small");
    }
    
    if !skip_checksum {
        check_checksum(&data)?;
    }
    
    // Cartridge type is at 0x147
    match data[0x147] {
        0x00 => mbc0::MBC0::new(data).map(|v| Box::new(v) as Box<dyn MBC>),
        0x01..=0x03 => mbc1::MBC1::new(data).map(|v| Box::new(v) as Box<dyn MBC>),
        0x05..=0x06 => mbc2::MBC2::new(data).map(|v| Box::new(v) as Box<dyn MBC>),
        0x0F..=0x13 => mbc3::MBC3::new(data).map(|v| Box::new(v) as Box<dyn MBC>),
        0x19..=0x1E => mbc5::MBC5::new(data).map(|v| Box::new(v) as Box<dyn MBC>),
        _ => Err("Unsupported MBC type"),
    }
}

/// Calculate number of RAM banks from header
pub fn ram_banks(v: u8) -> usize {
    match v {
        1 => 1,  // Listed as 2KB but we use full 8KB banks
        2 => 1,
        3 => 4,
        4 => 16,
        5 => 8,
        _ => 0,
    }
}

/// Calculate number of ROM banks from header
pub fn rom_banks(v: u8) -> usize {
    if v <= 8 {
        2 << v
    } else {
        0
    }
}

/// Verify ROM header checksum
fn check_checksum(data: &[u8]) -> StrResult<()> {
    let mut value: u8 = 0;
    for i in 0x134..0x14D {
        value = value.wrapping_sub(data[i]).wrapping_sub(1);
    }
    match data[0x14D] == value {
        true => Ok(()),
        false => Err("Cartridge checksum is invalid"),
    }
}
