//! GameBoy CPU (LR35902) Emulation
//!
//! Sharp LR35902 - a Z80 derivative with some differences.
//! Runs at 4.19 MHz (or 8.38 MHz in CGB double-speed mode).
//!
//! OPTIMIZED VERSION: Added #[inline] hints for ARM bare-metal performance.

extern crate alloc;

use alloc::boxed::Box;
use super::mbc;
use super::mmu::MMU;
use super::register::{CpuFlag, Registers};
use super::register::CpuFlag::{C, H, N, Z};
use super::StrResult;

/// CPU state
pub struct CPU {
    pub reg: Registers,
    pub mmu: MMU,
    halted: bool,
    halt_bug: bool,
    ime: bool,      // Interrupt master enable
    setdi: u32,     // Delayed DI
    setei: u32,     // Delayed EI
}

impl CPU {
    /// Create CPU for classic GameBoy
    pub fn new(cart: Box<dyn mbc::MBC + 'static>) -> StrResult<CPU> {
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

    /// Create CPU for GameBoy Color
    pub fn new_cgb(cart: Box<dyn mbc::MBC + 'static>) -> StrResult<CPU> {
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

    /// Execute one instruction cycle
    #[inline]
    pub fn do_cycle(&mut self) -> u32 {
        self.update_ime();
        let ticks = self.handle_interrupts();
        if ticks != 0 {
            return self.mmu.do_cycle(ticks);
        }

        let ticks = if self.halted {
            4
        } else {
            self.execute()
        };

        self.mmu.do_cycle(ticks)
    }

    #[inline(always)]
    fn update_ime(&mut self) {
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

    #[inline(always)]
    fn handle_interrupts(&mut self) -> u32 {
        if !self.ime && !self.halted {
            return 0;
        }

        let triggered = self.mmu.inte & self.mmu.intf;
        if triggered == 0 {
            return 0;
        }

        self.halted = false;
        if !self.ime {
            return 0;
        }
        self.ime = false;

        // Find highest priority interrupt
        let interrupt = triggered.trailing_zeros();
        if interrupt >= 5 {
            return 0;
        }

        // Clear interrupt flag
        self.mmu.intf &= !(1 << interrupt);

        // Push PC and jump to handler
        self.push(self.reg.pc);
        self.reg.pc = 0x0040 + (interrupt as u16) * 8;

        16
    }

    /// Read byte at PC and increment
    #[inline(always)]
    fn fetchbyte(&mut self) -> u8 {
        let b = self.mmu.rb(self.reg.pc);
        if !self.halt_bug {
            self.reg.pc = self.reg.pc.wrapping_add(1);
        }
        self.halt_bug = false;
        b
    }

    /// Read word at PC and increment
    #[inline(always)]
    fn fetchword(&mut self) -> u16 {
        let w = self.mmu.rw(self.reg.pc);
        self.reg.pc = self.reg.pc.wrapping_add(2);
        w
    }

    /// Push word onto stack
    #[inline(always)]
    fn push(&mut self, value: u16) {
        self.reg.sp = self.reg.sp.wrapping_sub(2);
        self.mmu.ww(self.reg.sp, value);
    }

    /// Pop word from stack
    #[inline(always)]
    fn pop(&mut self) -> u16 {
        let value = self.mmu.rw(self.reg.sp);
        self.reg.sp = self.reg.sp.wrapping_add(2);
        value
    }

    /// Read byte from memory
    #[inline(always)]
    pub fn read_byte(&mut self, addr: u16) -> u8 {
        self.mmu.rb(addr)
    }

    /// Write byte to memory
    #[inline(always)]
    pub fn write_byte(&mut self, addr: u16, value: u8) {
        self.mmu.wb(addr, value);
    }

    /// Read word from memory
    #[inline(always)]
    pub fn read_wide(&mut self, addr: u16) -> u16 {
        self.mmu.rw(addr)
    }

    /// Write word to memory
    #[inline(always)]
    pub fn write_wide(&mut self, addr: u16, value: u16) {
        self.mmu.ww(addr, value);
    }

    /// Execute one instruction
    #[inline]
    fn execute(&mut self) -> u32 {
        let opcode = self.fetchbyte();
        match opcode {
            0x00 => 4, // NOP
            0x01 => { let v = self.fetchword(); self.reg.set_bc(v); 12 } // LD BC,nn
            0x02 => { self.mmu.wb(self.reg.bc(), self.reg.a); 8 } // LD (BC),A
            0x03 => { self.reg.set_bc(self.reg.bc().wrapping_add(1)); 8 } // INC BC
            0x04 => { self.reg.b = self.alu_inc(self.reg.b); 4 } // INC B
            0x05 => { self.reg.b = self.alu_dec(self.reg.b); 4 } // DEC B
            0x06 => { self.reg.b = self.fetchbyte(); 8 } // LD B,n
            0x07 => { self.reg.a = self.alu_rlc(self.reg.a); self.reg.set_flag(Z, false); 4 } // RLCA
            0x08 => { let a = self.fetchword(); self.mmu.ww(a, self.reg.sp); 20 } // LD (nn),SP
            0x09 => { self.alu_add16(self.reg.bc()); 8 } // ADD HL,BC
            0x0A => { self.reg.a = self.mmu.rb(self.reg.bc()); 8 } // LD A,(BC)
            0x0B => { self.reg.set_bc(self.reg.bc().wrapping_sub(1)); 8 } // DEC BC
            0x0C => { self.reg.c = self.alu_inc(self.reg.c); 4 } // INC C
            0x0D => { self.reg.c = self.alu_dec(self.reg.c); 4 } // DEC C
            0x0E => { self.reg.c = self.fetchbyte(); 8 } // LD C,n
            0x0F => { self.reg.a = self.alu_rrc(self.reg.a); self.reg.set_flag(Z, false); 4 } // RRCA

            0x10 => { // STOP
                self.mmu.switch_speed();
                4
            }
            0x11 => { let v = self.fetchword(); self.reg.set_de(v); 12 } // LD DE,nn
            0x12 => { self.mmu.wb(self.reg.de(), self.reg.a); 8 } // LD (DE),A
            0x13 => { self.reg.set_de(self.reg.de().wrapping_add(1)); 8 } // INC DE
            0x14 => { self.reg.d = self.alu_inc(self.reg.d); 4 } // INC D
            0x15 => { self.reg.d = self.alu_dec(self.reg.d); 4 } // DEC D
            0x16 => { self.reg.d = self.fetchbyte(); 8 } // LD D,n
            0x17 => { self.reg.a = self.alu_rl(self.reg.a); self.reg.set_flag(Z, false); 4 } // RLA
            0x18 => { self.cpu_jr(); 12 } // JR n
            0x19 => { self.alu_add16(self.reg.de()); 8 } // ADD HL,DE
            0x1A => { self.reg.a = self.mmu.rb(self.reg.de()); 8 } // LD A,(DE)
            0x1B => { self.reg.set_de(self.reg.de().wrapping_sub(1)); 8 } // DEC DE
            0x1C => { self.reg.e = self.alu_inc(self.reg.e); 4 } // INC E
            0x1D => { self.reg.e = self.alu_dec(self.reg.e); 4 } // DEC E
            0x1E => { self.reg.e = self.fetchbyte(); 8 } // LD E,n
            0x1F => { self.reg.a = self.alu_rr(self.reg.a); self.reg.set_flag(Z, false); 4 } // RRA

            0x20 => { if !self.reg.flag(Z) { self.cpu_jr(); 12 } else { self.reg.pc = self.reg.pc.wrapping_add(1); 8 } } // JR NZ,n
            0x21 => { let v = self.fetchword(); self.reg.set_hl(v); 12 } // LD HL,nn
            0x22 => { self.mmu.wb(self.reg.hl(), self.reg.a); self.reg.set_hl(self.reg.hl().wrapping_add(1)); 8 } // LD (HL+),A
            0x23 => { self.reg.set_hl(self.reg.hl().wrapping_add(1)); 8 } // INC HL
            0x24 => { self.reg.h = self.alu_inc(self.reg.h); 4 } // INC H
            0x25 => { self.reg.h = self.alu_dec(self.reg.h); 4 } // DEC H
            0x26 => { self.reg.h = self.fetchbyte(); 8 } // LD H,n
            0x27 => { self.alu_daa(); 4 } // DAA
            0x28 => { if self.reg.flag(Z) { self.cpu_jr(); 12 } else { self.reg.pc = self.reg.pc.wrapping_add(1); 8 } } // JR Z,n
            0x29 => { self.alu_add16(self.reg.hl()); 8 } // ADD HL,HL
            0x2A => { self.reg.a = self.mmu.rb(self.reg.hl()); self.reg.set_hl(self.reg.hl().wrapping_add(1)); 8 } // LD A,(HL+)
            0x2B => { self.reg.set_hl(self.reg.hl().wrapping_sub(1)); 8 } // DEC HL
            0x2C => { self.reg.l = self.alu_inc(self.reg.l); 4 } // INC L
            0x2D => { self.reg.l = self.alu_dec(self.reg.l); 4 } // DEC L
            0x2E => { self.reg.l = self.fetchbyte(); 8 } // LD L,n
            0x2F => { self.reg.a = !self.reg.a; self.reg.set_flag(N, true); self.reg.set_flag(H, true); 4 } // CPL

            0x30 => { if !self.reg.flag(C) { self.cpu_jr(); 12 } else { self.reg.pc = self.reg.pc.wrapping_add(1); 8 } } // JR NC,n
            0x31 => { self.reg.sp = self.fetchword(); 12 } // LD SP,nn
            0x32 => { self.mmu.wb(self.reg.hl(), self.reg.a); self.reg.set_hl(self.reg.hl().wrapping_sub(1)); 8 } // LD (HL-),A
            0x33 => { self.reg.sp = self.reg.sp.wrapping_add(1); 8 } // INC SP
            0x34 => { let hl = self.reg.hl(); let byte = self.mmu.rb(hl); let v = self.alu_inc(byte); self.mmu.wb(hl, v); 12 } // INC (HL)
            0x35 => { let hl = self.reg.hl(); let byte = self.mmu.rb(hl); let v = self.alu_dec(byte); self.mmu.wb(hl, v); 12 } // DEC (HL)
            0x36 => { let v = self.fetchbyte(); self.mmu.wb(self.reg.hl(), v); 12 } // LD (HL),n
            0x37 => { self.reg.set_flag(N, false); self.reg.set_flag(H, false); self.reg.set_flag(C, true); 4 } // SCF
            0x38 => { if self.reg.flag(C) { self.cpu_jr(); 12 } else { self.reg.pc = self.reg.pc.wrapping_add(1); 8 } } // JR C,n
            0x39 => { self.alu_add16(self.reg.sp); 8 } // ADD HL,SP
            0x3A => { self.reg.a = self.mmu.rb(self.reg.hl()); self.reg.set_hl(self.reg.hl().wrapping_sub(1)); 8 } // LD A,(HL-)
            0x3B => { self.reg.sp = self.reg.sp.wrapping_sub(1); 8 } // DEC SP
            0x3C => { self.reg.a = self.alu_inc(self.reg.a); 4 } // INC A
            0x3D => { self.reg.a = self.alu_dec(self.reg.a); 4 } // DEC A
            0x3E => { self.reg.a = self.fetchbyte(); 8 } // LD A,n
            0x3F => { let c = !self.reg.flag(C); self.reg.set_flag(N, false); self.reg.set_flag(H, false); self.reg.set_flag(C, c); 4 } // CCF

            // LD r,r' instructions (0x40-0x7F except 0x76)
            0x40 => 4, // LD B,B
            0x41 => { self.reg.b = self.reg.c; 4 }
            0x42 => { self.reg.b = self.reg.d; 4 }
            0x43 => { self.reg.b = self.reg.e; 4 }
            0x44 => { self.reg.b = self.reg.h; 4 }
            0x45 => { self.reg.b = self.reg.l; 4 }
            0x46 => { self.reg.b = self.mmu.rb(self.reg.hl()); 8 }
            0x47 => { self.reg.b = self.reg.a; 4 }
            0x48 => { self.reg.c = self.reg.b; 4 }
            0x49 => 4, // LD C,C
            0x4A => { self.reg.c = self.reg.d; 4 }
            0x4B => { self.reg.c = self.reg.e; 4 }
            0x4C => { self.reg.c = self.reg.h; 4 }
            0x4D => { self.reg.c = self.reg.l; 4 }
            0x4E => { self.reg.c = self.mmu.rb(self.reg.hl()); 8 }
            0x4F => { self.reg.c = self.reg.a; 4 }
            0x50 => { self.reg.d = self.reg.b; 4 }
            0x51 => { self.reg.d = self.reg.c; 4 }
            0x52 => 4, // LD D,D
            0x53 => { self.reg.d = self.reg.e; 4 }
            0x54 => { self.reg.d = self.reg.h; 4 }
            0x55 => { self.reg.d = self.reg.l; 4 }
            0x56 => { self.reg.d = self.mmu.rb(self.reg.hl()); 8 }
            0x57 => { self.reg.d = self.reg.a; 4 }
            0x58 => { self.reg.e = self.reg.b; 4 }
            0x59 => { self.reg.e = self.reg.c; 4 }
            0x5A => { self.reg.e = self.reg.d; 4 }
            0x5B => 4, // LD E,E
            0x5C => { self.reg.e = self.reg.h; 4 }
            0x5D => { self.reg.e = self.reg.l; 4 }
            0x5E => { self.reg.e = self.mmu.rb(self.reg.hl()); 8 }
            0x5F => { self.reg.e = self.reg.a; 4 }
            0x60 => { self.reg.h = self.reg.b; 4 }
            0x61 => { self.reg.h = self.reg.c; 4 }
            0x62 => { self.reg.h = self.reg.d; 4 }
            0x63 => { self.reg.h = self.reg.e; 4 }
            0x64 => 4, // LD H,H
            0x65 => { self.reg.h = self.reg.l; 4 }
            0x66 => { self.reg.h = self.mmu.rb(self.reg.hl()); 8 }
            0x67 => { self.reg.h = self.reg.a; 4 }
            0x68 => { self.reg.l = self.reg.b; 4 }
            0x69 => { self.reg.l = self.reg.c; 4 }
            0x6A => { self.reg.l = self.reg.d; 4 }
            0x6B => { self.reg.l = self.reg.e; 4 }
            0x6C => { self.reg.l = self.reg.h; 4 }
            0x6D => 4, // LD L,L
            0x6E => { self.reg.l = self.mmu.rb(self.reg.hl()); 8 }
            0x6F => { self.reg.l = self.reg.a; 4 }
            0x70 => { self.mmu.wb(self.reg.hl(), self.reg.b); 8 }
            0x71 => { self.mmu.wb(self.reg.hl(), self.reg.c); 8 }
            0x72 => { self.mmu.wb(self.reg.hl(), self.reg.d); 8 }
            0x73 => { self.mmu.wb(self.reg.hl(), self.reg.e); 8 }
            0x74 => { self.mmu.wb(self.reg.hl(), self.reg.h); 8 }
            0x75 => { self.mmu.wb(self.reg.hl(), self.reg.l); 8 }
            0x76 => { // HALT
                self.halted = true;
                if !self.ime && (self.mmu.inte & self.mmu.intf) != 0 {
                    self.halt_bug = true;
                }
                4
            }
            0x77 => { self.mmu.wb(self.reg.hl(), self.reg.a); 8 }
            0x78 => { self.reg.a = self.reg.b; 4 }
            0x79 => { self.reg.a = self.reg.c; 4 }
            0x7A => { self.reg.a = self.reg.d; 4 }
            0x7B => { self.reg.a = self.reg.e; 4 }
            0x7C => { self.reg.a = self.reg.h; 4 }
            0x7D => { self.reg.a = self.reg.l; 4 }
            0x7E => { self.reg.a = self.mmu.rb(self.reg.hl()); 8 }
            0x7F => 4, // LD A,A

            // ALU operations (0x80-0xBF)
            0x80 => { self.alu_add(self.reg.b, false); 4 }
            0x81 => { self.alu_add(self.reg.c, false); 4 }
            0x82 => { self.alu_add(self.reg.d, false); 4 }
            0x83 => { self.alu_add(self.reg.e, false); 4 }
            0x84 => { self.alu_add(self.reg.h, false); 4 }
            0x85 => { self.alu_add(self.reg.l, false); 4 }
            0x86 => { let v = self.mmu.rb(self.reg.hl()); self.alu_add(v, false); 8 }
            0x87 => { self.alu_add(self.reg.a, false); 4 }
            0x88 => { self.alu_add(self.reg.b, true); 4 }
            0x89 => { self.alu_add(self.reg.c, true); 4 }
            0x8A => { self.alu_add(self.reg.d, true); 4 }
            0x8B => { self.alu_add(self.reg.e, true); 4 }
            0x8C => { self.alu_add(self.reg.h, true); 4 }
            0x8D => { self.alu_add(self.reg.l, true); 4 }
            0x8E => { let v = self.mmu.rb(self.reg.hl()); self.alu_add(v, true); 8 }
            0x8F => { self.alu_add(self.reg.a, true); 4 }
            0x90 => { self.alu_sub(self.reg.b, false); 4 }
            0x91 => { self.alu_sub(self.reg.c, false); 4 }
            0x92 => { self.alu_sub(self.reg.d, false); 4 }
            0x93 => { self.alu_sub(self.reg.e, false); 4 }
            0x94 => { self.alu_sub(self.reg.h, false); 4 }
            0x95 => { self.alu_sub(self.reg.l, false); 4 }
            0x96 => { let v = self.mmu.rb(self.reg.hl()); self.alu_sub(v, false); 8 }
            0x97 => { self.alu_sub(self.reg.a, false); 4 }
            0x98 => { self.alu_sub(self.reg.b, true); 4 }
            0x99 => { self.alu_sub(self.reg.c, true); 4 }
            0x9A => { self.alu_sub(self.reg.d, true); 4 }
            0x9B => { self.alu_sub(self.reg.e, true); 4 }
            0x9C => { self.alu_sub(self.reg.h, true); 4 }
            0x9D => { self.alu_sub(self.reg.l, true); 4 }
            0x9E => { let v = self.mmu.rb(self.reg.hl()); self.alu_sub(v, true); 8 }
            0x9F => { self.alu_sub(self.reg.a, true); 4 }
            0xA0 => { self.alu_and(self.reg.b); 4 }
            0xA1 => { self.alu_and(self.reg.c); 4 }
            0xA2 => { self.alu_and(self.reg.d); 4 }
            0xA3 => { self.alu_and(self.reg.e); 4 }
            0xA4 => { self.alu_and(self.reg.h); 4 }
            0xA5 => { self.alu_and(self.reg.l); 4 }
            0xA6 => { let v = self.mmu.rb(self.reg.hl()); self.alu_and(v); 8 }
            0xA7 => { self.alu_and(self.reg.a); 4 }
            0xA8 => { self.alu_xor(self.reg.b); 4 }
            0xA9 => { self.alu_xor(self.reg.c); 4 }
            0xAA => { self.alu_xor(self.reg.d); 4 }
            0xAB => { self.alu_xor(self.reg.e); 4 }
            0xAC => { self.alu_xor(self.reg.h); 4 }
            0xAD => { self.alu_xor(self.reg.l); 4 }
            0xAE => { let v = self.mmu.rb(self.reg.hl()); self.alu_xor(v); 8 }
            0xAF => { self.alu_xor(self.reg.a); 4 }
            0xB0 => { self.alu_or(self.reg.b); 4 }
            0xB1 => { self.alu_or(self.reg.c); 4 }
            0xB2 => { self.alu_or(self.reg.d); 4 }
            0xB3 => { self.alu_or(self.reg.e); 4 }
            0xB4 => { self.alu_or(self.reg.h); 4 }
            0xB5 => { self.alu_or(self.reg.l); 4 }
            0xB6 => { let v = self.mmu.rb(self.reg.hl()); self.alu_or(v); 8 }
            0xB7 => { self.alu_or(self.reg.a); 4 }
            0xB8 => { self.alu_cp(self.reg.b); 4 }
            0xB9 => { self.alu_cp(self.reg.c); 4 }
            0xBA => { self.alu_cp(self.reg.d); 4 }
            0xBB => { self.alu_cp(self.reg.e); 4 }
            0xBC => { self.alu_cp(self.reg.h); 4 }
            0xBD => { self.alu_cp(self.reg.l); 4 }
            0xBE => { let v = self.mmu.rb(self.reg.hl()); self.alu_cp(v); 8 }
            0xBF => { self.alu_cp(self.reg.a); 4 }

            // Control flow and misc (0xC0-0xFF)
            0xC0 => { if !self.reg.flag(Z) { self.reg.pc = self.pop(); 20 } else { 8 } } // RET NZ
            0xC1 => { let v = self.pop(); self.reg.set_bc(v); 12 } // POP BC
            0xC2 => { if !self.reg.flag(Z) { self.reg.pc = self.fetchword(); 16 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 12 } } // JP NZ,nn
            0xC3 => { self.reg.pc = self.fetchword(); 16 } // JP nn
            0xC4 => { if !self.reg.flag(Z) { let a = self.fetchword(); self.push(self.reg.pc); self.reg.pc = a; 24 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 12 } } // CALL NZ,nn
            0xC5 => { self.push(self.reg.bc()); 16 } // PUSH BC
            0xC6 => { let v = self.fetchbyte(); self.alu_add(v, false); 8 } // ADD A,n
            0xC7 => { self.push(self.reg.pc); self.reg.pc = 0x00; 16 } // RST 00
            0xC8 => { if self.reg.flag(Z) { self.reg.pc = self.pop(); 20 } else { 8 } } // RET Z
            0xC9 => { self.reg.pc = self.pop(); 16 } // RET
            0xCA => { if self.reg.flag(Z) { self.reg.pc = self.fetchword(); 16 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 12 } } // JP Z,nn
            0xCB => { self.execute_cb() } // CB prefix
            0xCC => { if self.reg.flag(Z) { let a = self.fetchword(); self.push(self.reg.pc); self.reg.pc = a; 24 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 12 } } // CALL Z,nn
            0xCD => { let a = self.fetchword(); self.push(self.reg.pc); self.reg.pc = a; 24 } // CALL nn
            0xCE => { let v = self.fetchbyte(); self.alu_add(v, true); 8 } // ADC A,n
            0xCF => { self.push(self.reg.pc); self.reg.pc = 0x08; 16 } // RST 08

            0xD0 => { if !self.reg.flag(C) { self.reg.pc = self.pop(); 20 } else { 8 } } // RET NC
            0xD1 => { let v = self.pop(); self.reg.set_de(v); 12 } // POP DE
            0xD2 => { if !self.reg.flag(C) { self.reg.pc = self.fetchword(); 16 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 12 } } // JP NC,nn
            0xD4 => { if !self.reg.flag(C) { let a = self.fetchword(); self.push(self.reg.pc); self.reg.pc = a; 24 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 12 } } // CALL NC,nn
            0xD5 => { self.push(self.reg.de()); 16 } // PUSH DE
            0xD6 => { let v = self.fetchbyte(); self.alu_sub(v, false); 8 } // SUB n
            0xD7 => { self.push(self.reg.pc); self.reg.pc = 0x10; 16 } // RST 10
            0xD8 => { if self.reg.flag(C) { self.reg.pc = self.pop(); 20 } else { 8 } } // RET C
            0xD9 => { self.reg.pc = self.pop(); self.ime = true; 16 } // RETI
            0xDA => { if self.reg.flag(C) { self.reg.pc = self.fetchword(); 16 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 12 } } // JP C,nn
            0xDC => { if self.reg.flag(C) { let a = self.fetchword(); self.push(self.reg.pc); self.reg.pc = a; 24 } else { self.reg.pc = self.reg.pc.wrapping_add(2); 12 } } // CALL C,nn
            0xDE => { let v = self.fetchbyte(); self.alu_sub(v, true); 8 } // SBC A,n
            0xDF => { self.push(self.reg.pc); self.reg.pc = 0x18; 16 } // RST 18

            0xE0 => { let a = 0xFF00 | self.fetchbyte() as u16; self.mmu.wb(a, self.reg.a); 12 } // LDH (n),A
            0xE1 => { let v = self.pop(); self.reg.set_hl(v); 12 } // POP HL
            0xE2 => { self.mmu.wb(0xFF00 | self.reg.c as u16, self.reg.a); 8 } // LD (C),A
            0xE5 => { self.push(self.reg.hl()); 16 } // PUSH HL
            0xE6 => { let v = self.fetchbyte(); self.alu_and(v); 8 } // AND n
            0xE7 => { self.push(self.reg.pc); self.reg.pc = 0x20; 16 } // RST 20
            0xE8 => { // ADD SP,n
                let v = self.fetchbyte() as i8 as i16 as u16;
                let sp = self.reg.sp;
                self.reg.set_flag(Z, false);
                self.reg.set_flag(N, false);
                self.reg.set_flag(H, (sp & 0x0F) + (v & 0x0F) > 0x0F);
                self.reg.set_flag(C, (sp & 0xFF) + (v & 0xFF) > 0xFF);
                self.reg.sp = sp.wrapping_add(v);
                16
            }
            0xE9 => { self.reg.pc = self.reg.hl(); 4 } // JP HL
            0xEA => { let a = self.fetchword(); self.mmu.wb(a, self.reg.a); 16 } // LD (nn),A
            0xEE => { let v = self.fetchbyte(); self.alu_xor(v); 8 } // XOR n
            0xEF => { self.push(self.reg.pc); self.reg.pc = 0x28; 16 } // RST 28

            0xF0 => { let a = 0xFF00 | self.fetchbyte() as u16; self.reg.a = self.mmu.rb(a); 12 } // LDH A,(n)
            0xF1 => { let v = self.pop(); self.reg.set_af(v); 12 } // POP AF
            0xF2 => { self.reg.a = self.mmu.rb(0xFF00 | self.reg.c as u16); 8 } // LD A,(C)
            0xF3 => { self.setdi = 2; 4 } // DI
            0xF5 => { self.push(self.reg.af()); 16 } // PUSH AF
            0xF6 => { let v = self.fetchbyte(); self.alu_or(v); 8 } // OR n
            0xF7 => { self.push(self.reg.pc); self.reg.pc = 0x30; 16 } // RST 30
            0xF8 => { // LD HL,SP+n
                let v = self.fetchbyte() as i8 as i16 as u16;
                let sp = self.reg.sp;
                self.reg.set_flag(Z, false);
                self.reg.set_flag(N, false);
                self.reg.set_flag(H, (sp & 0x0F) + (v & 0x0F) > 0x0F);
                self.reg.set_flag(C, (sp & 0xFF) + (v & 0xFF) > 0xFF);
                self.reg.set_hl(sp.wrapping_add(v));
                12
            }
            0xF9 => { self.reg.sp = self.reg.hl(); 8 } // LD SP,HL
            0xFA => { let a = self.fetchword(); self.reg.a = self.mmu.rb(a); 16 } // LD A,(nn)
            0xFB => { self.setei = 2; 4 } // EI
            0xFE => { let v = self.fetchbyte(); self.alu_cp(v); 8 } // CP n
            0xFF => { self.push(self.reg.pc); self.reg.pc = 0x38; 16 } // RST 38

            _ => 4, // Undefined opcodes
        }
    }

    /// Execute CB-prefixed instruction
    #[inline]
    fn execute_cb(&mut self) -> u32 {
        let opcode = self.fetchbyte();
        let reg_idx = opcode & 0x07;
        let op_type = opcode >> 3;

        // Get value from register
        let (mut value, cycles) = match reg_idx {
            0 => (self.reg.b, 8),
            1 => (self.reg.c, 8),
            2 => (self.reg.d, 8),
            3 => (self.reg.e, 8),
            4 => (self.reg.h, 8),
            5 => (self.reg.l, 8),
            6 => (self.mmu.rb(self.reg.hl()), 16),
            7 => (self.reg.a, 8),
            _ => unreachable!(),
        };

        // Perform operation
        value = match op_type {
            0 => self.alu_rlc(value),
            1 => self.alu_rrc(value),
            2 => self.alu_rl(value),
            3 => self.alu_rr(value),
            4 => self.alu_sla(value),
            5 => self.alu_sra(value),
            6 => self.alu_swap(value),
            7 => self.alu_srl(value),
            8..=15 => { self.alu_bit(value, op_type - 8); return cycles; }
            16..=23 => value & !(1 << (op_type - 16)),
            24..=31 => value | (1 << (op_type - 24)),
            _ => unreachable!(),
        };

        // Store result
        match reg_idx {
            0 => self.reg.b = value,
            1 => self.reg.c = value,
            2 => self.reg.d = value,
            3 => self.reg.e = value,
            4 => self.reg.h = value,
            5 => self.reg.l = value,
            6 => self.mmu.wb(self.reg.hl(), value),
            7 => self.reg.a = value,
            _ => unreachable!(),
        };

        cycles
    }

    // =========================================================================
    // ALU operations - ALL INLINED for performance
    // =========================================================================

    #[inline(always)]
    fn alu_add(&mut self, value: u8, with_carry: bool) {
        let carry = if with_carry && self.reg.flag(C) { 1 } else { 0 };
        let result = self.reg.a as u16 + value as u16 + carry as u16;
        let half = (self.reg.a & 0x0F) + (value & 0x0F) + carry;
        self.reg.a = result as u8;
        self.reg.set_flag(Z, self.reg.a == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, half > 0x0F);
        self.reg.set_flag(C, result > 0xFF);
    }

    #[inline(always)]
    fn alu_sub(&mut self, value: u8, with_carry: bool) {
        let carry = if with_carry && self.reg.flag(C) { 1 } else { 0 };
        let result = self.reg.a as i16 - value as i16 - carry as i16;
        let half = (self.reg.a & 0x0F) as i16 - (value & 0x0F) as i16 - carry as i16;
        self.reg.a = result as u8;
        self.reg.set_flag(Z, self.reg.a == 0);
        self.reg.set_flag(N, true);
        self.reg.set_flag(H, half < 0);
        self.reg.set_flag(C, result < 0);
    }

    #[inline(always)]
    fn alu_and(&mut self, value: u8) {
        self.reg.a &= value;
        self.reg.set_flag(Z, self.reg.a == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, true);
        self.reg.set_flag(C, false);
    }

    #[inline(always)]
    fn alu_or(&mut self, value: u8) {
        self.reg.a |= value;
        self.reg.set_flag(Z, self.reg.a == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, false);
        self.reg.set_flag(C, false);
    }

    #[inline(always)]
    fn alu_xor(&mut self, value: u8) {
        self.reg.a ^= value;
        self.reg.set_flag(Z, self.reg.a == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, false);
        self.reg.set_flag(C, false);
    }

    #[inline(always)]
    fn alu_cp(&mut self, value: u8) {
        let result = self.reg.a as i16 - value as i16;
        let half = (self.reg.a & 0x0F) as i16 - (value & 0x0F) as i16;
        self.reg.set_flag(Z, result as u8 == 0);
        self.reg.set_flag(N, true);
        self.reg.set_flag(H, half < 0);
        self.reg.set_flag(C, result < 0);
    }

    #[inline(always)]
    fn alu_inc(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, (value & 0x0F) + 1 > 0x0F);
        result
    }

    #[inline(always)]
    fn alu_dec(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, true);
        self.reg.set_flag(H, (value & 0x0F) == 0);
        result
    }

    #[inline(always)]
    fn alu_add16(&mut self, value: u16) {
        let hl = self.reg.hl();
        let result = hl as u32 + value as u32;
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF);
        self.reg.set_flag(C, result > 0xFFFF);
        self.reg.set_hl(result as u16);
    }

    #[inline(always)]
    fn alu_daa(&mut self) {
        let mut a = self.reg.a as i16;
        if self.reg.flag(N) {
            if self.reg.flag(H) { a = (a - 6) & 0xFF; }
            if self.reg.flag(C) { a -= 0x60; }
        } else {
            if self.reg.flag(H) || (a & 0x0F) > 9 { a += 0x06; }
            if self.reg.flag(C) || a > 0x9F { a += 0x60; }
        }
        self.reg.a = a as u8;
        self.reg.set_flag(Z, self.reg.a == 0);
        self.reg.set_flag(H, false);
        if a >= 0x100 { self.reg.set_flag(C, true); }
    }

    #[inline(always)]
    fn alu_rlc(&mut self, value: u8) -> u8 {
        let carry = value >> 7;
        let result = (value << 1) | carry;
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, false);
        self.reg.set_flag(C, carry != 0);
        result
    }

    #[inline(always)]
    fn alu_rrc(&mut self, value: u8) -> u8 {
        let carry = value & 1;
        let result = (value >> 1) | (carry << 7);
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, false);
        self.reg.set_flag(C, carry != 0);
        result
    }

    #[inline(always)]
    fn alu_rl(&mut self, value: u8) -> u8 {
        let old_carry = if self.reg.flag(C) { 1 } else { 0 };
        let new_carry = value >> 7;
        let result = (value << 1) | old_carry;
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, false);
        self.reg.set_flag(C, new_carry != 0);
        result
    }

    #[inline(always)]
    fn alu_rr(&mut self, value: u8) -> u8 {
        let old_carry = if self.reg.flag(C) { 1 } else { 0 };
        let new_carry = value & 1;
        let result = (value >> 1) | (old_carry << 7);
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, false);
        self.reg.set_flag(C, new_carry != 0);
        result
    }

    #[inline(always)]
    fn alu_sla(&mut self, value: u8) -> u8 {
        let carry = value >> 7;
        let result = value << 1;
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, false);
        self.reg.set_flag(C, carry != 0);
        result
    }

    #[inline(always)]
    fn alu_sra(&mut self, value: u8) -> u8 {
        let carry = value & 1;
        let result = (value >> 1) | (value & 0x80);
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, false);
        self.reg.set_flag(C, carry != 0);
        result
    }

    #[inline(always)]
    fn alu_srl(&mut self, value: u8) -> u8 {
        let carry = value & 1;
        let result = value >> 1;
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, false);
        self.reg.set_flag(C, carry != 0);
        result
    }

    #[inline(always)]
    fn alu_swap(&mut self, value: u8) -> u8 {
        let result = (value >> 4) | (value << 4);
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, false);
        self.reg.set_flag(C, false);
        result
    }

    #[inline(always)]
    fn alu_bit(&mut self, value: u8, bit: u8) {
        let result = value & (1 << bit);
        self.reg.set_flag(Z, result == 0);
        self.reg.set_flag(N, false);
        self.reg.set_flag(H, true);
    }

    #[inline(always)]
    fn cpu_jr(&mut self) {
        let n = self.fetchbyte() as i8;
        self.reg.pc = ((self.reg.pc as i32) + (n as i32)) as u16;
    }
}
