//! Memory Management Unit
//!
//! Handles all memory access, DMA transfers, and coordinates peripheral components.

use alloc::boxed::Box;

use crate::cartridge::Cartridge;
use crate::gbmode::{GbMode, GbSpeed};
use crate::gpu::GPU;
use crate::keypad::Keypad;
use crate::serial::Serial;
use crate::sound::Sound;
use crate::timer::Timer;

const WRAM_SIZE: usize = 0x8000; // (32 KB for CGB, 8 KB for DMG)
const ZRAM_SIZE: usize = 0x7F; // (127 bytes)

#[derive(PartialEq, Clone, Copy)]
enum DMAType {
    NoDMA,
    GDMA,
    HDMA,
}

/// Memory Management Unit
pub struct MMU {
    wram: Box<[u8; WRAM_SIZE]>,
    zram: [u8; ZRAM_SIZE],
    hdma: [u8; 4],
    pub inte: u8,
    pub intf: u8,
    pub serial: Serial,
    pub timer: Timer,
    pub keypad: Keypad,
    pub gpu: GPU,
    pub sound: Option<Sound>,
    hdma_status: DMAType,
    hdma_src: u16,
    hdma_dst: u16,
    hdma_len: u8,
    wrambank: usize,
    pub mbc: Box<dyn Cartridge>,
    pub gbmode: GbMode,
    gbspeed: GbSpeed,
    speed_switch_req: bool,
    undocumented_cgb_regs: [u8; 3], // 0xFF72, 0xFF73, 0xFF75
}

fn fill_random(slice: &mut [u8], start: u32) {
    // Simple LCG to generate (non-cryptographic) random values
    // Each distinct invocation should use a different start value
    const A: u32 = 1103515245;
    const C: u32 = 12345;

    let mut x = start;
    for v in slice.iter_mut() {
        x = x.wrapping_mul(A).wrapping_add(C);
        *v = ((x >> 23) & 0xFF) as u8;
    }
}

impl MMU {
    /// Create new MMU in classic Game Boy mode
    pub fn new(cart: Box<dyn Cartridge>) -> Result<MMU, crate::EmulatorError> {
        let serial = Serial::new();
        let mut mmu = MMU {
            wram: Box::new([0u8; WRAM_SIZE]),
            zram: [0; ZRAM_SIZE],
            hdma: [0; 4],
            wrambank: 1,
            inte: 0,
            intf: 0,
            serial,
            timer: Timer::new(),
            keypad: Keypad::new(),
            gpu: GPU::new(),
            sound: None,
            mbc: cart,
            gbmode: GbMode::Classic,
            gbspeed: GbSpeed::Single,
            speed_switch_req: false,
            hdma_src: 0,
            hdma_dst: 0,
            hdma_status: DMAType::NoDMA,
            hdma_len: 0xFF,
            undocumented_cgb_regs: [0; 3],
        };
        
        // Check if ROM requires CGB mode
        if mmu.rb(0x0143) == 0xC0 {
            return Err(crate::EmulatorError::RequiresColorMode);
        }
        mmu.set_initial();
        Ok(mmu)
    }

    /// Create new MMU in Game Boy Color mode
    pub fn new_cgb(cart: Box<dyn Cartridge>) -> Result<MMU, crate::EmulatorError> {
        let serial = Serial::new();
        let mut mmu = MMU {
            wram: Box::new([0; WRAM_SIZE]),
            zram: [0; ZRAM_SIZE],
            wrambank: 1,
            hdma: [0; 4],
            inte: 0,
            intf: 0,
            serial,
            timer: Timer::new(),
            keypad: Keypad::new(),
            gpu: GPU::new_cgb(),
            sound: None,
            mbc: cart,
            gbmode: GbMode::Color,
            gbspeed: GbSpeed::Single,
            speed_switch_req: false,
            hdma_src: 0,
            hdma_dst: 0,
            hdma_status: DMAType::NoDMA,
            hdma_len: 0xFF,
            undocumented_cgb_regs: [0; 3],
        };
        fill_random(&mut mmu.wram[..], 42);
        mmu.determine_mode();
        mmu.set_initial();
        Ok(mmu)
    }

    fn set_initial(&mut self) {
        self.wb(0xFF05, 0);
        self.wb(0xFF06, 0);
        self.wb(0xFF07, 0);
        self.wb(0xFF10, 0x80);
        self.wb(0xFF11, 0xBF);
        self.wb(0xFF12, 0xF3);
        self.wb(0xFF14, 0xBF);
        self.wb(0xFF16, 0x3F);
        self.wb(0xFF17, 0);
        self.wb(0xFF19, 0xBF);
        self.wb(0xFF1A, 0x7F);
        self.wb(0xFF1B, 0xFF);
        self.wb(0xFF1C, 0x9F);
        self.wb(0xFF1E, 0xFF);
        self.wb(0xFF20, 0xFF);
        self.wb(0xFF21, 0);
        self.wb(0xFF22, 0);
        self.wb(0xFF23, 0xBF);
        self.wb(0xFF24, 0x77);
        self.wb(0xFF25, 0xF3);
        self.wb(0xFF26, 0xF1);
        self.wb(0xFF40, 0x91);
        self.wb(0xFF42, 0);
        self.wb(0xFF43, 0);
        self.wb(0xFF45, 0);
        self.wb(0xFF47, 0xFC);
        self.wb(0xFF48, 0xFF);
        self.wb(0xFF49, 0xFF);
        self.wb(0xFF4A, 0);
        self.wb(0xFF4B, 0);
    }

    fn determine_mode(&mut self) {
        let mode = match self.rb(0x0143) & 0x80 {
            0x80 => GbMode::Color,
            _ => GbMode::ColorAsClassic,
        };
        self.gbmode = mode;
        self.gpu.gbmode = mode;
    }

    /// Run peripheral cycles and return GPU ticks
    pub fn do_cycle(&mut self, ticks: u32) -> u32 {
        let cpudivider = self.gbspeed as u32;
        let vramticks = self.perform_vramdma();
        let gputicks = ticks / cpudivider + vramticks;
        let cputicks = ticks + vramticks * cpudivider;

        self.timer.do_cycle(cputicks);
        self.intf |= self.timer.interrupt;
        self.timer.interrupt = 0;

        self.intf |= self.keypad.interrupt;
        self.keypad.interrupt = 0;

        self.gpu.do_cycle(gputicks);
        self.intf |= self.gpu.interrupt;
        self.gpu.interrupt = 0;

        if let Some(ref mut sound) = self.sound {
            sound.do_cycle(gputicks);
        }

        self.intf |= self.serial.interrupt;
        self.serial.interrupt = 0;

        gputicks
    }

    /// Read byte from memory
    #[inline]
    pub fn rb(&mut self, address: u16) -> u8 {
        match address {
            0x0000..=0x7FFF => self.mbc.read_rom(address),
            0x8000..=0x9FFF => self.gpu.rb(address),
            0xA000..=0xBFFF => self.mbc.read_ram(address),
            0xC000..=0xCFFF | 0xE000..=0xEFFF => self.wram[address as usize & 0x0FFF],
            0xD000..=0xDFFF | 0xF000..=0xFDFF => {
                self.wram[(self.wrambank * 0x1000) | address as usize & 0x0FFF]
            }
            0xFE00..=0xFE9F => self.gpu.rb(address),
            0xFF00 => self.keypad.rb(),
            0xFF01..=0xFF02 => self.serial.rb(address),
            0xFF04..=0xFF07 => self.timer.rb(address),
            0xFF0F => self.intf | 0b11100000,
            0xFF10..=0xFF3F => self.sound.as_mut().map_or(0xFF, |s| s.rb(address)),
            0xFF4D | 0xFF4F | 0xFF51..=0xFF55 | 0xFF6C | 0xFF70 if self.gbmode != GbMode::Color => {
                0xFF
            }
            0xFF72..=0xFF73 | 0xFF75..=0xFF77 if self.gbmode == GbMode::Classic => 0xFF,
            0xFF4D => {
                0b01111110
                    | (if self.gbspeed == GbSpeed::Double {
                        0x80
                    } else {
                        0
                    })
                    | (if self.speed_switch_req { 1 } else { 0 })
            }
            0xFF40..=0xFF4F => self.gpu.rb(address),
            0xFF51..=0xFF55 => self.hdma_read(address),
            0xFF68..=0xFF6B => self.gpu.rb(address),
            0xFF70 => self.wrambank as u8,
            0xFF72..=0xFF73 => self.undocumented_cgb_regs[address as usize - 0xFF72],
            0xFF75 => self.undocumented_cgb_regs[2] | 0b10001111,
            0xFF76..=0xFF77 => 0x00, // CGB PCM registers - not implemented
            0xFF80..=0xFFFE => self.zram[address as usize & 0x007F],
            0xFFFF => self.inte,
            _ => 0xFF,
        }
    }

    /// Read word from memory (little endian)
    #[inline]
    pub fn rw(&mut self, address: u16) -> u16 {
        (self.rb(address) as u16) | ((self.rb(address.wrapping_add(1)) as u16) << 8)
    }

    /// Write byte to memory
    #[inline]
    pub fn wb(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x7FFF => self.mbc.write_rom(address, value),
            0x8000..=0x9FFF => self.gpu.wb(address, value),
            0xA000..=0xBFFF => self.mbc.write_ram(address, value),
            0xC000..=0xCFFF | 0xE000..=0xEFFF => self.wram[address as usize & 0x0FFF] = value,
            0xD000..=0xDFFF | 0xF000..=0xFDFF => {
                self.wram[(self.wrambank * 0x1000) | (address as usize & 0x0FFF)] = value
            }
            0xFE00..=0xFE9F => self.gpu.wb(address, value),
            0xFF00 => self.keypad.wb(value),
            0xFF01..=0xFF02 => self.serial.wb(address, value),
            0xFF04..=0xFF07 => self.timer.wb(address, value),
            0xFF10..=0xFF3F => {
                if let Some(ref mut s) = self.sound {
                    s.wb(address, value)
                }
            }
            0xFF46 => self.oamdma(value),
            0xFF4D | 0xFF4F | 0xFF51..=0xFF55 | 0xFF6C | 0xFF70 | 0xFF76..=0xFF77
                if self.gbmode != GbMode::Color => {}
            0xFF72..=0xFF73 | 0xFF75..=0xFF77 if self.gbmode == GbMode::Classic => {}
            0xFF4D => {
                if value & 0x1 == 0x1 {
                    self.speed_switch_req = true;
                }
            }
            0xFF40..=0xFF4F => self.gpu.wb(address, value),
            0xFF51..=0xFF55 => self.hdma_write(address, value),
            0xFF68..=0xFF6B => self.gpu.wb(address, value),
            0xFF0F => self.intf = value,
            0xFF70 => {
                self.wrambank = match value & 0x7 {
                    0 => 1,
                    n => n as usize,
                };
            }
            0xFF72..=0xFF73 => self.undocumented_cgb_regs[address as usize - 0xFF72] = value,
            0xFF75 => self.undocumented_cgb_regs[2] = value,
            0xFF80..=0xFFFE => self.zram[address as usize & 0x007F] = value,
            0xFFFF => self.inte = value,
            _ => {}
        };
    }

    /// Write word to memory (little endian)
    #[inline]
    pub fn ww(&mut self, address: u16, value: u16) {
        self.wb(address, (value & 0xFF) as u8);
        self.wb(address.wrapping_add(1), (value >> 8) as u8);
    }

    /// Handle speed switch (CGB)
    pub fn switch_speed(&mut self) {
        if self.speed_switch_req {
            if self.gbspeed == GbSpeed::Double {
                self.gbspeed = GbSpeed::Single;
            } else {
                self.gbspeed = GbSpeed::Double;
            }
        }
        self.speed_switch_req = false;
    }

    fn oamdma(&mut self, value: u8) {
        let base = (value as u16) << 8;
        for i in 0..0xA0 {
            let b = self.rb(base + i);
            self.wb(0xFE00 + i, b);
        }
    }

    fn hdma_read(&self, a: u16) -> u8 {
        match a {
            0xFF51..=0xFF54 => self.hdma[(a - 0xFF51) as usize],
            0xFF55 => {
                self.hdma_len
                    | if self.hdma_status == DMAType::NoDMA {
                        0x80
                    } else {
                        0
                    }
            }
            _ => panic!("Invalid HDMA read address: {:04X}", a),
        }
    }

    fn hdma_write(&mut self, a: u16, v: u8) {
        match a {
            0xFF51 => self.hdma[0] = v,
            0xFF52 => self.hdma[1] = v & 0xF0,
            0xFF53 => self.hdma[2] = v & 0x1F,
            0xFF54 => self.hdma[3] = v & 0xF0,
            0xFF55 => {
                if self.hdma_status == DMAType::HDMA {
                    if v & 0x80 == 0 {
                        self.hdma_status = DMAType::NoDMA;
                    };
                    return;
                }
                let src = ((self.hdma[0] as u16) << 8) | (self.hdma[1] as u16);
                let dst = ((self.hdma[2] as u16) << 8) | (self.hdma[3] as u16) | 0x8000;
                if !(src <= 0x7FF0 || (src >= 0xA000 && src <= 0xDFF0)) {
                    // Invalid source address - silently ignore in release
                    #[cfg(debug_assertions)]
                    panic!("HDMA transfer with illegal start address {:04X}", src);
                    #[cfg(not(debug_assertions))]
                    return;
                }

                self.hdma_src = src;
                self.hdma_dst = dst;
                self.hdma_len = v & 0x7F;

                self.hdma_status = if v & 0x80 == 0x80 {
                    DMAType::HDMA
                } else {
                    DMAType::GDMA
                };
            }
            _ => panic!("Invalid HDMA write address: {:04X}", a),
        };
    }

    fn perform_vramdma(&mut self) -> u32 {
        match self.hdma_status {
            DMAType::NoDMA => 0,
            DMAType::GDMA => self.perform_gdma(),
            DMAType::HDMA => self.perform_hdma(),
        }
    }

    fn perform_hdma(&mut self) -> u32 {
        if !self.gpu.may_hdma() {
            return 0;
        }

        self.perform_vramdma_row();
        if self.hdma_len == 0x7F {
            self.hdma_status = DMAType::NoDMA;
        }

        8
    }

    fn perform_gdma(&mut self) -> u32 {
        let len = self.hdma_len as u32 + 1;
        for _ in 0..len {
            self.perform_vramdma_row();
        }

        self.hdma_status = DMAType::NoDMA;
        len * 8
    }

    fn perform_vramdma_row(&mut self) {
        let mmu_src = self.hdma_src;
        for j in 0..0x10 {
            let b: u8 = self.rb(mmu_src + j);
            self.gpu.wb(self.hdma_dst + j, b);
        }
        self.hdma_src += 0x10;
        self.hdma_dst += 0x10;

        if self.hdma_len == 0 {
            self.hdma_len = 0x7F;
        } else {
            self.hdma_len -= 1;
        }
    }

    /// Serialize MMU state
    pub fn serialize(&self, output: &mut alloc::vec::Vec<u8>) {
        // WRAM
        output.extend_from_slice(&*self.wram);
        // ZRAM
        output.extend_from_slice(&self.zram);
        // HDMA registers
        output.extend_from_slice(&self.hdma);
        // Interrupt registers
        output.push(self.inte);
        output.push(self.intf);
        // WRAM bank
        output.push(self.wrambank as u8);
        // Mode and speed
        output.push(self.gbmode as u8);
        output.push(self.gbspeed as u8);
        output.push(if self.speed_switch_req { 1 } else { 0 });
        // HDMA state
        output.push(self.hdma_status as u8);
        output.extend_from_slice(&self.hdma_src.to_le_bytes());
        output.extend_from_slice(&self.hdma_dst.to_le_bytes());
        output.push(self.hdma_len);
        // Undocumented regs
        output.extend_from_slice(&self.undocumented_cgb_regs);
        
        // Serialize components
        self.timer.serialize(output);
        self.keypad.serialize(output);
        self.gpu.serialize(output);
        self.serial.serialize(output);
        self.mbc.serialize(output);
    }

    /// Deserialize MMU state
    pub fn deserialize(&mut self, data: &[u8]) -> Result<usize, ()> {
        let mut offset = 0;
        
        // WRAM
        if data.len() < offset + WRAM_SIZE {
            return Err(());
        }
        (*self.wram).copy_from_slice(&data[offset..offset + WRAM_SIZE]);
        offset += WRAM_SIZE;
        
        // ZRAM
        if data.len() < offset + ZRAM_SIZE {
            return Err(());
        }
        self.zram.copy_from_slice(&data[offset..offset + ZRAM_SIZE]);
        offset += ZRAM_SIZE;
        
        // HDMA
        if data.len() < offset + 4 {
            return Err(());
        }
        self.hdma.copy_from_slice(&data[offset..offset + 4]);
        offset += 4;
        
        // Interrupt registers
        if data.len() < offset + 2 {
            return Err(());
        }
        self.inte = data[offset];
        self.intf = data[offset + 1];
        offset += 2;
        
        // WRAM bank
        if data.len() < offset + 1 {
            return Err(());
        }
        self.wrambank = data[offset] as usize;
        offset += 1;
        
        // Mode and speed
        if data.len() < offset + 4 {
            return Err(());
        }
        self.gbmode = match data[offset] {
            0 => GbMode::Classic,
            1 => GbMode::Color,
            _ => GbMode::ColorAsClassic,
        };
        self.gbspeed = if data[offset + 1] == 2 {
            GbSpeed::Double
        } else {
            GbSpeed::Single
        };
        self.speed_switch_req = data[offset + 2] != 0;
        self.hdma_status = match data[offset + 3] {
            1 => DMAType::GDMA,
            2 => DMAType::HDMA,
            _ => DMAType::NoDMA,
        };
        offset += 4;
        
        // HDMA addresses
        if data.len() < offset + 5 {
            return Err(());
        }
        self.hdma_src = u16::from_le_bytes([data[offset], data[offset + 1]]);
        self.hdma_dst = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        self.hdma_len = data[offset + 4];
        offset += 5;
        
        // Undocumented regs
        if data.len() < offset + 3 {
            return Err(());
        }
        self.undocumented_cgb_regs.copy_from_slice(&data[offset..offset + 3]);
        offset += 3;
        
        // Deserialize components
        offset += self.timer.deserialize(&data[offset..])?;
        offset += self.keypad.deserialize(&data[offset..])?;
        offset += self.gpu.deserialize(&data[offset..])?;
        offset += self.serial.deserialize(&data[offset..])?;
        offset += self.mbc.deserialize(&data[offset..]).map_err(|_| ())?;
        
        Ok(offset)
    }
}
