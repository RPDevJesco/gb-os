//! GameBoy CPU Registers
//!
//! The GameBoy CPU is similar to Z80 but with some differences.
//! Registers: AF, BC, DE, HL, SP, PC
//!
//! OPTIMIZED VERSION: Added #[inline(always)] for ARM bare-metal performance.
//! Register access is THE most frequent operation in the emulator.

use super::gbmode::GbMode;

/// CPU flags in the F register
pub enum CpuFlag {
    /// Zero flag (bit 7)
    Z = 0b10000000,
    /// Subtract flag (bit 6)
    N = 0b01000000,
    /// Half-carry flag (bit 5)
    H = 0b00100000,
    /// Carry flag (bit 4)
    C = 0b00010000,
}

/// CPU register file
pub struct Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Registers {
    /// Create new register set with initial values
    pub fn new(mode: GbMode) -> Registers {
        match mode {
            GbMode::Classic => Registers {
                a: 0x01,
                f: 0xB0,
                b: 0x00,
                c: 0x13,
                d: 0x00,
                e: 0xD8,
                h: 0x01,
                l: 0x4D,
                sp: 0xFFFE,
                pc: 0x0100,
            },
            GbMode::Color | GbMode::ColorAsClassic => Registers {
                a: 0x11,
                f: 0x80,
                b: 0x00,
                c: 0x00,
                d: 0xFF,
                e: 0x56,
                h: 0x00,
                l: 0x0D,
                sp: 0xFFFE,
                pc: 0x0100,
            },
        }
    }

    /// Get AF register pair
    #[inline(always)]
    pub fn af(&self) -> u16 {
        ((self.a as u16) << 8) | (self.f as u16)
    }

    /// Set AF register pair
    #[inline(always)]
    pub fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = (value & 0xF0) as u8; // Lower 4 bits always 0
    }

    /// Get BC register pair
    #[inline(always)]
    pub fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }

    /// Set BC register pair
    #[inline(always)]
    pub fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = value as u8;
    }

    /// Get DE register pair
    #[inline(always)]
    pub fn de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }

    /// Set DE register pair
    #[inline(always)]
    pub fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = value as u8;
    }

    /// Get HL register pair
    #[inline(always)]
    pub fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | (self.l as u16)
    }

    /// Set HL register pair
    #[inline(always)]
    pub fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = value as u8;
    }

    /// Get flag state
    #[inline(always)]
    pub fn flag(&self, flag: CpuFlag) -> bool {
        self.f & (flag as u8) != 0
    }

    /// Set or clear a flag
    #[inline(always)]
    pub fn set_flag(&mut self, flag: CpuFlag, value: bool) {
        if value {
            self.f |= flag as u8;
        } else {
            self.f &= !(flag as u8);
        }
    }
}
