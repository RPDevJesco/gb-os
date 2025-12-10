//! MBC1 - Memory Bank Controller 1
//!
//! Supports up to 2MB ROM and 32KB RAM.
//! Used by many early GameBoy games.

extern crate alloc;

use alloc::vec::Vec;
use super::{ram_banks, rom_banks, MBC};
use crate::gameboy::StrResult;

pub struct MBC1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    ram_on: bool,
    ram_updated: bool,
    banking_mode: u8,
    rombank: usize,
    rambank: usize,
    has_battery: bool,
    rombanks: usize,
    rambanks: usize,
}

impl MBC1 {
    pub fn new(data: Vec<u8>) -> StrResult<MBC1> {
        let (has_battery, rambanks) = match data[0x147] {
            0x02 => (false, ram_banks(data[0x149])),
            0x03 => (true, ram_banks(data[0x149])),
            _ => (false, 0),
        };
        let rombanks = rom_banks(data[0x148]);
        let ramsize = rambanks * 0x2000;

        let mut ram = Vec::with_capacity(ramsize);
        ram.resize(ramsize, 0);

        Ok(MBC1 {
            rom: data,
            ram,
            ram_on: false,
            banking_mode: 0,
            rombank: 1,
            rambank: 0,
            ram_updated: false,
            has_battery,
            rombanks,
            rambanks,
        })
    }
}

impl MBC for MBC1 {
    fn readrom(&self, addr: u16) -> u8 {
        let bank = if addr < 0x4000 {
            if self.banking_mode == 0 {
                0
            } else {
                self.rombank & 0xE0
            }
        } else {
            self.rombank
        };
        let idx = bank * 0x4000 | ((addr as usize) & 0x3FFF);
        *self.rom.get(idx).unwrap_or(&0xFF)
    }

    fn readram(&self, addr: u16) -> u8 {
        if !self.ram_on || self.rambanks == 0 {
            return 0xFF;
        }
        let rambank = if self.banking_mode == 1 {
            self.rambank
        } else {
            0
        };
        let idx = (rambank * 0x2000) | ((addr & 0x1FFF) as usize);
        *self.ram.get(idx).unwrap_or(&0xFF)
    }

    fn writerom(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_on = value & 0xF == 0xA;
            }
            0x2000..=0x3FFF => {
                let lower_bits = match (value as usize) & 0x1F {
                    0 => 1,
                    n => n,
                };
                self.rombank = ((self.rombank & 0x60) | lower_bits) % self.rombanks.max(1);
            }
            0x4000..=0x5FFF => {
                if self.rombanks > 0x20 {
                    let upper_bits = (value as usize & 0x03) % (self.rombanks >> 5).max(1);
                    self.rombank = self.rombank & 0x1F | (upper_bits << 5);
                }
                if self.rambanks > 1 {
                    self.rambank = (value as usize) & 0x03;
                }
            }
            0x6000..=0x7FFF => {
                self.banking_mode = value & 0x01;
            }
            _ => {}
        }
    }

    fn writeram(&mut self, addr: u16, value: u8) {
        if !self.ram_on || self.rambanks == 0 {
            return;
        }
        let rambank = if self.banking_mode == 1 {
            self.rambank
        } else {
            0
        };
        let idx = (rambank * 0x2000) | ((addr & 0x1FFF) as usize);
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
