//! MBC2 - Memory Bank Controller 2
//!
//! Has built-in 512x4 bits RAM.
//! Used by a few games like Kirby's Dream Land.

extern crate alloc;

use alloc::vec::Vec;
use alloc::vec;
use super::{rom_banks, MBC};
use crate::gameboy::StrResult;

pub struct MBC2 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    ram_on: bool,
    ram_updated: bool,
    rombank: usize,
    has_battery: bool,
    rombanks: usize,
}

impl MBC2 {
    pub fn new(data: Vec<u8>) -> StrResult<MBC2> {
        let has_battery = match data[0x147] {
            0x06 => true,
            _ => false,
        };
        let rombanks = rom_banks(data[0x148]);

        Ok(MBC2 {
            rom: data,
            ram: vec![0; 512],
            ram_on: false,
            ram_updated: false,
            rombank: 1,
            has_battery,
            rombanks,
        })
    }
}

impl MBC for MBC2 {
    fn readrom(&self, addr: u16) -> u8 {
        let bank = if addr < 0x4000 { 0 } else { self.rombank };
        let idx = bank * 0x4000 | ((addr as usize) & 0x3FFF);
        *self.rom.get(idx).unwrap_or(&0xFF)
    }

    fn readram(&self, addr: u16) -> u8 {
        if !self.ram_on {
            return 0xFF;
        }
        // MBC2 RAM is only 4 bits wide
        self.ram[(addr as usize) & 0x1FF] | 0xF0
    }

    fn writerom(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x3FFF => {
                if addr & 0x100 == 0 {
                    self.ram_on = value & 0xF == 0xA;
                } else {
                    self.rombank = match (value as usize) & 0x0F {
                        0 => 1,
                        n => n,
                    } % self.rombanks.max(1);
                }
            }
            _ => {}
        }
    }

    fn writeram(&mut self, addr: u16, value: u8) {
        if !self.ram_on {
            return;
        }
        // MBC2 RAM is only 4 bits wide
        self.ram[(addr as usize) & 0x1FF] = value | 0xF0;
        self.ram_updated = true;
    }

    fn is_battery_backed(&self) -> bool {
        self.has_battery
    }

    fn loadram(&mut self, ramdata: &[u8]) -> StrResult<()> {
        if ramdata.len() != self.ram.len() {
            return Err("Loaded RAM has incorrect length");
        }
        self.ram.copy_from_slice(ramdata);
        Ok(())
    }

    fn dumpram(&self) -> Vec<u8> {
        self.ram.clone()
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        let result = self.ram_updated;
        self.ram_updated = false;
        result
    }
}
