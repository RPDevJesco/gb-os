//! MBC0 - No Memory Bank Controller
//!
//! Simple 32KB ROM cartridges with no banking.

extern crate alloc;

use alloc::vec::Vec;
use super::MBC;
use crate::gameboy::StrResult;

pub struct MBC0 {
    rom: Vec<u8>,
}

impl MBC0 {
    pub fn new(data: Vec<u8>) -> StrResult<MBC0> {
        Ok(MBC0 { rom: data })
    }
}

impl MBC for MBC0 {
    fn readrom(&self, addr: u16) -> u8 {
        *self.rom.get(addr as usize).unwrap_or(&0xFF)
    }

    fn readram(&self, _addr: u16) -> u8 {
        0xFF
    }

    fn writerom(&mut self, _addr: u16, _value: u8) {
        // No-op for MBC0
    }

    fn writeram(&mut self, _addr: u16, _value: u8) {
        // No-op for MBC0
    }

    fn is_battery_backed(&self) -> bool {
        false
    }

    fn loadram(&mut self, _ramdata: &[u8]) -> StrResult<()> {
        Ok(())
    }

    fn dumpram(&self) -> Vec<u8> {
        Vec::new()
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        false
    }
}
