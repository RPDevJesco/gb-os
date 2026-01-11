//! Cartridge handling and Memory Bank Controller (MBC) implementations
//!
//! Supports:
//! - MBC0 (no mapper)
//! - MBC1
//! - MBC2
//! - MBC3 (with RTC support)
//! - MBC5

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::{EmulatorError};

/// ROM information extracted from cartridge header
#[derive(Debug, Clone)]
pub struct RomInfo {
    /// Internal ROM name
    pub name: String,
    /// Cartridge type byte
    pub cartridge_type: u8,
    /// ROM size in bytes
    pub rom_size: usize,
    /// RAM size in bytes
    pub ram_size: usize,
    /// Whether this is a CGB-enhanced game
    pub cgb_flag: u8,
    /// Whether cartridge has battery backup
    pub has_battery: bool,
    /// Whether cartridge has RTC
    pub has_rtc: bool,
}

/// Trait for cartridge RAM persistence
pub trait CartridgeRam {
    /// Export RAM contents for saving
    fn export(&self) -> Vec<u8>;
    /// Import RAM contents from save
    fn import(&mut self, data: &[u8]) -> Result<(), EmulatorError>;
}

/// Memory Bank Controller trait
pub trait Cartridge: Send {
    /// Read from ROM address space (0x0000-0x7FFF)
    fn read_rom(&self, address: u16) -> u8;
    /// Read from RAM address space (0xA000-0xBFFF)
    fn read_ram(&self, address: u16) -> u8;
    /// Write to ROM address space (for bank switching)
    fn write_rom(&mut self, address: u16, value: u8);
    /// Write to RAM address space
    fn write_ram(&mut self, address: u16, value: u8);

    /// Check if cartridge has battery-backed RAM
    fn has_battery(&self) -> bool;
    /// Export RAM data for save files
    fn export_ram(&self) -> Vec<u8>;
    /// Import RAM data from save files
    fn import_ram(&mut self, data: &[u8]) -> Result<(), EmulatorError>;
    /// Check and reset RAM modified flag
    fn check_and_reset_ram_updated(&mut self) -> bool;

    /// Get ROM name
    fn rom_name(&self) -> &str;

    /// Serialize cartridge state
    fn serialize(&self, output: &mut Vec<u8>);
    /// Deserialize cartridge state
    fn deserialize(&mut self, data: &[u8]) -> Result<usize, EmulatorError>;
}

/// Load a cartridge from ROM data
/// Load a cartridge from ROM data
pub fn load_cartridge(
    rom: &[u8],
    skip_checksum: bool,
    unix_timestamp: u64,  // Changed from &dyn TimeSource
) -> Result<Box<dyn Cartridge>, EmulatorError> {
    if rom.len() < 0x150 {
        return Err(EmulatorError::RomTooSmall);
    }

    if !skip_checksum {
        validate_checksum(rom)?;
    }

    let cartridge_type = rom[0x147];

    match cartridge_type {
        0x00 => Ok(Box::new(Mbc0::new(rom))),
        0x01..=0x03 => Ok(Box::new(Mbc1::new(rom))),
        0x05..=0x06 => Ok(Box::new(Mbc2::new(rom))),
        0x0F..=0x13 => Ok(Box::new(Mbc3::new(rom, unix_timestamp))),  // Direct value
        0x19..=0x1E => Ok(Box::new(Mbc5::new(rom))),
        _ => Err(EmulatorError::UnsupportedCartridge),
    }
}

fn validate_checksum(rom: &[u8]) -> Result<(), EmulatorError> {
    let mut checksum: u8 = 0;
    for i in 0x134..0x14D {
        checksum = checksum.wrapping_sub(rom[i]).wrapping_sub(1);
    }
    if rom[0x14D] == checksum {
        Ok(())
    } else {
        Err(EmulatorError::InvalidChecksum)
    }
}

fn rom_banks(code: u8) -> usize {
    if code <= 8 {
        2 << code
    } else {
        0
    }
}

fn ram_banks(code: u8) -> usize {
    match code {
        1 | 2 => 1,
        3 => 4,
        4 => 16,
        5 => 8,
        _ => 0,
    }
}

fn extract_rom_name(rom: &[u8]) -> String {
    const TITLE_START: usize = 0x134;
    const CGB_FLAG: usize = 0x143;

    let title_size = if rom[CGB_FLAG] & 0x80 == 0x80 { 11 } else { 16 };
    let mut name = String::with_capacity(title_size);

    for i in 0..title_size {
        match rom[TITLE_START + i] {
            0 => break,
            v if v.is_ascii() => name.push(v as char),
            _ => break,
        }
    }

    name
}

// ============================================================================
// MBC0 - No mapper (32KB ROM only)
// ============================================================================

struct Mbc0 {
    rom: Vec<u8>,
    name: String,
}

impl Mbc0 {
    fn new(rom: &[u8]) -> Self {
        Self {
            name: extract_rom_name(rom),
            rom: rom.to_vec(),
        }
    }
}

impl Cartridge for Mbc0 {
    fn read_rom(&self, address: u16) -> u8 {
        self.rom.get(address as usize).copied().unwrap_or(0xFF)
    }

    fn read_ram(&self, _address: u16) -> u8 {
        0xFF
    }

    fn write_rom(&mut self, _address: u16, _value: u8) {}
    fn write_ram(&mut self, _address: u16, _value: u8) {}

    fn has_battery(&self) -> bool {
        false
    }

    fn export_ram(&self) -> Vec<u8> {
        Vec::new()
    }

    fn import_ram(&mut self, _data: &[u8]) -> Result<(), EmulatorError> {
        Ok(())
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        false
    }

    fn rom_name(&self) -> &str {
        &self.name
    }

    fn serialize(&self, _output: &mut Vec<u8>) {}

    fn deserialize(&mut self, _data: &[u8]) -> Result<usize, EmulatorError> {
        Ok(0)
    }
}

// ============================================================================
// MBC1 - Up to 2MB ROM and/or 32KB RAM
// ============================================================================

struct Mbc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    name: String,
    ram_enabled: bool,
    ram_updated: bool,
    banking_mode: u8,
    rom_bank: usize,
    ram_bank: usize,
    has_battery: bool,
    rom_banks: usize,
    ram_banks: usize,
}

impl Mbc1 {
    fn new(rom: &[u8]) -> Self {
        let cartridge_type = rom[0x147];
        let (has_battery, ram_bank_count) = match cartridge_type {
            0x03 => (true, ram_banks(rom[0x149])),
            0x02 => (false, ram_banks(rom[0x149])),
            _ => (false, 0),
        };

        let rom_bank_count = rom_banks(rom[0x148]);
        let ram_size = ram_bank_count * 0x2000;

        Self {
            name: extract_rom_name(rom),
            rom: rom.to_vec(),
            ram: vec![0; ram_size],
            ram_enabled: false,
            ram_updated: false,
            banking_mode: 0,
            rom_bank: 1,
            ram_bank: 0,
            has_battery,
            rom_banks: rom_bank_count,
            ram_banks: ram_bank_count,
        }
    }
}

impl Cartridge for Mbc1 {
    fn read_rom(&self, address: u16) -> u8 {
        let bank = if address < 0x4000 {
            if self.banking_mode == 0 {
                0
            } else {
                self.rom_bank & 0xE0
            }
        } else {
            self.rom_bank
        };

        let idx = bank * 0x4000 | (address as usize & 0x3FFF);
        self.rom.get(idx).copied().unwrap_or(0xFF)
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled || self.ram.is_empty() {
            return 0xFF;
        }

        let bank = if self.banking_mode == 1 {
            self.ram_bank
        } else {
            0
        };

        let idx = bank * 0x2000 | (address as usize & 0x1FFF);
        self.ram.get(idx).copied().unwrap_or(0xFF)
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x3FFF => {
                let lower_bits = match value as usize & 0x1F {
                    0 => 1,
                    n => n,
                };
                self.rom_bank = ((self.rom_bank & 0x60) | lower_bits) % self.rom_banks.max(1);
            }
            0x4000..=0x5FFF => {
                if self.rom_banks > 0x20 {
                    let upper_bits = (value as usize & 0x03) % (self.rom_banks >> 5).max(1);
                    self.rom_bank = (self.rom_bank & 0x1F) | (upper_bits << 5);
                }
                if self.ram_banks > 1 {
                    self.ram_bank = (value as usize) & 0x03;
                }
            }
            0x6000..=0x7FFF => {
                self.banking_mode = value & 0x01;
            }
            _ => {}
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_enabled || self.ram.is_empty() {
            return;
        }

        let bank = if self.banking_mode == 1 {
            self.ram_bank
        } else {
            0
        };

        let idx = bank * 0x2000 | (address as usize & 0x1FFF);
        if idx < self.ram.len() {
            self.ram[idx] = value;
            self.ram_updated = true;
        }
    }

    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn export_ram(&self) -> Vec<u8> {
        self.ram.clone()
    }

    fn import_ram(&mut self, data: &[u8]) -> Result<(), EmulatorError> {
        if data.len() != self.ram.len() {
            return Err(EmulatorError::InvalidSaveState);
        }
        self.ram.copy_from_slice(data);
        Ok(())
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        let result = self.ram_updated;
        self.ram_updated = false;
        result
    }

    fn rom_name(&self) -> &str {
        &self.name
    }

    fn serialize(&self, output: &mut Vec<u8>) {
        output.push(self.ram_enabled as u8);
        output.push(self.banking_mode);
        output.extend_from_slice(&(self.rom_bank as u16).to_le_bytes());
        output.extend_from_slice(&(self.ram_bank as u16).to_le_bytes());
        output.extend_from_slice(&self.ram);
    }

    fn deserialize(&mut self, data: &[u8]) -> Result<usize, EmulatorError> {
        if data.len() < 6 + self.ram.len() {
            return Err(EmulatorError::InvalidSaveState);
        }

        self.ram_enabled = data[0] != 0;
        self.banking_mode = data[1];
        self.rom_bank = u16::from_le_bytes([data[2], data[3]]) as usize;
        self.ram_bank = u16::from_le_bytes([data[4], data[5]]) as usize;
        let ram_len = self.ram.len();
        self.ram.copy_from_slice(&data[6..6 + ram_len]);

        Ok(6 + ram_len)
    }
}

// ============================================================================
// MBC2 - 256KB ROM, 512x4 bits RAM
// ============================================================================

struct Mbc2 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    name: String,
    ram_enabled: bool,
    ram_updated: bool,
    rom_bank: usize,
    has_battery: bool,
    rom_banks: usize,
}

impl Mbc2 {
    fn new(rom: &[u8]) -> Self {
        let has_battery = rom[0x147] == 0x06;
        let rom_bank_count = rom_banks(rom[0x148]);

        Self {
            name: extract_rom_name(rom),
            rom: rom.to_vec(),
            ram: vec![0; 512],
            ram_enabled: false,
            ram_updated: false,
            rom_bank: 1,
            has_battery,
            rom_banks: rom_bank_count,
        }
    }
}

impl Cartridge for Mbc2 {
    fn read_rom(&self, address: u16) -> u8 {
        let bank = if address < 0x4000 { 0 } else { self.rom_bank };
        let idx = bank * 0x4000 | (address as usize & 0x3FFF);
        self.rom.get(idx).copied().unwrap_or(0xFF)
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        self.ram[(address as usize) & 0x1FF] | 0xF0
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        if address < 0x4000 {
            if address & 0x100 == 0 {
                self.ram_enabled = value & 0x0F == 0x0A;
            } else {
                self.rom_bank = match (value as usize) & 0x0F {
                    0 => 1,
                    n => n,
                } % self.rom_banks.max(1);
            }
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }
        self.ram[(address as usize) & 0x1FF] = value | 0xF0;
        self.ram_updated = true;
    }

    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn export_ram(&self) -> Vec<u8> {
        self.ram.clone()
    }

    fn import_ram(&mut self, data: &[u8]) -> Result<(), EmulatorError> {
        if data.len() != self.ram.len() {
            return Err(EmulatorError::InvalidSaveState);
        }
        self.ram.copy_from_slice(data);
        Ok(())
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        let result = self.ram_updated;
        self.ram_updated = false;
        result
    }

    fn rom_name(&self) -> &str {
        &self.name
    }

    fn serialize(&self, output: &mut Vec<u8>) {
        output.push(self.ram_enabled as u8);
        output.extend_from_slice(&(self.rom_bank as u16).to_le_bytes());
        output.extend_from_slice(&self.ram);
    }

    fn deserialize(&mut self, data: &[u8]) -> Result<usize, EmulatorError> {
        let ram_len = self.ram.len();
        if data.len() < 3 + ram_len {
            return Err(EmulatorError::InvalidSaveState);
        }

        self.ram_enabled = data[0] != 0;
        self.rom_bank = u16::from_le_bytes([data[1], data[2]]) as usize;
        self.ram.copy_from_slice(&data[3..3 + ram_len]);

        Ok(3 + ram_len)
    }
}

// ============================================================================
// MBC3 - Up to 2MB ROM, 32KB RAM, RTC
// ============================================================================

struct Mbc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    name: String,
    rom_bank: usize,
    ram_bank: usize,
    ram_banks: usize,
    rtc_select: bool,
    ram_enabled: bool,
    ram_updated: bool,
    has_battery: bool,
    // RTC state
    rtc_regs: [u8; 5],
    rtc_latch: [u8; 5],
    rtc_zero: u64,
    /// Whether this cart has RTC (reserved for future time-aware API)
    #[allow(dead_code)]
    has_rtc: bool,
}

impl Mbc3 {
    fn new(rom: &[u8], current_time: u64) -> Self {
        let subtype = rom[0x147];

        let has_battery = matches!(subtype, 0x0F | 0x10 | 0x13);
        let has_rtc = matches!(subtype, 0x0F | 0x10);

        let ram_bank_count = match subtype {
            0x10 | 0x12 | 0x13 => ram_banks(rom[0x149]),
            _ => 0,
        };

        let ram_size = ram_bank_count * 0x2000;

        Self {
            name: extract_rom_name(rom),
            rom: rom.to_vec(),
            ram: vec![0; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            ram_banks: ram_bank_count,
            rtc_select: false,
            ram_enabled: false,
            ram_updated: false,
            has_battery,
            rtc_regs: [0; 5],
            rtc_latch: [0; 5],
            rtc_zero: current_time,
            has_rtc,
        }
    }

    /// Update RTC registers from elapsed time
    /// Note: Reserved for future time-aware API where TimeSource is passed to operations
    #[allow(dead_code)]
    fn update_rtc(&mut self, current_time: u64) {
        if !self.has_rtc {
            return;
        }

        // Check if halted
        if self.rtc_regs[4] & 0x40 != 0 {
            return;
        }

        let elapsed = current_time.saturating_sub(self.rtc_zero);

        self.rtc_regs[0] = (elapsed % 60) as u8;
        self.rtc_regs[1] = ((elapsed / 60) % 60) as u8;
        self.rtc_regs[2] = ((elapsed / 3600) % 24) as u8;

        let days = elapsed / (3600 * 24);
        self.rtc_regs[3] = days as u8;
        self.rtc_regs[4] = (self.rtc_regs[4] & 0xFE) | (((days >> 8) & 0x01) as u8);

        if days >= 512 {
            self.rtc_regs[4] |= 0x80; // Day counter overflow
        }
    }

    fn latch_rtc(&mut self) {
        self.rtc_latch.copy_from_slice(&self.rtc_regs);
    }
}

impl Cartridge for Mbc3 {
    fn read_rom(&self, address: u16) -> u8 {
        let idx = if address < 0x4000 {
            address as usize
        } else {
            self.rom_bank * 0x4000 | (address as usize & 0x3FFF)
        };
        self.rom.get(idx).copied().unwrap_or(0xFF)
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }

        if !self.rtc_select && self.ram_bank < self.ram_banks {
            let idx = self.ram_bank * 0x2000 | (address as usize & 0x1FFF);
            self.ram.get(idx).copied().unwrap_or(0xFF)
        } else if self.rtc_select && self.ram_bank < 5 {
            self.rtc_latch[self.ram_bank]
        } else {
            0xFF
        }
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x3FFF => {
                self.rom_bank = match value & 0x7F {
                    0 => 1,
                    n => n as usize,
                };
            }
            0x4000..=0x5FFF => {
                self.rtc_select = value & 0x08 != 0;
                self.ram_bank = (value & 0x07) as usize;
            }
            0x6000..=0x7FFF => {
                self.latch_rtc();
            }
            _ => {}
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }

        if !self.rtc_select && self.ram_bank < self.ram_banks {
            let idx = self.ram_bank * 0x2000 | (address as usize & 0x1FFF);
            if idx < self.ram.len() {
                self.ram[idx] = value;
                self.ram_updated = true;
            }
        } else if self.rtc_select && self.ram_bank < 5 {
            let mask = match self.ram_bank {
                0 | 1 => 0x3F,
                2 => 0x1F,
                4 => 0xC1,
                _ => 0xFF,
            };
            self.rtc_regs[self.ram_bank] = value & mask;
            self.ram_updated = true;
        }
    }

    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn export_ram(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(8 + self.ram.len());
        data.extend_from_slice(&self.rtc_zero.to_be_bytes());
        data.extend_from_slice(&self.ram);
        data
    }

    fn import_ram(&mut self, data: &[u8]) -> Result<(), EmulatorError> {
        if data.len() != 8 + self.ram.len() {
            return Err(EmulatorError::InvalidSaveState);
        }

        self.rtc_zero = u64::from_be_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]);
        self.ram.copy_from_slice(&data[8..]);
        Ok(())
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        let result = self.ram_updated;
        self.ram_updated = false;
        result
    }

    fn rom_name(&self) -> &str {
        &self.name
    }

    fn serialize(&self, output: &mut Vec<u8>) {
        output.push(self.ram_enabled as u8);
        output.push(self.rtc_select as u8);
        output.extend_from_slice(&(self.rom_bank as u16).to_le_bytes());
        output.extend_from_slice(&(self.ram_bank as u16).to_le_bytes());
        output.extend_from_slice(&self.rtc_regs);
        output.extend_from_slice(&self.rtc_latch);
        output.extend_from_slice(&self.rtc_zero.to_le_bytes());
        output.extend_from_slice(&self.ram);
    }

    fn deserialize(&mut self, data: &[u8]) -> Result<usize, EmulatorError> {
        let min_size = 1 + 1 + 2 + 2 + 5 + 5 + 8 + self.ram.len();
        if data.len() < min_size {
            return Err(EmulatorError::InvalidSaveState);
        }

        let mut offset = 0;
        self.ram_enabled = data[offset] != 0;
        offset += 1;
        self.rtc_select = data[offset] != 0;
        offset += 1;
        self.rom_bank = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;
        self.ram_bank = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;
        self.rtc_regs.copy_from_slice(&data[offset..offset + 5]);
        offset += 5;
        self.rtc_latch.copy_from_slice(&data[offset..offset + 5]);
        offset += 5;
        self.rtc_zero = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;
        let ram_len = self.ram.len();
        self.ram.copy_from_slice(&data[offset..offset + ram_len]);
        offset += ram_len;

        Ok(offset)
    }
}

// ============================================================================
// MBC5 - Up to 8MB ROM, 128KB RAM
// ============================================================================

struct Mbc5 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    name: String,
    rom_bank: usize,
    ram_bank: usize,
    ram_enabled: bool,
    ram_updated: bool,
    has_battery: bool,
    rom_banks: usize,
    ram_banks: usize,
}

impl Mbc5 {
    fn new(rom: &[u8]) -> Self {
        let subtype = rom[0x147];

        let has_battery = matches!(subtype, 0x1B | 0x1E);
        let ram_bank_count = match subtype {
            0x1A | 0x1B | 0x1D | 0x1E => ram_banks(rom[0x149]),
            _ => 0,
        };

        let rom_bank_count = rom_banks(rom[0x148]);
        let ram_size = ram_bank_count * 0x2000;

        Self {
            name: extract_rom_name(rom),
            rom: rom.to_vec(),
            ram: vec![0; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
            ram_updated: false,
            has_battery,
            rom_banks: rom_bank_count.max(1),
            ram_banks: ram_bank_count.max(1),
        }
    }
}

impl Cartridge for Mbc5 {
    fn read_rom(&self, address: u16) -> u8 {
        let idx = if address < 0x4000 {
            address as usize
        } else {
            self.rom_bank * 0x4000 | (address as usize & 0x3FFF)
        };
        self.rom.get(idx).copied().unwrap_or(0xFF)
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled || self.ram.is_empty() {
            return 0xFF;
        }
        let idx = self.ram_bank * 0x2000 | (address as usize & 0x1FFF);
        self.ram.get(idx).copied().unwrap_or(0xFF)
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x2FFF => {
                self.rom_bank = ((self.rom_bank & 0x100) | (value as usize)) % self.rom_banks;
            }
            0x3000..=0x3FFF => {
                self.rom_bank =
                    ((self.rom_bank & 0x0FF) | ((value as usize & 0x01) << 8)) % self.rom_banks;
            }
            0x4000..=0x5FFF => {
                self.ram_bank = (value as usize & 0x0F) % self.ram_banks;
            }
            _ => {}
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_enabled || self.ram.is_empty() {
            return;
        }
        let idx = self.ram_bank * 0x2000 | (address as usize & 0x1FFF);
        if idx < self.ram.len() {
            self.ram[idx] = value;
            self.ram_updated = true;
        }
    }

    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn export_ram(&self) -> Vec<u8> {
        self.ram.clone()
    }

    fn import_ram(&mut self, data: &[u8]) -> Result<(), EmulatorError> {
        if data.len() != self.ram.len() {
            return Err(EmulatorError::InvalidSaveState);
        }
        self.ram.copy_from_slice(data);
        Ok(())
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        let result = self.ram_updated;
        self.ram_updated = false;
        result
    }

    fn rom_name(&self) -> &str {
        &self.name
    }

    fn serialize(&self, output: &mut Vec<u8>) {
        output.push(self.ram_enabled as u8);
        output.extend_from_slice(&(self.rom_bank as u16).to_le_bytes());
        output.extend_from_slice(&(self.ram_bank as u16).to_le_bytes());
        output.extend_from_slice(&self.ram);
    }

    fn deserialize(&mut self, data: &[u8]) -> Result<usize, EmulatorError> {
        let ram_len = self.ram.len();
        if data.len() < 5 + ram_len {
            return Err(EmulatorError::InvalidSaveState);
        }

        self.ram_enabled = data[0] != 0;
        self.rom_bank = u16::from_le_bytes([data[1], data[2]]) as usize;
        self.ram_bank = u16::from_le_bytes([data[3], data[4]]) as usize;
        self.ram.copy_from_slice(&data[5..5 + ram_len]);

        Ok(5 + ram_len)
    }
}
