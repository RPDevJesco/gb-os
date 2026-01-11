//! CPU emulation for the Sharp LR35902 (Game Boy CPU)
//!
//! This is a hybrid between the Intel 8080 and Zilog Z80.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::cartridge::Cartridge;
use crate::mmu::MMU;
use crate::register::CpuFlag::{C, H, N, Z};
use crate::register::Registers;
use crate::EmulatorError;

/// Sharp LR35902 CPU
pub struct CPU {
    reg: Registers,
    pub mmu: MMU,
    halted: bool,
    /// HALT bug: when interrupts are pending, IME is off, and HALT is called,
    /// the PC fails to increment on the next instruction
    halt_bug: bool,
    /// Interrupt Master Enable
    ime: bool,
    /// Countdown for DI instruction (takes effect after next instruction)
    setdi: u32,
    /// Countdown for EI instruction (takes effect after next instruction)
    setei: u32,
}

impl CPU {
    /// Create new CPU in classic Game Boy mode
    pub fn new(cart: Box<dyn Cartridge>) -> Result<CPU, EmulatorError> {
        let mmu = MMU::new(cart)?;
        let registers = Registers::new(mmu.gbmode);
        Ok(CPU {
            reg: registers,
            halted: false,
            halt_bug: false,
            ime: true,
            setdi: 0,
            setei: 0,
            mmu,
        })
    }

    /// Create new CPU in Game Boy Color mode
    pub fn new_cgb(cart: Box<dyn Cartridge>) -> Result<CPU, EmulatorError> {
        let mmu = MMU::new_cgb(cart)?;
        let registers = Registers::new(mmu.gbmode);
        Ok(CPU {
            reg: registers,
            halted: false,
            halt_bug: false,
            ime: true,
            setdi: 0,
            setei: 0,
            mmu,
        })
    }

    /// Execute one instruction and return number of T-cycles
    #[inline]
    pub fn do_cycle(&mut self) -> u32 {
        let ticks = self.docycle() * 4;
        self.mmu.do_cycle(ticks)
    }

    fn docycle(&mut self) -> u32 {
        self.updateime();
        match self.handleinterrupt() {
            0 => {}
            n => return n,
        };

        if self.halted {
            // Emulate a noop instruction
            1
        } else {
            self.call()
        }
    }

    #[inline]
    fn fetchbyte(&mut self) -> u8 {
        let b = self.mmu.rb(self.reg.pc);
        if self.halt_bug {
            self.halt_bug = false;
        } else {
            self.reg.pc = self.reg.pc.wrapping_add(1);
        }
        b
    }

    #[inline]
    fn fetchword(&mut self) -> u16 {
        let w = self.mmu.rw(self.reg.pc);
        self.reg.pc = self.reg.pc.wrapping_add(2);
        w
    }

    fn updateime(&mut self) {
        self.setdi = match self.setdi {
            2 => 1,
            1 => {
                self.ime = false;
                0
            }
            _ => 0,
        };
        self.setei = match self.setei {
            2 => 1,
            1 => {
                self.ime = true;
                0
            }
            _ => 0,
        };
    }

    fn handleinterrupt(&mut self) -> u32 {
        if !self.ime && !self.halted {
            return 0;
        }

        let triggered = self.mmu.inte & self.mmu.intf & 0x1F;
        if triggered == 0 {
            return 0;
        }

        self.halted = false;
        if !self.ime {
            return 0;
        }
        self.ime = false;

        let n = triggered.trailing_zeros();
        if n >= 5 {
            return 0; // Invalid interrupt
        }
        self.mmu.intf &= !(1 << n);
        let pc = self.reg.pc;
        self.pushstack(pc);
        self.reg.pc = 0x0040 | ((n as u16) << 3);

        4
    }

    #[inline]
    fn pushstack(&mut self, value: u16) {
        self.reg.sp = self.reg.sp.wrapping_sub(2);
        self.mmu.ww(self.reg.sp, value);
    }

    #[inline]
    fn popstack(&mut self) -> u16 {
        let res = self.mmu.rw(self.reg.sp);
        self.reg.sp = self.reg.sp.wrapping_add(2);
        res
    }

    /// Execute a single instruction and return M-cycles
    fn call(&mut self) -> u32 {
        let opcode = self.fetchbyte();
        match opcode {
            0x00 => 1, // NOP
            0x01 => { let v = self.fetchword(); self.reg.setbc(v); 3 }
            0x02 => { self.mmu.wb(self.reg.bc(), self.reg.a); 2 }
            0x03 => { self.reg.setbc(self.reg.bc().wrapping_add(1)); 2 }
            0x04 => { self.reg.b = self.alu_inc(self.reg.b); 1 }
            0x05 => { self.reg.b = self.alu_dec(self.reg.b); 1 }
            0x06 => { self.reg.b = self.fetchbyte(); 2 }
            0x07 => { self.reg.a = self.alu_rlc(self.reg.a); self.reg.flag(Z, false); 1 }
            0x08 => { let a = self.fetchword(); self.mmu.ww(a, self.reg.sp); 5 }
            0x09 => { self.alu_add16(self.reg.bc()); 2 }
            0x0A => { self.reg.a = self.mmu.rb(self.reg.bc()); 2 }
            0x0B => { self.reg.setbc(self.reg.bc().wrapping_sub(1)); 2 }
            0x0C => { self.reg.c = self.alu_inc(self.reg.c); 1 }
            0x0D => { self.reg.c = self.alu_dec(self.reg.c); 1 }
            0x0E => { self.reg.c = self.fetchbyte(); 2 }
            0x0F => { self.reg.a = self.alu_rrc(self.reg.a); self.reg.flag(Z, false); 1 }
            0x10 => { self.mmu.switch_speed(); 1 } // STOP
            0x11 => { let v = self.fetchword(); self.reg.setde(v); 3 }
            0x12 => { self.mmu.wb(self.reg.de(), self.reg.a); 2 }
            0x13 => { self.reg.setde(self.reg.de().wrapping_add(1)); 2 }
            0x14 => { self.reg.d = self.alu_inc(self.reg.d); 1 }
            0x15 => { self.reg.d = self.alu_dec(self.reg.d); 1 }
            0x16 => { self.reg.d = self.fetchbyte(); 2 }
            0x17 => { self.reg.a = self.alu_rl(self.reg.a); self.reg.flag(Z, false); 1 }
            0x18 => { self.cpu_jr(); 3 }
            0x19 => { self.alu_add16(self.reg.de()); 2 }
            0x1A => { self.reg.a = self.mmu.rb(self.reg.de()); 2 }
            0x1B => { self.reg.setde(self.reg.de().wrapping_sub(1)); 2 }
            0x1C => { self.reg.e = self.alu_inc(self.reg.e); 1 }
            0x1D => { self.reg.e = self.alu_dec(self.reg.e); 1 }
            0x1E => { self.reg.e = self.fetchbyte(); 2 }
            0x1F => { self.reg.a = self.alu_rr(self.reg.a); self.reg.flag(Z, false); 1 }
            0x20 => { if !self.reg.getflag(Z) { self.cpu_jr(); 3 } else { self.reg.pc = self.reg.pc.wrapping_add(1); 2 } }
            0x21 => { let v = self.fetchword(); self.reg.sethl(v); 3 }
            0x22 => { self.mmu.wb(self.reg.hli(), self.reg.a); 2 }
            0x23 => { let v = self.reg.hl().wrapping_add(1); self.reg.sethl(v); 2 }
            0x24 => { self.reg.h = self.alu_inc(self.reg.h); 1 }
            0x25 => { self.reg.h = self.alu_dec(self.reg.h); 1 }
            0x26 => { self.reg.h = self.fetchbyte(); 2 }
            0x27 => { self.alu_daa(); 1 }
            0x28 => { if self.reg.getflag(Z) { self.cpu_jr(); 3 } else { self.reg.pc = self.reg.pc.wrapping_add(1); 2 } }
            0x29 => { let v = self.reg.hl(); self.alu_add16(v); 2 }
            0x2A => { self.reg.a = self.mmu.rb(self.reg.hli()); 2 }
            0x2B => { let v = self.reg.hl().wrapping_sub(1); self.reg.sethl(v); 2 }
            0x2C => { self.reg.l = self.alu_inc(self.reg.l); 1 }
            0x2D => { self.reg.l = self.alu_dec(self.reg.l); 1 }
            0x2E => { self.reg.l = self.fetchbyte(); 2 }
            0x2F => { self.reg.a = !self.reg.a; self.reg.flag(H, true); self.reg.flag(N, true); 1 }
            0x30 => { if !self.reg.getflag(C) { self.cpu_jr(); 3 } else { self.reg.pc = self.reg.pc.wrapping_add(1); 2 } }
            0x31 => { self.reg.sp = self.fetchword(); 3 }
            0x32 => { self.mmu.wb(self.reg.hld(), self.reg.a); 2 }
            0x33 => { self.reg.sp = self.reg.sp.wrapping_add(1); 2 }
            0x34 => { let a = self.reg.hl(); let v = self.mmu.rb(a); let v2 = self.alu_inc(v); self.mmu.wb(a, v2); 3 }
            0x35 => { let a = self.reg.hl(); let v = self.mmu.rb(a); let v2 = self.alu_dec(v); self.mmu.wb(a, v2); 3 }
            0x36 => { let v = self.fetchbyte(); self.mmu.wb(self.reg.hl(), v); 3 }
            0x37 => { self.reg.flag(C, true); self.reg.flag(H, false); self.reg.flag(N, false); 1 }
            0x38 => { if self.reg.getflag(C) { self.cpu_jr(); 3 } else { self.reg.pc = self.reg.pc.wrapping_add(1); 2 } }
            0x39 => { self.alu_add16(self.reg.sp); 2 }
            0x3A => { self.reg.a = self.mmu.rb(self.reg.hld()); 2 }
            0x3B => { self.reg.sp = self.reg.sp.wrapping_sub(1); 2 }
            0x3C => { self.reg.a = self.alu_inc(self.reg.a); 1 }
            0x3D => { self.reg.a = self.alu_dec(self.reg.a); 1 }
            0x3E => { self.reg.a = self.fetchbyte(); 2 }
            0x3F => { let v = !self.reg.getflag(C); self.reg.flag(C, v); self.reg.flag(H, false); self.reg.flag(N, false); 1 }
            // LD r, r' instructions (0x40-0x7F except 0x76)
            0x40 => 1, // LD B, B
            0x41 => { self.reg.b = self.reg.c; 1 }
            0x42 => { self.reg.b = self.reg.d; 1 }
            0x43 => { self.reg.b = self.reg.e; 1 }
            0x44 => { self.reg.b = self.reg.h; 1 }
            0x45 => { self.reg.b = self.reg.l; 1 }
            0x46 => { self.reg.b = self.mmu.rb(self.reg.hl()); 2 }
            0x47 => { self.reg.b = self.reg.a; 1 }
            0x48 => { self.reg.c = self.reg.b; 1 }
            0x49 => 1, // LD C, C
            0x4A => { self.reg.c = self.reg.d; 1 }
            0x4B => { self.reg.c = self.reg.e; 1 }
            0x4C => { self.reg.c = self.reg.h; 1 }
            0x4D => { self.reg.c = self.reg.l; 1 }
            0x4E => { self.reg.c = self.mmu.rb(self.reg.hl()); 2 }
            0x4F => { self.reg.c = self.reg.a; 1 }
            0x50 => { self.reg.d = self.reg.b; 1 }
            0x51 => { self.reg.d = self.reg.c; 1 }
            0x52 => 1, // LD D, D
            0x53 => { self.reg.d = self.reg.e; 1 }
            0x54 => { self.reg.d = self.reg.h; 1 }
            0x55 => { self.reg.d = self.reg.l; 1 }
            0x56 => { self.reg.d = self.mmu.rb(self.reg.hl()); 2 }
            0x57 => { self.reg.d = self.reg.a; 1 }
            0x58 => { self.reg.e = self.reg.b; 1 }
            0x59 => { self.reg.e = self.reg.c; 1 }
            0x5A => { self.reg.e = self.reg.d; 1 }
            0x5B => 1, // LD E, E
            0x5C => { self.reg.e = self.reg.h; 1 }
            0x5D => { self.reg.e = self.reg.l; 1 }
            0x5E => { self.reg.e = self.mmu.rb(self.reg.hl()); 2 }
            0x5F => { self.reg.e = self.reg.a; 1 }
            0x60 => { self.reg.h = self.reg.b; 1 }
            0x61 => { self.reg.h = self.reg.c; 1 }
            0x62 => { self.reg.h = self.reg.d; 1 }
            0x63 => { self.reg.h = self.reg.e; 1 }
            0x64 => 1, // LD H, H
            0x65 => { self.reg.h = self.reg.l; 1 }
            0x66 => { self.reg.h = self.mmu.rb(self.reg.hl()); 2 }
            0x67 => { self.reg.h = self.reg.a; 1 }
            0x68 => { self.reg.l = self.reg.b; 1 }
            0x69 => { self.reg.l = self.reg.c; 1 }
            0x6A => { self.reg.l = self.reg.d; 1 }
            0x6B => { self.reg.l = self.reg.e; 1 }
            0x6C => { self.reg.l = self.reg.h; 1 }
            0x6D => 1, // LD L, L
            0x6E => { self.reg.l = self.mmu.rb(self.reg.hl()); 2 }
            0x6F => { self.reg.l = self.reg.a; 1 }
            0x70 => { self.mmu.wb(self.reg.hl(), self.reg.b); 2 }
            0x71 => { self.mmu.wb(self.reg.hl(), self.reg.c); 2 }
            0x72 => { self.mmu.wb(self.reg.hl(), self.reg.d); 2 }
            0x73 => { self.mmu.wb(self.reg.hl(), self.reg.e); 2 }
            0x74 => { self.mmu.wb(self.reg.hl(), self.reg.h); 2 }
            0x75 => { self.mmu.wb(self.reg.hl(), self.reg.l); 2 }
            0x76 => { 
                self.halted = true;
                self.halt_bug = self.mmu.intf & self.mmu.inte & 0x1F != 0;
                1 
            }
            0x77 => { self.mmu.wb(self.reg.hl(), self.reg.a); 2 }
            0x78 => { self.reg.a = self.reg.b; 1 }
            0x79 => { self.reg.a = self.reg.c; 1 }
            0x7A => { self.reg.a = self.reg.d; 1 }
            0x7B => { self.reg.a = self.reg.e; 1 }
            0x7C => { self.reg.a = self.reg.h; 1 }
            0x7D => { self.reg.a = self.reg.l; 1 }
            0x7E => { self.reg.a = self.mmu.rb(self.reg.hl()); 2 }
            0x7F => 1, // LD A, A
            // ALU operations (0x80-0xBF)
            0x80 => { self.alu_add(self.reg.b, false); 1 }
            0x81 => { self.alu_add(self.reg.c, false); 1 }
            0x82 => { self.alu_add(self.reg.d, false); 1 }
            0x83 => { self.alu_add(self.reg.e, false); 1 }
            0x84 => { self.alu_add(self.reg.h, false); 1 }
            0x85 => { self.alu_add(self.reg.l, false); 1 }
            0x86 => { let v = self.mmu.rb(self.reg.hl()); self.alu_add(v, false); 2 }
            0x87 => { self.alu_add(self.reg.a, false); 1 }
            0x88 => { self.alu_add(self.reg.b, true); 1 }
            0x89 => { self.alu_add(self.reg.c, true); 1 }
            0x8A => { self.alu_add(self.reg.d, true); 1 }
            0x8B => { self.alu_add(self.reg.e, true); 1 }
            0x8C => { self.alu_add(self.reg.h, true); 1 }
            0x8D => { self.alu_add(self.reg.l, true); 1 }
            0x8E => { let v = self.mmu.rb(self.reg.hl()); self.alu_add(v, true); 2 }
            0x8F => { self.alu_add(self.reg.a, true); 1 }
            0x90 => { self.alu_sub(self.reg.b, false); 1 }
            0x91 => { self.alu_sub(self.reg.c, false); 1 }
            0x92 => { self.alu_sub(self.reg.d, false); 1 }
            0x93 => { self.alu_sub(self.reg.e, false); 1 }
            0x94 => { self.alu_sub(self.reg.h, false); 1 }
            0x95 => { self.alu_sub(self.reg.l, false); 1 }
            0x96 => { let v = self.mmu.rb(self.reg.hl()); self.alu_sub(v, false); 2 }
            0x97 => { self.alu_sub(self.reg.a, false); 1 }
            0x98 => { self.alu_sub(self.reg.b, true); 1 }
            0x99 => { self.alu_sub(self.reg.c, true); 1 }
            0x9A => { self.alu_sub(self.reg.d, true); 1 }
            0x9B => { self.alu_sub(self.reg.e, true); 1 }
            0x9C => { self.alu_sub(self.reg.h, true); 1 }
            0x9D => { self.alu_sub(self.reg.l, true); 1 }
            0x9E => { let v = self.mmu.rb(self.reg.hl()); self.alu_sub(v, true); 2 }
            0x9F => { self.alu_sub(self.reg.a, true); 1 }
            0xA0 => { self.alu_and(self.reg.b); 1 }
            0xA1 => { self.alu_and(self.reg.c); 1 }
            0xA2 => { self.alu_and(self.reg.d); 1 }
            0xA3 => { self.alu_and(self.reg.e); 1 }
            0xA4 => { self.alu_and(self.reg.h); 1 }
            0xA5 => { self.alu_and(self.reg.l); 1 }
            0xA6 => { let v = self.mmu.rb(self.reg.hl()); self.alu_and(v); 2 }
            0xA7 => { self.alu_and(self.reg.a); 1 }
            0xA8 => { self.alu_xor(self.reg.b); 1 }
            0xA9 => { self.alu_xor(self.reg.c); 1 }
            0xAA => { self.alu_xor(self.reg.d); 1 }
            0xAB => { self.alu_xor(self.reg.e); 1 }
            0xAC => { self.alu_xor(self.reg.h); 1 }
            0xAD => { self.alu_xor(self.reg.l); 1 }
            0xAE => { let v = self.mmu.rb(self.reg.hl()); self.alu_xor(v); 2 }
            0xAF => { self.alu_xor(self.reg.a); 1 }
            0xB0 => { self.alu_or(self.reg.b); 1 }
            0xB1 => { self.alu_or(self.reg.c); 1 }
            0xB2 => { self.alu_or(self.reg.d); 1 }
            0xB3 => { self.alu_or(self.reg.e); 1 }
            0xB4 => { self.alu_or(self.reg.h); 1 }
            0xB5 => { self.alu_or(self.reg.l); 1 }
            0xB6 => { let v = self.mmu.rb(self.reg.hl()); self.alu_or(v); 2 }
            0xB7 => { self.alu_or(self.reg.a); 1 }
            0xB8 => { self.alu_cp(self.reg.b); 1 }
            0xB9 => { self.alu_cp(self.reg.c); 1 }
            0xBA => { self.alu_cp(self.reg.d); 1 }
            0xBB => { self.alu_cp(self.reg.e); 1 }
            0xBC => { self.alu_cp(self.reg.h); 1 }
            0xBD => { self.alu_cp(self.reg.l); 1 }
            0xBE => { let v = self.mmu.rb(self.reg.hl()); self.alu_cp(v); 2 }
            0xBF => { self.alu_cp(self.reg.a); 1 }
            // Control flow and misc (0xC0-0xFF)
            0xC0 => { if !self.reg.getflag(Z) { self.reg.pc = self.popstack(); 5 } else { 2 } }
            0xC1 => { let v = self.popstack(); self.reg.setbc(v); 3 }
            0xC2 => { if !self.reg.getflag(Z) { self.reg.pc = self.fetchword(); 4 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 3 } }
            0xC3 => { self.reg.pc = self.fetchword(); 4 }
            0xC4 => { if !self.reg.getflag(Z) { self.pushstack(self.reg.pc.wrapping_add(2)); self.reg.pc = self.fetchword(); 6 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 3 } }
            0xC5 => { self.pushstack(self.reg.bc()); 4 }
            0xC6 => { let v = self.fetchbyte(); self.alu_add(v, false); 2 }
            0xC7 => { self.pushstack(self.reg.pc); self.reg.pc = 0x00; 4 }
            0xC8 => { if self.reg.getflag(Z) { self.reg.pc = self.popstack(); 5 } else { 2 } }
            0xC9 => { self.reg.pc = self.popstack(); 4 }
            0xCA => { if self.reg.getflag(Z) { self.reg.pc = self.fetchword(); 4 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 3 } }
            0xCB => self.call_cb(),
            0xCC => { if self.reg.getflag(Z) { self.pushstack(self.reg.pc.wrapping_add(2)); self.reg.pc = self.fetchword(); 6 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 3 } }
            0xCD => { self.pushstack(self.reg.pc.wrapping_add(2)); self.reg.pc = self.fetchword(); 6 }
            0xCE => { let v = self.fetchbyte(); self.alu_add(v, true); 2 }
            0xCF => { self.pushstack(self.reg.pc); self.reg.pc = 0x08; 4 }
            0xD0 => { if !self.reg.getflag(C) { self.reg.pc = self.popstack(); 5 } else { 2 } }
            0xD1 => { let v = self.popstack(); self.reg.setde(v); 3 }
            0xD2 => { if !self.reg.getflag(C) { self.reg.pc = self.fetchword(); 4 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 3 } }
            0xD4 => { if !self.reg.getflag(C) { self.pushstack(self.reg.pc.wrapping_add(2)); self.reg.pc = self.fetchword(); 6 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 3 } }
            0xD5 => { self.pushstack(self.reg.de()); 4 }
            0xD6 => { let v = self.fetchbyte(); self.alu_sub(v, false); 2 }
            0xD7 => { self.pushstack(self.reg.pc); self.reg.pc = 0x10; 4 }
            0xD8 => { if self.reg.getflag(C) { self.reg.pc = self.popstack(); 5 } else { 2 } }
            0xD9 => { self.reg.pc = self.popstack(); self.setei = 1; 4 } // RETI
            0xDA => { if self.reg.getflag(C) { self.reg.pc = self.fetchword(); 4 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 3 } }
            0xDC => { if self.reg.getflag(C) { self.pushstack(self.reg.pc.wrapping_add(2)); self.reg.pc = self.fetchword(); 6 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 3 } }
            0xDE => { let v = self.fetchbyte(); self.alu_sub(v, true); 2 }
            0xDF => { self.pushstack(self.reg.pc); self.reg.pc = 0x18; 4 }
            0xE0 => { let a = 0xFF00 | self.fetchbyte() as u16; self.mmu.wb(a, self.reg.a); 3 }
            0xE1 => { let v = self.popstack(); self.reg.sethl(v); 3 }
            0xE2 => { self.mmu.wb(0xFF00 | self.reg.c as u16, self.reg.a); 2 }
            0xE5 => { self.pushstack(self.reg.hl()); 4 }
            0xE6 => { let v = self.fetchbyte(); self.alu_and(v); 2 }
            0xE7 => { self.pushstack(self.reg.pc); self.reg.pc = 0x20; 4 }
            0xE8 => { self.reg.sp = self.alu_add16imm(self.reg.sp); 4 }
            0xE9 => { self.reg.pc = self.reg.hl(); 1 }
            0xEA => { let a = self.fetchword(); self.mmu.wb(a, self.reg.a); 4 }
            0xEE => { let v = self.fetchbyte(); self.alu_xor(v); 2 }
            0xEF => { self.pushstack(self.reg.pc); self.reg.pc = 0x28; 4 }
            0xF0 => { let a = 0xFF00 | self.fetchbyte() as u16; self.reg.a = self.mmu.rb(a); 3 }
            0xF1 => { let v = self.popstack() & 0xFFF0; self.reg.setaf(v); 3 }
            0xF2 => { self.reg.a = self.mmu.rb(0xFF00 | self.reg.c as u16); 2 }
            0xF3 => { self.setdi = 2; 1 } // DI
            0xF5 => { self.pushstack(self.reg.af()); 4 }
            0xF6 => { let v = self.fetchbyte(); self.alu_or(v); 2 }
            0xF7 => { self.pushstack(self.reg.pc); self.reg.pc = 0x30; 4 }
            0xF8 => { let r = self.alu_add16imm(self.reg.sp); self.reg.sethl(r); 3 }
            0xF9 => { self.reg.sp = self.reg.hl(); 2 }
            0xFA => { let a = self.fetchword(); self.reg.a = self.mmu.rb(a); 4 }
            0xFB => { self.setei = 2; 1 } // EI
            0xFE => { let v = self.fetchbyte(); self.alu_cp(v); 2 }
            0xFF => { self.pushstack(self.reg.pc); self.reg.pc = 0x38; 4 }
            // Undefined opcodes
            0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD => 1,
        }
    }

    /// Execute CB-prefixed instruction
    fn call_cb(&mut self) -> u32 {
        let opcode = self.fetchbyte();
        match opcode {
            // RLC r
            0x00 => { self.reg.b = self.alu_rlc(self.reg.b); 2 }
            0x01 => { self.reg.c = self.alu_rlc(self.reg.c); 2 }
            0x02 => { self.reg.d = self.alu_rlc(self.reg.d); 2 }
            0x03 => { self.reg.e = self.alu_rlc(self.reg.e); 2 }
            0x04 => { self.reg.h = self.alu_rlc(self.reg.h); 2 }
            0x05 => { self.reg.l = self.alu_rlc(self.reg.l); 2 }
            0x06 => { let a = self.reg.hl(); let v = self.mmu.rb(a); let v2 = self.alu_rlc(v); self.mmu.wb(a, v2); 4 }
            0x07 => { self.reg.a = self.alu_rlc(self.reg.a); 2 }
            // RRC r
            0x08 => { self.reg.b = self.alu_rrc(self.reg.b); 2 }
            0x09 => { self.reg.c = self.alu_rrc(self.reg.c); 2 }
            0x0A => { self.reg.d = self.alu_rrc(self.reg.d); 2 }
            0x0B => { self.reg.e = self.alu_rrc(self.reg.e); 2 }
            0x0C => { self.reg.h = self.alu_rrc(self.reg.h); 2 }
            0x0D => { self.reg.l = self.alu_rrc(self.reg.l); 2 }
            0x0E => { let a = self.reg.hl(); let v = self.mmu.rb(a); let v2 = self.alu_rrc(v); self.mmu.wb(a, v2); 4 }
            0x0F => { self.reg.a = self.alu_rrc(self.reg.a); 2 }
            // RL r
            0x10 => { self.reg.b = self.alu_rl(self.reg.b); 2 }
            0x11 => { self.reg.c = self.alu_rl(self.reg.c); 2 }
            0x12 => { self.reg.d = self.alu_rl(self.reg.d); 2 }
            0x13 => { self.reg.e = self.alu_rl(self.reg.e); 2 }
            0x14 => { self.reg.h = self.alu_rl(self.reg.h); 2 }
            0x15 => { self.reg.l = self.alu_rl(self.reg.l); 2 }
            0x16 => { let a = self.reg.hl(); let v = self.mmu.rb(a); let v2 = self.alu_rl(v); self.mmu.wb(a, v2); 4 }
            0x17 => { self.reg.a = self.alu_rl(self.reg.a); 2 }
            // RR r
            0x18 => { self.reg.b = self.alu_rr(self.reg.b); 2 }
            0x19 => { self.reg.c = self.alu_rr(self.reg.c); 2 }
            0x1A => { self.reg.d = self.alu_rr(self.reg.d); 2 }
            0x1B => { self.reg.e = self.alu_rr(self.reg.e); 2 }
            0x1C => { self.reg.h = self.alu_rr(self.reg.h); 2 }
            0x1D => { self.reg.l = self.alu_rr(self.reg.l); 2 }
            0x1E => { let a = self.reg.hl(); let v = self.mmu.rb(a); let v2 = self.alu_rr(v); self.mmu.wb(a, v2); 4 }
            0x1F => { self.reg.a = self.alu_rr(self.reg.a); 2 }
            // SLA r
            0x20 => { self.reg.b = self.alu_sla(self.reg.b); 2 }
            0x21 => { self.reg.c = self.alu_sla(self.reg.c); 2 }
            0x22 => { self.reg.d = self.alu_sla(self.reg.d); 2 }
            0x23 => { self.reg.e = self.alu_sla(self.reg.e); 2 }
            0x24 => { self.reg.h = self.alu_sla(self.reg.h); 2 }
            0x25 => { self.reg.l = self.alu_sla(self.reg.l); 2 }
            0x26 => { let a = self.reg.hl(); let v = self.mmu.rb(a); let v2 = self.alu_sla(v); self.mmu.wb(a, v2); 4 }
            0x27 => { self.reg.a = self.alu_sla(self.reg.a); 2 }
            // SRA r
            0x28 => { self.reg.b = self.alu_sra(self.reg.b); 2 }
            0x29 => { self.reg.c = self.alu_sra(self.reg.c); 2 }
            0x2A => { self.reg.d = self.alu_sra(self.reg.d); 2 }
            0x2B => { self.reg.e = self.alu_sra(self.reg.e); 2 }
            0x2C => { self.reg.h = self.alu_sra(self.reg.h); 2 }
            0x2D => { self.reg.l = self.alu_sra(self.reg.l); 2 }
            0x2E => { let a = self.reg.hl(); let v = self.mmu.rb(a); let v2 = self.alu_sra(v); self.mmu.wb(a, v2); 4 }
            0x2F => { self.reg.a = self.alu_sra(self.reg.a); 2 }
            // SWAP r
            0x30 => { self.reg.b = self.alu_swap(self.reg.b); 2 }
            0x31 => { self.reg.c = self.alu_swap(self.reg.c); 2 }
            0x32 => { self.reg.d = self.alu_swap(self.reg.d); 2 }
            0x33 => { self.reg.e = self.alu_swap(self.reg.e); 2 }
            0x34 => { self.reg.h = self.alu_swap(self.reg.h); 2 }
            0x35 => { self.reg.l = self.alu_swap(self.reg.l); 2 }
            0x36 => { let a = self.reg.hl(); let v = self.mmu.rb(a); let v2 = self.alu_swap(v); self.mmu.wb(a, v2); 4 }
            0x37 => { self.reg.a = self.alu_swap(self.reg.a); 2 }
            // SRL r
            0x38 => { self.reg.b = self.alu_srl(self.reg.b); 2 }
            0x39 => { self.reg.c = self.alu_srl(self.reg.c); 2 }
            0x3A => { self.reg.d = self.alu_srl(self.reg.d); 2 }
            0x3B => { self.reg.e = self.alu_srl(self.reg.e); 2 }
            0x3C => { self.reg.h = self.alu_srl(self.reg.h); 2 }
            0x3D => { self.reg.l = self.alu_srl(self.reg.l); 2 }
            0x3E => { let a = self.reg.hl(); let v = self.mmu.rb(a); let v2 = self.alu_srl(v); self.mmu.wb(a, v2); 4 }
            0x3F => { self.reg.a = self.alu_srl(self.reg.a); 2 }
            // BIT b, r (0x40-0x7F)
            0x40..=0x47 => { let v = self.get_reg_cb(opcode); self.alu_bit(v, 0); if opcode == 0x46 { 3 } else { 2 } }
            0x48..=0x4F => { let v = self.get_reg_cb(opcode); self.alu_bit(v, 1); if opcode == 0x4E { 3 } else { 2 } }
            0x50..=0x57 => { let v = self.get_reg_cb(opcode); self.alu_bit(v, 2); if opcode == 0x56 { 3 } else { 2 } }
            0x58..=0x5F => { let v = self.get_reg_cb(opcode); self.alu_bit(v, 3); if opcode == 0x5E { 3 } else { 2 } }
            0x60..=0x67 => { let v = self.get_reg_cb(opcode); self.alu_bit(v, 4); if opcode == 0x66 { 3 } else { 2 } }
            0x68..=0x6F => { let v = self.get_reg_cb(opcode); self.alu_bit(v, 5); if opcode == 0x6E { 3 } else { 2 } }
            0x70..=0x77 => { let v = self.get_reg_cb(opcode); self.alu_bit(v, 6); if opcode == 0x76 { 3 } else { 2 } }
            0x78..=0x7F => { let v = self.get_reg_cb(opcode); self.alu_bit(v, 7); if opcode == 0x7E { 3 } else { 2 } }
            // RES b, r (0x80-0xBF)
            0x80..=0x87 => { let v = self.get_reg_cb(opcode) & !(1 << 0); self.set_reg_cb(opcode, v); if opcode == 0x86 { 4 } else { 2 } }
            0x88..=0x8F => { let v = self.get_reg_cb(opcode) & !(1 << 1); self.set_reg_cb(opcode, v); if opcode == 0x8E { 4 } else { 2 } }
            0x90..=0x97 => { let v = self.get_reg_cb(opcode) & !(1 << 2); self.set_reg_cb(opcode, v); if opcode == 0x96 { 4 } else { 2 } }
            0x98..=0x9F => { let v = self.get_reg_cb(opcode) & !(1 << 3); self.set_reg_cb(opcode, v); if opcode == 0x9E { 4 } else { 2 } }
            0xA0..=0xA7 => { let v = self.get_reg_cb(opcode) & !(1 << 4); self.set_reg_cb(opcode, v); if opcode == 0xA6 { 4 } else { 2 } }
            0xA8..=0xAF => { let v = self.get_reg_cb(opcode) & !(1 << 5); self.set_reg_cb(opcode, v); if opcode == 0xAE { 4 } else { 2 } }
            0xB0..=0xB7 => { let v = self.get_reg_cb(opcode) & !(1 << 6); self.set_reg_cb(opcode, v); if opcode == 0xB6 { 4 } else { 2 } }
            0xB8..=0xBF => { let v = self.get_reg_cb(opcode) & !(1 << 7); self.set_reg_cb(opcode, v); if opcode == 0xBE { 4 } else { 2 } }
            // SET b, r (0xC0-0xFF)
            0xC0..=0xC7 => { let v = self.get_reg_cb(opcode) | (1 << 0); self.set_reg_cb(opcode, v); if opcode == 0xC6 { 4 } else { 2 } }
            0xC8..=0xCF => { let v = self.get_reg_cb(opcode) | (1 << 1); self.set_reg_cb(opcode, v); if opcode == 0xCE { 4 } else { 2 } }
            0xD0..=0xD7 => { let v = self.get_reg_cb(opcode) | (1 << 2); self.set_reg_cb(opcode, v); if opcode == 0xD6 { 4 } else { 2 } }
            0xD8..=0xDF => { let v = self.get_reg_cb(opcode) | (1 << 3); self.set_reg_cb(opcode, v); if opcode == 0xDE { 4 } else { 2 } }
            0xE0..=0xE7 => { let v = self.get_reg_cb(opcode) | (1 << 4); self.set_reg_cb(opcode, v); if opcode == 0xE6 { 4 } else { 2 } }
            0xE8..=0xEF => { let v = self.get_reg_cb(opcode) | (1 << 5); self.set_reg_cb(opcode, v); if opcode == 0xEE { 4 } else { 2 } }
            0xF0..=0xF7 => { let v = self.get_reg_cb(opcode) | (1 << 6); self.set_reg_cb(opcode, v); if opcode == 0xF6 { 4 } else { 2 } }
            0xF8..=0xFF => { let v = self.get_reg_cb(opcode) | (1 << 7); self.set_reg_cb(opcode, v); if opcode == 0xFE { 4 } else { 2 } }
        }
    }

    /// Get register value for CB-prefixed instructions
    #[inline]
    fn get_reg_cb(&mut self, opcode: u8) -> u8 {
        match opcode & 0x07 {
            0 => self.reg.b,
            1 => self.reg.c,
            2 => self.reg.d,
            3 => self.reg.e,
            4 => self.reg.h,
            5 => self.reg.l,
            6 => self.mmu.rb(self.reg.hl()),
            7 => self.reg.a,
            _ => unreachable!(),
        }
    }

    /// Set register value for CB-prefixed instructions
    #[inline]
    fn set_reg_cb(&mut self, opcode: u8, value: u8) {
        match opcode & 0x07 {
            0 => self.reg.b = value,
            1 => self.reg.c = value,
            2 => self.reg.d = value,
            3 => self.reg.e = value,
            4 => self.reg.h = value,
            5 => self.reg.l = value,
            6 => self.mmu.wb(self.reg.hl(), value),
            7 => self.reg.a = value,
            _ => unreachable!(),
        }
    }

    // =====================================================================
    // ALU Operations
    // =====================================================================

    #[inline]
    fn alu_add(&mut self, b: u8, usec: bool) {
        let c = if usec && self.reg.getflag(C) { 1 } else { 0 };
        let a = self.reg.a;
        let r = a.wrapping_add(b).wrapping_add(c);
        self.reg.flag(Z, r == 0);
        self.reg.flag(H, (a & 0xF) + (b & 0xF) + c > 0xF);
        self.reg.flag(N, false);
        self.reg.flag(C, (a as u16) + (b as u16) + (c as u16) > 0xFF);
        self.reg.a = r;
    }

    #[inline]
    fn alu_sub(&mut self, b: u8, usec: bool) {
        let c = if usec && self.reg.getflag(C) { 1 } else { 0 };
        let a = self.reg.a;
        let r = a.wrapping_sub(b).wrapping_sub(c);
        self.reg.flag(Z, r == 0);
        self.reg.flag(H, (a & 0x0F) < (b & 0x0F) + c);
        self.reg.flag(N, true);
        self.reg.flag(C, (a as u16) < (b as u16) + (c as u16));
        self.reg.a = r;
    }

    #[inline]
    fn alu_and(&mut self, b: u8) {
        let r = self.reg.a & b;
        self.reg.flag(Z, r == 0);
        self.reg.flag(H, true);
        self.reg.flag(C, false);
        self.reg.flag(N, false);
        self.reg.a = r;
    }

    #[inline]
    fn alu_or(&mut self, b: u8) {
        let r = self.reg.a | b;
        self.reg.flag(Z, r == 0);
        self.reg.flag(C, false);
        self.reg.flag(H, false);
        self.reg.flag(N, false);
        self.reg.a = r;
    }

    #[inline]
    fn alu_xor(&mut self, b: u8) {
        let r = self.reg.a ^ b;
        self.reg.flag(Z, r == 0);
        self.reg.flag(C, false);
        self.reg.flag(H, false);
        self.reg.flag(N, false);
        self.reg.a = r;
    }

    #[inline]
    fn alu_cp(&mut self, b: u8) {
        let a = self.reg.a;
        self.alu_sub(b, false);
        self.reg.a = a;
    }

    #[inline]
    fn alu_inc(&mut self, a: u8) -> u8 {
        let r = a.wrapping_add(1);
        self.reg.flag(Z, r == 0);
        self.reg.flag(H, (a & 0x0F) + 1 > 0x0F);
        self.reg.flag(N, false);
        r
    }

    #[inline]
    fn alu_dec(&mut self, a: u8) -> u8 {
        let r = a.wrapping_sub(1);
        self.reg.flag(Z, r == 0);
        self.reg.flag(H, (a & 0x0F) == 0);
        self.reg.flag(N, true);
        r
    }

    #[inline]
    fn alu_add16(&mut self, b: u16) {
        let a = self.reg.hl();
        let r = a.wrapping_add(b);
        self.reg.flag(H, (a & 0x0FFF) + (b & 0x0FFF) > 0x0FFF);
        self.reg.flag(N, false);
        self.reg.flag(C, a > 0xFFFF - b);
        self.reg.sethl(r);
    }

    #[inline]
    fn alu_add16imm(&mut self, a: u16) -> u16 {
        let b = self.fetchbyte() as i8 as i16 as u16;
        self.reg.flag(N, false);
        self.reg.flag(Z, false);
        self.reg.flag(H, (a & 0x000F) + (b & 0x000F) > 0x000F);
        self.reg.flag(C, (a & 0x00FF) + (b & 0x00FF) > 0x00FF);
        a.wrapping_add(b)
    }

    #[inline]
    fn alu_swap(&mut self, a: u8) -> u8 {
        self.reg.flag(Z, a == 0);
        self.reg.flag(C, false);
        self.reg.flag(H, false);
        self.reg.flag(N, false);
        (a >> 4) | (a << 4)
    }

    #[inline]
    fn alu_srflagupdate(&mut self, r: u8, c: bool) {
        self.reg.flag(H, false);
        self.reg.flag(N, false);
        self.reg.flag(Z, r == 0);
        self.reg.flag(C, c);
    }

    #[inline]
    fn alu_rlc(&mut self, a: u8) -> u8 {
        let c = a & 0x80 == 0x80;
        let r = (a << 1) | (if c { 1 } else { 0 });
        self.alu_srflagupdate(r, c);
        r
    }

    #[inline]
    fn alu_rl(&mut self, a: u8) -> u8 {
        let c = a & 0x80 == 0x80;
        let r = (a << 1) | (if self.reg.getflag(C) { 1 } else { 0 });
        self.alu_srflagupdate(r, c);
        r
    }

    #[inline]
    fn alu_rrc(&mut self, a: u8) -> u8 {
        let c = a & 0x01 == 0x01;
        let r = (a >> 1) | (if c { 0x80 } else { 0 });
        self.alu_srflagupdate(r, c);
        r
    }

    #[inline]
    fn alu_rr(&mut self, a: u8) -> u8 {
        let c = a & 0x01 == 0x01;
        let r = (a >> 1) | (if self.reg.getflag(C) { 0x80 } else { 0 });
        self.alu_srflagupdate(r, c);
        r
    }

    #[inline]
    fn alu_sla(&mut self, a: u8) -> u8 {
        let c = a & 0x80 == 0x80;
        let r = a << 1;
        self.alu_srflagupdate(r, c);
        r
    }

    #[inline]
    fn alu_sra(&mut self, a: u8) -> u8 {
        let c = a & 0x01 == 0x01;
        let r = (a >> 1) | (a & 0x80);
        self.alu_srflagupdate(r, c);
        r
    }

    #[inline]
    fn alu_srl(&mut self, a: u8) -> u8 {
        let c = a & 0x01 == 0x01;
        let r = a >> 1;
        self.alu_srflagupdate(r, c);
        r
    }

    #[inline]
    fn alu_bit(&mut self, a: u8, b: u8) {
        let r = a & (1 << (b as u32)) == 0;
        self.reg.flag(N, false);
        self.reg.flag(H, true);
        self.reg.flag(Z, r);
    }

    #[inline]
    fn alu_daa(&mut self) {
        let mut a = self.reg.a;
        let mut adjust = if self.reg.getflag(C) { 0x60 } else { 0x00 };
        if self.reg.getflag(H) {
            adjust |= 0x06;
        }
        if !self.reg.getflag(N) {
            if a & 0x0F > 0x09 {
                adjust |= 0x06;
            }
            if a > 0x99 {
                adjust |= 0x60;
            }
            a = a.wrapping_add(adjust);
        } else {
            a = a.wrapping_sub(adjust);
        }

        self.reg.flag(C, adjust >= 0x60);
        self.reg.flag(H, false);
        self.reg.flag(Z, a == 0);
        self.reg.a = a;
    }

    #[inline]
    fn cpu_jr(&mut self) {
        let n = self.fetchbyte() as i8;
        self.reg.pc = ((self.reg.pc as u32 as i32) + (n as i32)) as u16;
    }

    // =====================================================================
    // Debug/Serialization
    // =====================================================================

    /// Read a byte from memory (for debugging)
    #[inline]
    pub fn read_byte(&mut self, address: u16) -> u8 {
        self.mmu.rb(address)
    }

    /// Write a byte to memory (for debugging/cheats)
    #[inline]
    pub fn write_byte(&mut self, address: u16, byte: u8) {
        self.mmu.wb(address, byte)
    }

    /// Read a word from memory (for debugging)
    #[inline]
    pub fn read_wide(&mut self, address: u16) -> u16 {
        self.mmu.rw(address)
    }

    /// Write a word to memory (for debugging/cheats)
    #[inline]
    pub fn write_wide(&mut self, address: u16, wide: u16) {
        self.mmu.ww(address, wide)
    }

    /// Serialize CPU state
    pub fn serialize(&self, output: &mut Vec<u8>) {
        // Version
        output.push(1);
        
        // Registers
        self.reg.serialize(output);
        
        // CPU state
        output.push(if self.halted { 1 } else { 0 });
        output.push(if self.halt_bug { 1 } else { 0 });
        output.push(if self.ime { 1 } else { 0 });
        output.push(self.setdi as u8);
        output.push(self.setei as u8);
        
        // MMU
        self.mmu.serialize(output);
    }

    /// Deserialize CPU state
    pub fn deserialize(&mut self, data: &[u8]) -> Result<usize, ()> {
        if data.is_empty() {
            return Err(());
        }
        
        // Version check
        if data[0] != 1 {
            return Err(());
        }
        let mut offset = 1;
        
        // Registers
        offset += self.reg.deserialize(&data[offset..])?;
        
        // CPU state
        if data.len() < offset + 5 {
            return Err(());
        }
        self.halted = data[offset] != 0;
        self.halt_bug = data[offset + 1] != 0;
        self.ime = data[offset + 2] != 0;
        self.setdi = data[offset + 3] as u32;
        self.setei = data[offset + 4] as u32;
        offset += 5;
        
        // MMU
        offset += self.mmu.deserialize(&data[offset..])?;
        
        Ok(offset)
    }
}
