//! MBC5 - Memory Bank Controller 5
//!
//! Supports up to 8MB ROM and 128KB RAM.
//! Used by later GameBoy and GameBoy Color games.

extern crate alloc;

use alloc::vec::Vec;
use super::{ram_banks, rom_banks, MBC};
use crate::gameboy::StrResult;

pub struct MBC5 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rombank: usize,
    rambank: usize,
    ram_on: bool,
    ram_updated: bool,
    has_battery: bool,
    rombanks: usize,
    rambanks: usize,
}

impl MBC5 {
    pub fn new(data: Vec<u8>) -> StrResult<MBC5> {
        let subtype = data[0x147];
        let has_battery = matches!(subtype, 0x1B | 0x1E);
        let rambanks = match subtype {
            0x1A | 0x1B | 0x1D | 0x1E => ram_banks(data[0x149]),
            _ => 0,
        };
        let ramsize = 0x2000 * rambanks;
        let rombanks = rom_banks(data[0x148]);

        let mut ram = Vec::with_capacity(ramsize);
        ram.resize(ramsize, 0);

        Ok(MBC5 {
            rom: data,
            ram,
            rombank: 1,
            rambank: 0,
            ram_updated: false,
            ram_on: false,
            has_battery,
            rombanks,
            rambanks,
        })
    }
}

impl MBC for MBC5 {
    fn readrom(&self, addr: u16) -> u8 {
        let idx = if addr < 0x4000 {
            addr as usize
        } else {
            self.rombank * 0x4000 | ((addr as usize) & 0x3FFF)
        };
        *self.rom.get(idx).unwrap_or(&0)
    }

    fn readram(&self, addr: u16) -> u8 {
        if !self.ram_on || self.rambanks == 0 {
            return 0xFF;
        }
        let idx = self.rambank * 0x2000 | ((addr as usize) & 0x1FFF);
        *self.ram.get(idx).unwrap_or(&0xFF)
    }

    fn writerom(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_on = value & 0x0F == 0x0A,
            0x2000..=0x2FFF => {
                // Lower 8 bits of ROM bank
                if self.rombanks > 0 {
                    self.rombank = ((self.rombank & 0x100) | (value as usize)) % self.rombanks;
                }
            }
            0x3000..=0x3FFF => {
                // Bit 9 of ROM bank
                if self.rombanks > 0 {
                    self.rombank = ((self.rombank & 0x0FF) | (((value & 0x1) as usize) << 8)) % self.rombanks;
                }
            }
            0x4000..=0x5FFF => {
                // RAM bank (0-15)
                if self.rambanks > 0 {
                    self.rambank = ((value & 0x0F) as usize) % self.rambanks;
                }
            }
            0x6000..=0x7FFF => {
                // Unused
            }
            _ => {}
        }
    }

    fn writeram(&mut self, addr: u16, value: u8) {
        if !self.ram_on || self.rambanks == 0 {
            return;
        }
        let idx = self.rambank * 0x2000 | ((addr as usize) & 0x1FFF);
        if idx < self.ram.len() {
            self.ram[idx] = value;
            self.ram_updated = true;
        }
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
