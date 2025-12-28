//! GameBoy GPU (PPU) Emulation
//!
//! Emulates the GameBoy's Picture Processing Unit which renders
//! 160x144 pixel display at ~60fps.

extern crate alloc;

use alloc::boxed::Box;
use super::gbmode::GbMode;

/// Screen width in pixels
pub const SCREEN_W: usize = 160;
/// Screen height in pixels
pub const SCREEN_H: usize = 144;

/// VRAM size (8KB for DMG, 16KB for CGB)
const VRAM_SIZE: usize = 0x4000;
/// OAM (sprite attribute memory) size
const OAM_SIZE: usize = 0xA0;
/// Framebuffer size (RGB, 3 bytes per pixel)
const DATA_SIZE: usize = SCREEN_W * SCREEN_H * 3;

/// Sprite priority type
#[derive(Clone, Copy, PartialEq)]
enum PrioType {
    Normal,
    Priority,
    Color0,
}

/// GPU state
pub struct GPU {
    // Video RAM (2 banks for CGB) - BOXED to avoid 16KB on stack
    vram: Box<[u8; VRAM_SIZE]>,
    vram_bank: usize,
    // Object Attribute Memory
    oam: [u8; OAM_SIZE],
    // LCD control register (0xFF40)
    lcdc: u8,
    // LCD status register (0xFF41)
    stat: u8,
    // Scroll registers
    pub scy: u8,
    pub scx: u8,
    // Current scanline (0xFF44)
    pub line: u8,
    // LY compare (0xFF45)
    lyc: u8,
    // Window position
    pub wy: u8,
    pub winx: u8,
    // Palettes (DMG)
    palbr: u8,
    pal0r: u8,
    pal1r: u8,
    palb: [u8; 4],
    pal0: [u8; 4],
    pal1: [u8; 4],
    // CGB palettes
    cbgpal_ind: u8,
    cbgpal_inc: bool,
    cbgpal: [[[u8; 3]; 4]; 8],
    csprit_ind: u8,
    csprit_inc: bool,
    csprit: [[[u8; 3]; 4]; 8],
    // LCDC bit extracts for fast access
    lcd_on: bool,
    win_tilemap: u16,
    bg_tilemap: u16,
    tilebase: u16,
    sprite_size: u32,
    sprite_on: bool,
    win_on: bool,
    lcdc0: bool,
    // Mode timing
    modeclock: u32,
    mode: u8,
    // Window internal counter
    wy_trigger: bool,
    wy_pos: i32,
    // Output buffer (RGB, 3 bytes per pixel) - BOXED to avoid 69KB on stack
    pub data: Box<[u8; DATA_SIZE]>,
    // Per-scanline background priority
    bgprio: [PrioType; SCREEN_W],
    // Frame update flag
    pub updated: bool,
    // Hardware mode
    pub gbmode: GbMode,
    // Interrupt request
    pub interrupt: u8,
    // First frame after LCD on
    first_frame: bool,
}

impl GPU {
    pub fn new() -> GPU {
        GPU::new_internal(GbMode::Classic)
    }

    pub fn new_cgb() -> GPU {
        GPU::new_internal(GbMode::Color)
    }

    fn new_internal(mode: GbMode) -> GPU {
        // Allocate large arrays on heap to avoid stack overflow
        let vram = Box::new([0u8; VRAM_SIZE]);
        let data = Box::new([255u8; DATA_SIZE]);

        GPU {
            vram,
            vram_bank: 0,
            oam: [0; OAM_SIZE],
            lcdc: 0,
            stat: 0,
            scy: 0,
            scx: 0,
            line: 0,
            lyc: 0,
            wy: 0,
            winx: 0,
            palbr: 0,
            pal0r: 0,
            pal1r: 0,
            palb: [255, 192, 96, 0],
            pal0: [255, 192, 96, 0],
            pal1: [255, 192, 96, 0],
            cbgpal_ind: 0,
            cbgpal_inc: false,
            cbgpal: [[[255; 3]; 4]; 8],
            csprit_ind: 0,
            csprit_inc: false,
            csprit: [[[255; 3]; 4]; 8],
            lcd_on: false,
            win_tilemap: 0x9800,
            bg_tilemap: 0x9800,
            tilebase: 0x8800,
            sprite_size: 8,
            sprite_on: false,
            win_on: false,
            lcdc0: false,
            modeclock: 0,
            mode: 0,
            wy_trigger: false,
            wy_pos: -1,
            data,
            bgprio: [PrioType::Normal; SCREEN_W],
            updated: false,
            gbmode: mode,
            interrupt: 0,
            first_frame: false,
        }
    }

    /// Read GPU register
    pub fn rb(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0x9FFF => {
                if self.lcd_on && self.mode == 3 {
                    0xFF
                } else {
                    self.vram[self.vram_bank * 0x2000 + (addr as usize & 0x1FFF)]
                }
            }
            0xFE00..=0xFE9F => {
                if self.lcd_on && (self.mode == 2 || self.mode == 3) {
                    0xFF
                } else {
                    self.oam[addr as usize - 0xFE00]
                }
            }
            0xFF40 => self.lcdc,
            0xFF41 => self.stat | 0x80,
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.line,
            0xFF45 => self.lyc,
            0xFF47 => self.palbr,
            0xFF48 => self.pal0r,
            0xFF49 => self.pal1r,
            0xFF4A => self.wy,
            0xFF4B => self.winx,
            0xFF4F => self.vram_bank as u8 | 0xFE,
            0xFF68 => self.cbgpal_ind | if self.cbgpal_inc { 0x80 } else { 0 },
            0xFF69 => {
                let palnum = (self.cbgpal_ind >> 3) as usize;
                let colnum = ((self.cbgpal_ind >> 1) & 0x3) as usize;
                if self.cbgpal_ind & 1 == 0 {
                    let r = self.cbgpal[palnum][colnum][0] >> 3;
                    let glow = self.cbgpal[palnum][colnum][1] >> 3;
                    r | ((glow & 0x7) << 5)
                } else {
                    let ghigh = self.cbgpal[palnum][colnum][1] >> 6;
                    let b = self.cbgpal[palnum][colnum][2] >> 3;
                    ghigh | (b << 2)
                }
            }
            0xFF6A => self.csprit_ind | if self.csprit_inc { 0x80 } else { 0 },
            0xFF6B => {
                let palnum = (self.csprit_ind >> 3) as usize;
                let colnum = ((self.csprit_ind >> 1) & 0x3) as usize;
                if self.csprit_ind & 1 == 0 {
                    let r = self.csprit[palnum][colnum][0] >> 3;
                    let glow = self.csprit[palnum][colnum][1] >> 3;
                    r | ((glow & 0x7) << 5)
                } else {
                    let ghigh = self.csprit[palnum][colnum][1] >> 6;
                    let b = self.csprit[palnum][colnum][2] >> 3;
                    ghigh | (b << 2)
                }
            }
            _ => 0xFF,
        }
    }

    /// Write GPU register
    pub fn wb(&mut self, addr: u16, value: u8) {
        match addr {
            0x8000..=0x9FFF => {
                if !self.lcd_on || self.mode != 3 {
                    self.vram[self.vram_bank * 0x2000 + (addr as usize & 0x1FFF)] = value;
                }
            }
            0xFE00..=0xFE9F => {
                if !self.lcd_on || (self.mode != 2 && self.mode != 3) {
                    self.oam[addr as usize - 0xFE00] = value;
                }
            }
            0xFF40 => {
                let was_on = self.lcd_on;
                self.lcdc = value;
                self.lcd_on = value & 0x80 != 0;
                self.win_tilemap = if value & 0x40 != 0 { 0x9C00 } else { 0x9800 };
                self.win_on = value & 0x20 != 0;
                self.tilebase = if value & 0x10 != 0 { 0x8000 } else { 0x8800 };
                self.bg_tilemap = if value & 0x08 != 0 { 0x9C00 } else { 0x9800 };
                self.sprite_size = if value & 0x04 != 0 { 16 } else { 8 };
                self.sprite_on = value & 0x02 != 0;
                self.lcdc0 = value & 0x01 != 0;

                if !was_on && self.lcd_on {
                    self.modeclock = 0;
                    self.mode = 0;
                    self.line = 0;
                    self.wy_trigger = false;
                    self.wy_pos = -1;
                    self.first_frame = true;
                    self.update_stat_interrupt();
                }
                if was_on && !self.lcd_on {
                    self.clear_screen();
                }
            }
            0xFF41 => {
                self.stat = (self.stat & 0x07) | (value & 0x78);
            }
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => {} // LY is read-only
            0xFF45 => {
                self.lyc = value;
                if self.lcd_on {
                    self.update_stat_interrupt();
                }
            }
            0xFF47 => {
                self.palbr = value;
                self.update_pal();
            }
            0xFF48 => {
                self.pal0r = value;
                self.update_pal();
            }
            0xFF49 => {
                self.pal1r = value;
                self.update_pal();
            }
            0xFF4A => self.wy = value,
            0xFF4B => self.winx = value,
            0xFF4F => self.vram_bank = (value & 1) as usize,
            0xFF68 => {
                self.cbgpal_ind = value & 0x3F;
                self.cbgpal_inc = value & 0x80 != 0;
            }
            0xFF69 => {
                let palnum = (self.cbgpal_ind >> 3) as usize;
                let colnum = ((self.cbgpal_ind >> 1) & 0x3) as usize;
                if self.cbgpal_ind & 1 == 0 {
                    self.cbgpal[palnum][colnum][0] = (value & 0x1F) << 3;
                    self.cbgpal[palnum][colnum][1] =
                        (self.cbgpal[palnum][colnum][1] & 0xC0) | ((value >> 5) << 3);
                } else {
                    self.cbgpal[palnum][colnum][1] =
                        (self.cbgpal[palnum][colnum][1] & 0x38) | ((value & 0x3) << 6);
                    self.cbgpal[palnum][colnum][2] = ((value >> 2) & 0x1F) << 3;
                }
                if self.cbgpal_inc {
                    self.cbgpal_ind = (self.cbgpal_ind + 1) & 0x3F;
                }
            }
            0xFF6A => {
                self.csprit_ind = value & 0x3F;
                self.csprit_inc = value & 0x80 != 0;
            }
            0xFF6B => {
                let palnum = (self.csprit_ind >> 3) as usize;
                let colnum = ((self.csprit_ind >> 1) & 0x3) as usize;
                if self.csprit_ind & 1 == 0 {
                    self.csprit[palnum][colnum][0] = (value & 0x1F) << 3;
                    self.csprit[palnum][colnum][1] =
                        (self.csprit[palnum][colnum][1] & 0xC0) | ((value >> 5) << 3);
                } else {
                    self.csprit[palnum][colnum][1] =
                        (self.csprit[palnum][colnum][1] & 0x38) | ((value & 0x3) << 6);
                    self.csprit[palnum][colnum][2] = ((value >> 2) & 0x1F) << 3;
                }
                if self.csprit_inc {
                    self.csprit_ind = (self.csprit_ind + 1) & 0x3F;
                }
            }
            _ => {}
        }
    }

    /// Advance GPU by given cycles
    pub fn do_cycle(&mut self, cycles: u32) {
        if !self.lcd_on {
            return;
        }

        self.modeclock += cycles;

        match self.mode {
            // OAM search (mode 2)
            2 => {
                if self.modeclock >= 80 {
                    self.modeclock -= 80;
                    self.mode = 3;
                }
            }
            // Pixel transfer (mode 3)
            3 => {
                if self.modeclock >= 172 {
                    self.modeclock -= 172;
                    self.mode = 0;
                    self.renderscan();
                    self.update_stat_interrupt();
                }
            }
            // H-Blank (mode 0)
            0 => {
                if self.modeclock >= 204 {
                    self.modeclock -= 204;
                    self.line += 1;

                    if self.line == 144 {
                        self.mode = 1;
                        self.interrupt |= 0x01; // VBlank interrupt
                        self.updated = true;
                        self.wy_trigger = false;
                        self.wy_pos = -1;
                        self.first_frame = false;
                    } else {
                        if !self.wy_trigger && self.line == self.wy {
                            self.wy_trigger = true;
                        }
                        self.mode = 2;
                    }
                    self.update_stat_interrupt();
                }
            }
            // V-Blank (mode 1)
            1 => {
                if self.modeclock >= 456 {
                    self.modeclock -= 456;
                    self.line += 1;

                    if self.line > 153 {
                        self.mode = 2;
                        self.line = 0;
                        if self.wy == 0 {
                            self.wy_trigger = true;
                        }
                    }
                    self.update_stat_interrupt();
                }
            }
            _ => {}
        }
    }

    fn update_stat_interrupt(&mut self) {
        let lyc_match = self.line == self.lyc;

        // Update stat register
        self.stat = (self.stat & 0xFC) | self.mode;
        if lyc_match {
            self.stat |= 0x04;
        } else {
            self.stat &= !0x04;
        }

        // Check for STAT interrupt
        let trigger = (self.stat & 0x40 != 0 && lyc_match)
            || (self.stat & 0x20 != 0 && self.mode == 2)
            || (self.stat & 0x10 != 0 && self.mode == 1)
            || (self.stat & 0x08 != 0 && self.mode == 0);

        if trigger {
            self.interrupt |= 0x02;
        }
    }

    fn clear_screen(&mut self) {
        for v in self.data.iter_mut() {
            *v = 255;
        }
        self.updated = true;
    }

    fn update_pal(&mut self) {
        for i in 0..4 {
            self.palb[i] = Self::get_mono_pal_val(self.palbr, i);
            self.pal0[i] = Self::get_mono_pal_val(self.pal0r, i);
            self.pal1[i] = Self::get_mono_pal_val(self.pal1r, i);
        }
    }

    fn get_mono_pal_val(value: u8, index: usize) -> u8 {
        match (value >> (2 * index)) & 0x03 {
            0 => 255,
            1 => 192,
            2 => 96,
            _ => 0,
        }
    }

    fn renderscan(&mut self) {
        if self.first_frame {
            return;
        }

        for x in 0..SCREEN_W {
            self.setcolor(x, 255);
            self.bgprio[x] = PrioType::Normal;
        }

        self.draw_bg();
        self.draw_sprites();
    }

    fn setcolor(&mut self, x: usize, color: u8) {
        let idx = self.line as usize * SCREEN_W * 3 + x * 3;
        self.data[idx] = color;
        self.data[idx + 1] = color;
        self.data[idx + 2] = color;
    }

    fn setrgb(&mut self, x: usize, r: u8, g: u8, b: u8) {
        let idx = self.line as usize * SCREEN_W * 3 + x * 3;
        // GBC color correction
        let r = r as u32;
        let g = g as u32;
        let b = b as u32;
        self.data[idx] = ((r * 13 + g * 2 + b) >> 1) as u8;
        self.data[idx + 1] = ((g * 3 + b) << 1) as u8;
        self.data[idx + 2] = ((r * 3 + g * 2 + b * 11) >> 1) as u8;
    }

    fn draw_bg(&mut self) {
        let drawbg = self.gbmode == GbMode::Color || self.lcdc0;

        let wx_trigger = self.winx <= 166;
        let winy = if self.win_on && self.wy_trigger && wx_trigger {
            self.wy_pos += 1;
            self.wy_pos
        } else {
            -1
        };

        if winy < 0 && !drawbg {
            return;
        }

        let wintiley = (winy as u16 >> 3) & 31;
        let bgy = self.scy.wrapping_add(self.line);
        let bgtiley = (bgy as u16 >> 3) & 31;

        for x in 0..SCREEN_W {
            let winx = -((self.winx as i32) - 7) + (x as i32);
            let bgx = self.scx as u32 + x as u32;

            let (tilemapbase, tiley, tilex, pixely, pixelx) = if winy >= 0 && winx >= 0 {
                (
                    self.win_tilemap,
                    wintiley,
                    (winx as u16 >> 3),
                    winy as u16 & 0x07,
                    winx as u8 & 0x07,
                )
            } else if drawbg {
                (
                    self.bg_tilemap,
                    bgtiley,
                    ((bgx >> 3) & 31) as u16,
                    (bgy & 7) as u16,
                    (bgx & 7) as u8,
                )
            } else {
                continue;
            };

            let tilemapaddr = tilemapbase + tiley * 32 + tilex;
            let tileidx = self.vram[tilemapaddr as usize & 0x1FFF];

            let (tileaddry, tilexbit, bgp, bgprio) = if self.gbmode == GbMode::Color {
                let flags = self.vram[0x2000 + (tilemapaddr as usize & 0x1FFF)];
                let bank = if flags & 0x08 != 0 { 0x2000 } else { 0 };
                let flipy = if flags & 0x40 != 0 {
                    7 - pixely
                } else {
                    pixely
                };
                let flipx = if flags & 0x20 != 0 { pixelx } else { 7 - pixelx };
                let bgp = (flags & 0x07) as usize;
                let prio = if flags & 0x80 != 0 {
                    PrioType::Priority
                } else {
                    PrioType::Normal
                };
                let addr = self.get_tile_addr(tileidx, flipy) + bank;
                (addr, flipx, bgp, prio)
            } else {
                (
                    self.get_tile_addr(tileidx, pixely),
                    7 - pixelx,
                    0,
                    PrioType::Normal,
                )
            };

            let lo = self.vram[tileaddry as usize];
            let hi = self.vram[tileaddry as usize + 1];
            let colorbit = ((lo >> tilexbit) & 1) | (((hi >> tilexbit) & 1) << 1);

            if self.gbmode == GbMode::Color {
                self.setrgb(
                    x,
                    self.cbgpal[bgp][colorbit as usize][0],
                    self.cbgpal[bgp][colorbit as usize][1],
                    self.cbgpal[bgp][colorbit as usize][2],
                );
                self.bgprio[x] = if colorbit == 0 {
                    PrioType::Color0
                } else {
                    bgprio
                };
            } else {
                self.setcolor(x, self.palb[colorbit as usize]);
                self.bgprio[x] = if colorbit == 0 {
                    PrioType::Color0
                } else {
                    PrioType::Normal
                };
            }
        }
    }

    fn get_tile_addr(&self, tileidx: u8, row: u16) -> usize {
        let addr = if self.tilebase == 0x8000 {
            self.tilebase + tileidx as u16 * 16
        } else {
            (self.tilebase as i32 + (tileidx as i8 as i32 + 128) * 16) as u16
        };
        (addr + row * 2) as usize & 0x1FFF
    }

    fn draw_sprites(&mut self) {
        if !self.sprite_on {
            return;
        }

        let line = self.line as i32;
        let sprite_height = self.sprite_size as i32;

        // Count visible sprites (max 10 per line)
        let mut sprites: [(i32, usize); 10] = [(-1, 0); 10];
        let mut count = 0;

        for i in 0..40 {
            let idx = i * 4;
            let ypos = self.oam[idx] as i32 - 16;
            let xpos = self.oam[idx + 1] as i32 - 8;

            if line >= ypos && line < ypos + sprite_height {
                if count < 10 {
                    sprites[count] = (xpos, i);
                    count += 1;
                } else {
                    break;
                }
            }
        }

        // Sort by X position (lower X has priority), then by OAM index
        sprites[..count].sort_by(|a, b| {
            if a.0 != b.0 {
                b.0.cmp(&a.0) // Higher X drawn first (lower priority)
            } else {
                b.1.cmp(&a.1) // Higher index drawn first
            }
        });

        for &(_, i) in sprites[..count].iter() {
            let idx = i * 4;
            let ypos = self.oam[idx] as i32 - 16;
            let xpos = self.oam[idx + 1] as i32 - 8;
            let mut tileidx = self.oam[idx + 2];
            let flags = self.oam[idx + 3];

            if sprite_height == 16 {
                tileidx &= 0xFE;
            }

            let flipy = flags & 0x40 != 0;
            let flipx = flags & 0x20 != 0;
            let bgprio = flags & 0x80 != 0;

            let mut row = (line - ypos) as u16;
            if flipy {
                row = (sprite_height as u16) - 1 - row;
            }

            let (bank, palette) = if self.gbmode == GbMode::Color {
                let bank = if flags & 0x08 != 0 { 0x2000 } else { 0 };
                let pal = (flags & 0x07) as usize;
                (bank, pal)
            } else {
                let pal = if flags & 0x10 != 0 { 1 } else { 0 };
                (0, pal)
            };

            let tileaddr = 0x8000 + tileidx as usize * 16 + row as usize * 2;
            let lo = self.vram[bank + (tileaddr & 0x1FFF)];
            let hi = self.vram[bank + ((tileaddr + 1) & 0x1FFF)];

            for px in 0..8 {
                let screenx = xpos + px;
                if screenx < 0 || screenx >= SCREEN_W as i32 {
                    continue;
                }

                let bit = if flipx { px } else { 7 - px } as u8;
                let colorbit = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);

                if colorbit == 0 {
                    continue;
                }

                let draw = if self.gbmode == GbMode::Color {
                    !self.lcdc0
                        || (self.bgprio[screenx as usize] == PrioType::Color0)
                        || (!bgprio && self.bgprio[screenx as usize] != PrioType::Priority)
                } else {
                    !bgprio || self.bgprio[screenx as usize] == PrioType::Color0
                };

                if draw {
                    if self.gbmode == GbMode::Color {
                        self.setrgb(
                            screenx as usize,
                            self.csprit[palette][colorbit as usize][0],
                            self.csprit[palette][colorbit as usize][1],
                            self.csprit[palette][colorbit as usize][2],
                        );
                    } else {
                        let pal = if palette == 0 { &self.pal0 } else { &self.pal1 };
                        self.setcolor(screenx as usize, pal[colorbit as usize]);
                    }
                }
            }
        }
    }
}
