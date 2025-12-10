//! MBC3 - Memory Bank Controller 3
//!
//! Supports up to 2MB ROM, 32KB RAM, and Real-Time Clock.
//! Used by Pokemon Gold/Silver/Crystal.
//!
//! Note: RTC is stubbed in no_std mode (no system time available)

extern crate alloc;

use alloc::vec::Vec;
use super::{ram_banks, rom_banks, MBC};
use crate::gameboy::StrResult;

pub struct MBC3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rombank: usize,
    rambank: usize,
    ram_on: bool,
    ram_updated: bool,
    has_battery: bool,
    rombanks: usize,
    rambanks: usize,
    // RTC registers (stubbed)
    selectrtc: bool,
    rtc_ram: [u8; 5],
    rtc_ram_latch: [u8; 5],
    rtc_latch: u8,
}

impl MBC3 {
    pub fn new(data: Vec<u8>) -> StrResult<MBC3> {
        let subtype = data[0x147];
        let has_battery = matches!(subtype, 0x0F | 0x10 | 0x13);
        let rambanks = match subtype {
            0x10 | 0x12 | 0x13 => ram_banks(data[0x149]),
            _ => 0,
        };
        let ramsize = 0x2000 * rambanks;
        let rombanks = rom_banks(data[0x148]);

        let mut ram = Vec::with_capacity(ramsize);
        ram.resize(ramsize, 0);

        Ok(MBC3 {
            rom: data,
            ram,
            rombank: 1,
            rambank: 0,
            ram_updated: false,
            ram_on: false,
            has_battery,
            rombanks,
            rambanks,
            selectrtc: false,
            rtc_ram: [0; 5],
            rtc_ram_latch: [0; 5],
            rtc_latch: 0xFF,
        })
    }

    fn latch_rtc_reg(&mut self) {
        // In a real implementation, we'd read system time here
        // For now, just copy current values
        self.rtc_ram_latch.copy_from_slice(&self.rtc_ram);
    }
}

impl MBC for MBC3 {
    fn readrom(&self, addr: u16) -> u8 {
        let idx = if addr < 0x4000 {
            addr as usize
        } else {
            self.rombank * 0x4000 | ((addr as usize) & 0x3FFF)
        };
        *self.rom.get(idx).unwrap_or(&0xFF)
    }

    fn readram(&self, addr: u16) -> u8 {
        if !self.ram_on {
            return 0xFF;
        }
        if !self.selectrtc && self.rambank < self.rambanks {
            let idx = self.rambank * 0x2000 | ((addr as usize) & 0x1FFF);
            *self.ram.get(idx).unwrap_or(&0xFF)
        } else if self.selectrtc && self.rambank < 5 {
            self.rtc_ram_latch[self.rambank]
        } else {
            0xFF
        }
    }

    fn writerom(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_on = (value & 0x0F) == 0x0A,
            0x2000..=0x3FFF => {
                self.rombank = match value & 0x7F {
                    0 => 1,
                    n => n as usize,
                } % self.rombanks.max(1);
            }
            0x4000..=0x5FFF => {
                self.selectrtc = value & 0x8 == 0x8;
                self.rambank = (value & 0x7) as usize;
            }
            0x6000..=0x7FFF => {
                // RTC latch
                if self.rtc_latch == 0 && value == 1 {
                    self.latch_rtc_reg();
                }
                self.rtc_latch = value;
            }
            _ => {}
        }
    }

    fn writeram(&mut self, addr: u16, value: u8) {
        if !self.ram_on {
            return;
        }
        if !self.selectrtc && self.rambank < self.rambanks {
            let idx = self.rambank * 0x2000 | ((addr as usize) & 0x1FFF);
            if idx < self.ram.len() {
                self.ram[idx] = value;
                self.ram_updated = true;
            }
        } else if self.selectrtc && self.rambank < 5 {
            self.rtc_ram[self.rambank] = value;
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
