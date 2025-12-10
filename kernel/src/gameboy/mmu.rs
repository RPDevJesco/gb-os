//! GameBoy Memory Management Unit
//!
//! Maps the 64KB address space to various hardware components:
//! - 0x0000-0x7FFF: ROM (via MBC)
//! - 0x8000-0x9FFF: VRAM
//! - 0xA000-0xBFFF: External RAM (via MBC)
//! - 0xC000-0xDFFF: Work RAM
//! - 0xE000-0xFDFF: Echo RAM
//! - 0xFE00-0xFE9F: OAM
//! - 0xFF00-0xFF7F: I/O Registers
//! - 0xFF80-0xFFFE: High RAM
//! - 0xFFFF: Interrupt Enable

extern crate alloc;

use alloc::boxed::Box;
use super::gbmode::{GbMode, GbSpeed};
use super::gpu::GPU;
use super::keypad::Keypad;
use super::mbc;
use super::serial::Serial;
use super::timer::Timer;
use super::StrResult;

const WRAM_SIZE: usize = 0x8000;
const ZRAM_SIZE: usize = 0x7F;

/// Memory Management Unit
pub struct MMU {
    // Work RAM (8 banks for CGB)
    wram: [u8; WRAM_SIZE],
    // Zero-page RAM
    zram: [u8; ZRAM_SIZE],
    // WRAM bank select (CGB)
    wrambank: usize,
    // HDMA registers (CGB)
    hdma: [u8; 4],
    // Interrupt enable
    pub inte: u8,
    // Interrupt flags
    pub intf: u8,
    // Serial port
    pub serial: Serial,
    // Timer
    pub timer: Timer,
    // Keypad
    pub keypad: Keypad,
    // GPU
    pub gpu: GPU,
    // Memory bank controller
    pub mbc: Box<dyn mbc::MBC + 'static>,
    // Hardware mode
    pub gbmode: GbMode,
    // CPU speed (CGB)
    gbspeed: GbSpeed,
    speed_switch_req: bool,
    // HDMA state
    hdma_src: u16,
    hdma_dst: u16,
    hdma_status: DMAType,
    hdma_len: u8,
    // Undocumented CGB registers
    undocumented_cgb_regs: [u8; 3],
}

#[derive(PartialEq)]
enum DMAType {
    NoDMA,
    GDMA,
    HDMA,
}

/// Simple LCG for initializing RAM with "random" values
fn fill_random(slice: &mut [u8], start: u32) {
    const A: u32 = 1103515245;
    const C: u32 = 12345;
    let mut x = start;
    for v in slice.iter_mut() {
        x = x.wrapping_mul(A).wrapping_add(C);
        *v = ((x >> 23) & 0xFF) as u8;
    }
}

impl MMU {
    /// Create MMU for classic GameBoy
    pub fn new(
        cart: Box<dyn mbc::MBC + 'static>,
    ) -> StrResult<MMU> {
        let mut res = MMU {
            wram: [0; WRAM_SIZE],
            zram: [0; ZRAM_SIZE],
            wrambank: 1,
            hdma: [0; 4],
            inte: 0,
            intf: 0,
            serial: Serial::new(),
            timer: Timer::new(),
            keypad: Keypad::new(),
            gpu: GPU::new(),
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
        fill_random(&mut res.wram, 42);
        
        // Check if ROM requires CGB
        if res.rb(0x0143) == 0xC0 {
            return Err("This game does not work in Classic mode");
        }
        res.set_initial();
        Ok(res)
    }

    /// Create MMU for GameBoy Color
    pub fn new_cgb(cart: Box<dyn mbc::MBC + 'static>) -> StrResult<MMU> {
        let mut res = MMU {
            wram: [0; WRAM_SIZE],
            zram: [0; ZRAM_SIZE],
            wrambank: 1,
            hdma: [0; 4],
            inte: 0,
            intf: 0,
            serial: Serial::new(),
            timer: Timer::new(),
            keypad: Keypad::new(),
            gpu: GPU::new_cgb(),
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
        fill_random(&mut res.wram, 42);
        res.determine_mode();
        res.set_initial();
        Ok(res)
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

    /// Run one cycle of connected hardware
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

        self.serial.do_cycle(gputicks);
        self.intf |= self.serial.interrupt;
        self.serial.interrupt = 0;

        gputicks
    }

    /// Read byte from memory
    pub fn rb(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.mbc.readrom(addr),
            0x8000..=0x9FFF => self.gpu.rb(addr),
            0xA000..=0xBFFF => self.mbc.readram(addr),
            0xC000..=0xCFFF | 0xE000..=0xEFFF => self.wram[addr as usize & 0x0FFF],
            0xD000..=0xDFFF | 0xF000..=0xFDFF => {
                self.wram[(self.wrambank * 0x1000) | (addr as usize & 0x0FFF)]
            }
            0xFE00..=0xFE9F => self.gpu.rb(addr),
            0xFF00 => self.keypad.rb(),
            0xFF01..=0xFF02 => self.serial.rb(addr),
            0xFF04..=0xFF07 => self.timer.rb(addr),
            0xFF0F => self.intf | 0b11100000,
            0xFF10..=0xFF3F => 0xFF, // Sound registers (stubbed)
            0xFF4D if self.gbmode != GbMode::Color => 0xFF,
            0xFF4F if self.gbmode != GbMode::Color => 0xFF,
            0xFF51..=0xFF55 if self.gbmode != GbMode::Color => 0xFF,
            0xFF6C if self.gbmode != GbMode::Color => 0xFF,
            0xFF70 if self.gbmode != GbMode::Color => 0xFF,
            0xFF72..=0xFF73 if self.gbmode == GbMode::Classic => 0xFF,
            0xFF75..=0xFF77 if self.gbmode == GbMode::Classic => 0xFF,
            0xFF4D => {
                0b01111110
                    | (if self.gbspeed == GbSpeed::Double { 0x80 } else { 0 })
                    | (if self.speed_switch_req { 1 } else { 0 })
            }
            0xFF40..=0xFF4F => self.gpu.rb(addr),
            0xFF51..=0xFF55 => self.hdma_read(addr),
            0xFF68..=0xFF6B => self.gpu.rb(addr),
            0xFF70 => self.wrambank as u8,
            0xFF72..=0xFF73 => self.undocumented_cgb_regs[addr as usize - 0xFF72],
            0xFF75 => self.undocumented_cgb_regs[2] | 0b10001111,
            0xFF76..=0xFF77 => 0x00,
            0xFF80..=0xFFFE => self.zram[addr as usize & 0x007F],
            0xFFFF => self.inte,
            _ => 0xFF,
        }
    }

    /// Read word from memory
    pub fn rw(&mut self, addr: u16) -> u16 {
        (self.rb(addr) as u16) | ((self.rb(addr.wrapping_add(1)) as u16) << 8)
    }

    /// Write byte to memory
    pub fn wb(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x7FFF => self.mbc.writerom(addr, value),
            0x8000..=0x9FFF => self.gpu.wb(addr, value),
            0xA000..=0xBFFF => self.mbc.writeram(addr, value),
            0xC000..=0xCFFF | 0xE000..=0xEFFF => self.wram[addr as usize & 0x0FFF] = value,
            0xD000..=0xDFFF | 0xF000..=0xFDFF => {
                self.wram[(self.wrambank * 0x1000) | (addr as usize & 0x0FFF)] = value
            }
            0xFE00..=0xFE9F => self.gpu.wb(addr, value),
            0xFF00 => self.keypad.wb(value),
            0xFF01..=0xFF02 => self.serial.wb(addr, value),
            0xFF04..=0xFF07 => self.timer.wb(addr, value),
            0xFF0F => self.intf = value,
            0xFF10..=0xFF3F => {} // Sound registers (stubbed)
            0xFF46 => self.oam_dma(value),
            0xFF4D if self.gbmode != GbMode::Color => {}
            0xFF4F if self.gbmode != GbMode::Color => {}
            0xFF51..=0xFF55 if self.gbmode != GbMode::Color => {}
            0xFF6C if self.gbmode != GbMode::Color => {}
            0xFF70 if self.gbmode != GbMode::Color => {}
            0xFF4D => self.speed_switch_req = value & 1 != 0,
            0xFF40..=0xFF4F => self.gpu.wb(addr, value),
            0xFF51..=0xFF55 => self.hdma_write(addr, value),
            0xFF68..=0xFF6B => self.gpu.wb(addr, value),
            0xFF70 => {
                self.wrambank = match value & 0x7 {
                    0 => 1,
                    n => n as usize,
                }
            }
            0xFF72..=0xFF73 => self.undocumented_cgb_regs[addr as usize - 0xFF72] = value,
            0xFF75 => self.undocumented_cgb_regs[2] = value,
            0xFF80..=0xFFFE => self.zram[addr as usize & 0x007F] = value,
            0xFFFF => self.inte = value,
            _ => {}
        }
    }

    /// Write word to memory
    pub fn ww(&mut self, addr: u16, value: u16) {
        self.wb(addr, value as u8);
        self.wb(addr.wrapping_add(1), (value >> 8) as u8);
    }

    /// OAM DMA transfer
    fn oam_dma(&mut self, value: u8) {
        let base = (value as u16) << 8;
        for i in 0..0xA0 {
            let b = self.rb(base + i);
            self.wb(0xFE00 + i, b);
        }
    }

    /// HDMA read (CGB)
    fn hdma_read(&self, addr: u16) -> u8 {
        match addr {
            0xFF51 => (self.hdma_src >> 8) as u8,
            0xFF52 => (self.hdma_src & 0xF0) as u8,
            0xFF53 => ((self.hdma_dst >> 8) & 0x1F) as u8,
            0xFF54 => (self.hdma_dst & 0xF0) as u8,
            0xFF55 => {
                if self.hdma_status == DMAType::NoDMA {
                    0xFF
                } else {
                    self.hdma_len & 0x7F
                }
            }
            _ => 0xFF,
        }
    }

    /// HDMA write (CGB)
    fn hdma_write(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF51 => self.hdma_src = (self.hdma_src & 0x00FF) | ((value as u16) << 8),
            0xFF52 => self.hdma_src = (self.hdma_src & 0xFF00) | ((value & 0xF0) as u16),
            0xFF53 => {
                self.hdma_dst = (self.hdma_dst & 0x00FF) | (((value & 0x1F) as u16) << 8) | 0x8000
            }
            0xFF54 => self.hdma_dst = (self.hdma_dst & 0xFF00) | ((value & 0xF0) as u16),
            0xFF55 => {
                if self.hdma_status == DMAType::HDMA && value & 0x80 == 0 {
                    self.hdma_status = DMAType::NoDMA;
                } else {
                    self.hdma_len = value & 0x7F;
                    if value & 0x80 != 0 {
                        self.hdma_status = DMAType::HDMA;
                    } else {
                        self.hdma_status = DMAType::GDMA;
                    }
                }
            }
            _ => {}
        }
    }

    /// Perform VRAM DMA if active
    fn perform_vramdma(&mut self) -> u32 {
        match self.hdma_status {
            DMAType::NoDMA => 0,
            DMAType::GDMA => {
                let len = ((self.hdma_len as u32) + 1) * 16;
                for _ in 0..len {
                    let b = self.rb(self.hdma_src);
                    self.gpu.wb(self.hdma_dst, b);
                    self.hdma_src = self.hdma_src.wrapping_add(1);
                    self.hdma_dst = self.hdma_dst.wrapping_add(1);
                }
                self.hdma_status = DMAType::NoDMA;
                self.hdma_len = 0xFF;
                len
            }
            DMAType::HDMA => {
                // Only transfer during H-blank
                if self.gpu.rb(0xFF41) & 0x03 != 0 {
                    return 0;
                }
                for _ in 0..16 {
                    let b = self.rb(self.hdma_src);
                    self.gpu.wb(self.hdma_dst, b);
                    self.hdma_src = self.hdma_src.wrapping_add(1);
                    self.hdma_dst = self.hdma_dst.wrapping_add(1);
                }
                if self.hdma_len == 0 {
                    self.hdma_status = DMAType::NoDMA;
                    self.hdma_len = 0xFF;
                } else {
                    self.hdma_len -= 1;
                }
                16
            }
        }
    }

    /// Handle speed switch (CGB)
    pub fn switch_speed(&mut self) {
        if self.speed_switch_req {
            self.gbspeed = match self.gbspeed {
                GbSpeed::Single => GbSpeed::Double,
                GbSpeed::Double => GbSpeed::Single,
            };
            self.speed_switch_req = false;
        }
    }
}
