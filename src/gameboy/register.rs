//! CPU register definitions and operations
//!
//! The Game Boy CPU has 8-bit registers that can be combined into 16-bit pairs:
//! - AF (Accumulator + Flags)
//! - BC
//! - DE
//! - HL
//! - SP (Stack Pointer)
//! - PC (Program Counter)

use crate::gbmode::GbMode;

/// CPU register set
#[derive(Clone, Copy)]
pub struct Registers {
    pub a: u8,
    f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub pc: u16,
    pub sp: u16,
}

/// CPU status flags
#[derive(Clone, Copy)]
pub enum CpuFlag {
    /// Carry flag
    C = 0b0001_0000,
    /// Half-carry flag (BCD)
    H = 0b0010_0000,
    /// Subtract flag (BCD)
    N = 0b0100_0000,
    /// Zero flag
    Z = 0b1000_0000,
}

impl Registers {
    /// Create registers with initial values for the given mode
    pub fn new(mode: GbMode) -> Self {
        use CpuFlag::*;

        match mode {
            GbMode::Classic => Self {
                a: 0x01,
                f: C as u8 | H as u8 | Z as u8,
                b: 0x00,
                c: 0x13,
                d: 0x00,
                e: 0xD8,
                h: 0x01,
                l: 0x4D,
                pc: 0x0100,
                sp: 0xFFFE,
            },
            GbMode::ColorAsClassic => Self {
                a: 0x11,
                f: Z as u8,
                b: 0x00,
                c: 0x00,
                d: 0x00,
                e: 0x08,
                h: 0x00,
                l: 0x7C,
                pc: 0x0100,
                sp: 0xFFFE,
            },
            GbMode::Color => Self {
                a: 0x11,
                f: Z as u8,
                b: 0x00,
                c: 0x00,
                d: 0xFF,
                e: 0x56,
                h: 0x00,
                l: 0x0D,
                pc: 0x0100,
                sp: 0xFFFE,
            },
        }
    }

    /// Get AF register pair
    #[inline(always)]
    pub fn af(&self) -> u16 {
        ((self.a as u16) << 8) | ((self.f & 0xF0) as u16)
    }

    /// Get BC register pair
    #[inline(always)]
    pub fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }

    /// Get DE register pair
    #[inline(always)]
    pub fn de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }

    /// Get HL register pair
    #[inline(always)]
    pub fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | (self.l as u16)
    }

    /// Get HL and decrement
    #[inline(always)]
    pub fn hld(&mut self) -> u16 {
        let res = self.hl();
        self.sethl(res.wrapping_sub(1));
        res
    }

    /// Get HL and increment
    #[inline(always)]
    pub fn hli(&mut self) -> u16 {
        let res = self.hl();
        self.sethl(res.wrapping_add(1));
        res
    }

    /// Set AF register pair
    #[inline(always)]
    pub fn setaf(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = (value & 0x00F0) as u8;
    }

    /// Set BC register pair
    #[inline(always)]
    pub fn setbc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = (value & 0x00FF) as u8;
    }

    /// Set DE register pair
    #[inline(always)]
    pub fn setde(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = (value & 0x00FF) as u8;
    }

    /// Set HL register pair
    #[inline(always)]
    pub fn sethl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = (value & 0x00FF) as u8;
    }

    /// Set or clear a flag
    #[inline(always)]
    pub fn flag(&mut self, flag: CpuFlag, set: bool) {
        let mask = flag as u8;
        if set {
            self.f |= mask;
        } else {
            self.f &= !mask;
        }
        self.f &= 0xF0;
    }

    /// Get a flag's value
    #[inline(always)]
    pub fn getflag(&self, flag: CpuFlag) -> bool {
        (self.f & (flag as u8)) != 0
    }

    /// Serialize registers to bytes
    pub fn serialize(&self, output: &mut alloc::vec::Vec<u8>) {
        output.push(self.a);
        output.push(self.f);
        output.push(self.b);
        output.push(self.c);
        output.push(self.d);
        output.push(self.e);
        output.push(self.h);
        output.push(self.l);
        output.extend_from_slice(&self.pc.to_le_bytes());
        output.extend_from_slice(&self.sp.to_le_bytes());
    }

    /// Deserialize registers from bytes
    pub fn deserialize(&mut self, data: &[u8]) -> Result<usize, ()> {
        if data.len() < 12 {
            return Err(());
        }
        self.a = data[0];
        self.f = data[1] & 0xF0;
        self.b = data[2];
        self.c = data[3];
        self.d = data[4];
        self.e = data[5];
        self.h = data[6];
        self.l = data[7];
        self.pc = u16::from_le_bytes([data[8], data[9]]);
        self.sp = u16::from_le_bytes([data[10], data[11]]);
        Ok(12)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_pairs() {
        let mut reg = Registers::new(GbMode::Classic);

        reg.a = 0x12;
        reg.f = 0x30;
        assert_eq!(reg.af(), 0x1230);

        reg.setbc(0xABCD);
        assert_eq!(reg.b, 0xAB);
        assert_eq!(reg.c, 0xCD);
        assert_eq!(reg.bc(), 0xABCD);
    }

    #[test]
    fn test_flags() {
        let mut reg = Registers::new(GbMode::Classic);

        reg.flag(CpuFlag::Z, true);
        assert!(reg.getflag(CpuFlag::Z));

        reg.flag(CpuFlag::Z, false);
        assert!(!reg.getflag(CpuFlag::Z));
    }

    #[test]
    fn test_hli_hld() {
        let mut reg = Registers::new(GbMode::Classic);
        reg.sethl(0x1000);

        assert_eq!(reg.hli(), 0x1000);
        assert_eq!(reg.hl(), 0x1001);

        assert_eq!(reg.hld(), 0x1001);
        assert_eq!(reg.hl(), 0x1000);
    }
}
