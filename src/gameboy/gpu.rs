//! GPU (Picture Processing Unit) emulation
//!
//! Handles all graphics rendering for the Game Boy.
//!
//! Screen dimensions: 160x144 pixels
//! Tiles: 8x8 pixels
//! Background: 256x256 pixels (32x32 tiles)
//! Window: overlay on background
//! Sprites: up to 40, 10 per scanline

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::gbmode::GbMode;

/// Screen width in pixels
pub const SCREEN_W: usize = 160;
/// Screen height in pixels
pub const SCREEN_H: usize = 144;

const VRAM_SIZE: usize = 0x4000; // (16 KB for CGB, 8 KB for DMG)
const OAM_SIZE: usize = 0xA0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrioType {
    Color0,
    PrioFlag,
    Normal,
}

/// GPU state
pub struct GPU {
    /// Current mode (0-3)
    mode: u8,
    /// Mode clock counter
    mode_clock: u32,
    /// Current scanline
    line: u8,
    /// LY compare value
    lyc: u8,
    /// LCD enabled
    lcd_on: bool,
    /// Window tile map base (0x9800 or 0x9C00)
    win_tilemap: u16,
    /// Window enabled
    win_on: bool,
    /// Tile data base (0x8000 or 0x8800)
    tilebase: u16,
    /// Background tile map base
    bg_tilemap: u16,
    /// Sprite size (8 or 16)
    sprite_size: u32,
    /// Sprites enabled
    sprite_on: bool,
    /// LCDC bit 0 (BG enable on DMG, priority on CGB)
    lcdc0: bool,
    /// LYC=LY interrupt enable
    lyc_inte: bool,
    /// Mode 0 (H-Blank) interrupt enable
    m0_inte: bool,
    /// Mode 1 (V-Blank) interrupt enable
    m1_inte: bool,
    /// Mode 2 (OAM search) interrupt enable
    m2_inte: bool,
    /// Scroll Y
    scy: u8,
    /// Scroll X
    scx: u8,
    /// Window Y
    winy: u8,
    /// Window X
    winx: u8,
    /// Window Y triggered this frame
    wy_trigger: bool,
    /// Window internal line counter
    wy_pos: i32,
    /// Background palette register (DMG)
    palbr: u8,
    /// Object palette 0 register (DMG)
    pal0r: u8,
    /// Object palette 1 register (DMG)
    pal1r: u8,
    /// Background palette (DMG)
    palb: [u8; 4],
    /// Object palette 0 (DMG)
    pal0: [u8; 4],
    /// Object palette 1 (DMG)
    pal1: [u8; 4],
    /// Video RAM
    vram: Box<[u8; VRAM_SIZE]>,
    /// Object Attribute Memory
    oam: [u8; OAM_SIZE],
    /// CGB background palette auto-increment
    cbgpal_inc: bool,
    /// CGB background palette index
    cbgpal_ind: u8,
    /// CGB background palettes (8 palettes x 4 colors x 3 bytes RGB)
    cbgpal: [[[u8; 3]; 4]; 8],
    /// CGB sprite palette auto-increment
    csprit_inc: bool,
    /// CGB sprite palette index
    csprit_ind: u8,
    /// CGB sprite palettes
    csprit: [[[u8; 3]; 4]; 8],
    /// VRAM bank (CGB)
    vrambank: usize,
    /// Framebuffer (RGB888)
    pub data: Vec<u8>,
    /// Background priority array (per-pixel)
    bgprio: [PrioType; SCREEN_W],
    /// Frame has been updated
    pub updated: bool,
    /// Pending interrupt
    pub interrupt: u8,
    /// Game Boy mode
    pub gbmode: GbMode,
    /// H-blank in progress
    hblanking: bool,
    /// First frame after LCD enable (skip rendering)
    first_frame: bool,
}

impl GPU {
    /// Create a new GPU for DMG mode
    pub fn new() -> Self {
        Self {
            mode: 0,
            mode_clock: 0,
            line: 0,
            lyc: 0,
            lcd_on: false,
            win_tilemap: 0x9C00,
            win_on: false,
            tilebase: 0x8000,
            bg_tilemap: 0x9C00,
            sprite_size: 8,
            sprite_on: false,
            lcdc0: false,
            lyc_inte: false,
            m2_inte: false,
            m1_inte: false,
            m0_inte: false,
            scy: 0,
            scx: 0,
            winy: 0,
            winx: 0,
            wy_trigger: false,
            wy_pos: -1,
            palbr: 0,
            pal0r: 0,
            pal1r: 1,
            palb: [0; 4],
            pal0: [0; 4],
            pal1: [0; 4],
            vram: Box::new([0; VRAM_SIZE]),
            oam: [0; OAM_SIZE],
            data: vec![0; SCREEN_W * SCREEN_H * 3],
            bgprio: [PrioType::Normal; SCREEN_W],
            updated: false,
            interrupt: 0,
            gbmode: GbMode::Classic,
            cbgpal_inc: false,
            cbgpal_ind: 0,
            cbgpal: [[[0u8; 3]; 4]; 8],
            csprit_inc: false,
            csprit_ind: 0,
            csprit: [[[0u8; 3]; 4]; 8],
            vrambank: 0,
            hblanking: false,
            first_frame: false,
        }
    }

    /// Create a new GPU for CGB mode
    pub fn new_cgb() -> Self {
        Self::new()
    }

    /// Run the GPU for the given number of cycles
    pub fn do_cycle(&mut self, ticks: u32) {
        if !self.lcd_on {
            return;
        }

        self.hblanking = false;
        let mut remaining = ticks;

        while remaining > 0 {
            let step = remaining.min(80);
            self.mode_clock += step;
            remaining -= step;

            // Full scanline = 456 cycles
            if self.mode_clock >= 456 {
                self.mode_clock -= 456;
                self.line = (self.line + 1) % 154;
                self.check_lyc_interrupt();

                // V-Blank (lines 144-153)
                if self.line >= 144 && self.mode != 1 {
                    self.change_mode(1);
                }
            }

            // Normal scanline (lines 0-143)
            if self.line < 144 {
                if self.mode_clock <= 80 {
                    // Mode 2: OAM search
                    if self.mode != 2 {
                        self.change_mode(2);
                    }
                } else if self.mode_clock <= 252 {
                    // Mode 3: Drawing
                    if self.mode != 3 {
                        self.change_mode(3);
                    }
                } else {
                    // Mode 0: H-Blank
                    if self.mode != 0 {
                        self.change_mode(0);
                    }
                }
            }
        }
    }

    fn check_lyc_interrupt(&mut self) {
        if self.lyc_inte && self.line == self.lyc {
            self.interrupt |= 0x02;
        }
    }

    fn change_mode(&mut self, mode: u8) {
        self.mode = mode;

        let trigger_stat = match self.mode {
            0 => {
                self.render_scanline();
                self.hblanking = true;
                self.m0_inte
            }
            1 => {
                self.wy_trigger = false;
                self.interrupt |= 0x01;
                self.updated = true;
                self.first_frame = false;
                self.m1_inte
            }
            2 => self.m2_inte,
            3 => {
                if self.win_on && !self.wy_trigger && self.line == self.winy {
                    self.wy_trigger = true;
                    self.wy_pos = -1;
                }
                false
            }
            _ => false,
        };

        if trigger_stat {
            self.interrupt |= 0x02;
        }
    }

    /// Read a GPU register
    #[inline]
    pub fn rb(&self, address: u16) -> u8 {
        match address {
            0x8000..=0x9FFF => self.vram[(self.vrambank * 0x2000) | (address as usize & 0x1FFF)],
            0xFE00..=0xFE9F => self.oam[address as usize - 0xFE00],
            0xFF40 => {
                (if self.lcd_on { 0x80 } else { 0 })
                    | (if self.win_tilemap == 0x9C00 { 0x40 } else { 0 })
                    | (if self.win_on { 0x20 } else { 0 })
                    | (if self.tilebase == 0x8000 { 0x10 } else { 0 })
                    | (if self.bg_tilemap == 0x9C00 { 0x08 } else { 0 })
                    | (if self.sprite_size == 16 { 0x04 } else { 0 })
                    | (if self.sprite_on { 0x02 } else { 0 })
                    | (if self.lcdc0 { 0x01 } else { 0 })
            }
            0xFF41 => {
                0x80 | (if self.lyc_inte { 0x40 } else { 0 })
                    | (if self.m2_inte { 0x20 } else { 0 })
                    | (if self.m1_inte { 0x10 } else { 0 })
                    | (if self.m0_inte { 0x08 } else { 0 })
                    | (if self.line == self.lyc { 0x04 } else { 0 })
                    | self.mode
            }
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.line,
            0xFF45 => self.lyc,
            0xFF46 => 0,
            0xFF47 => self.palbr,
            0xFF48 => self.pal0r,
            0xFF49 => self.pal1r,
            0xFF4A => self.winy,
            0xFF4B => self.winx,
            0xFF4C | 0xFF4E => 0xFF,
            0xFF4F..=0xFF6B if self.gbmode != GbMode::Color => 0xFF,
            0xFF4F => self.vrambank as u8 | 0xFE,
            0xFF68 => 0x40 | self.cbgpal_ind | (if self.cbgpal_inc { 0x80 } else { 0 }),
            0xFF69 => {
                let palnum = (self.cbgpal_ind >> 3) as usize;
                let colnum = ((self.cbgpal_ind >> 1) & 0x3) as usize;
                if self.cbgpal_ind & 0x01 == 0x00 {
                    self.cbgpal[palnum][colnum][0] | ((self.cbgpal[palnum][colnum][1] & 0x07) << 5)
                } else {
                    ((self.cbgpal[palnum][colnum][1] & 0x18) >> 3)
                        | (self.cbgpal[palnum][colnum][2] << 2)
                }
            }
            0xFF6A => 0x40 | self.csprit_ind | (if self.csprit_inc { 0x80 } else { 0 }),
            0xFF6B => {
                let palnum = (self.csprit_ind >> 3) as usize;
                let colnum = ((self.csprit_ind >> 1) & 0x3) as usize;
                if self.csprit_ind & 0x01 == 0x00 {
                    self.csprit[palnum][colnum][0] | ((self.csprit[palnum][colnum][1] & 0x07) << 5)
                } else {
                    ((self.csprit[palnum][colnum][1] & 0x18) >> 3)
                        | (self.csprit[palnum][colnum][2] << 2)
                }
            }
            _ => 0xFF,
        }
    }

    #[inline]
    fn rbvram0(&self, addr: u16) -> u8 {
        self.vram[addr as usize & 0x1FFF]
    }

    #[inline]
    fn rbvram1(&self, addr: u16) -> u8 {
        self.vram[0x2000 + (addr as usize & 0x1FFF)]
    }

    /// Write to a GPU register
    #[inline]
    pub fn wb(&mut self, address: u16, value: u8) {
        match address {
            0x8000..=0x9FFF => {
                self.vram[(self.vrambank * 0x2000) | (address as usize & 0x1FFF)] = value
            }
            0xFE00..=0xFE9F => self.oam[address as usize - 0xFE00] = value,
            0xFF40 => {
                let was_on = self.lcd_on;
                self.lcd_on = value & 0x80 != 0;
                self.win_tilemap = if value & 0x40 != 0 { 0x9C00 } else { 0x9800 };
                self.win_on = value & 0x20 != 0;
                self.tilebase = if value & 0x10 != 0 { 0x8000 } else { 0x8800 };
                self.bg_tilemap = if value & 0x08 != 0 { 0x9C00 } else { 0x9800 };
                self.sprite_size = if value & 0x04 != 0 { 16 } else { 8 };
                self.sprite_on = value & 0x02 != 0;
                self.lcdc0 = value & 0x01 != 0;

                if was_on && !self.lcd_on {
                    self.mode_clock = 0;
                    self.line = 0;
                    self.mode = 0;
                    self.wy_trigger = false;
                    self.first_frame = true;
                    self.clear_screen();
                }
                if !was_on && self.lcd_on {
                    self.change_mode(2);
                    self.mode_clock = 4;
                }
            }
            0xFF41 => {
                self.lyc_inte = value & 0x40 != 0;
                self.m2_inte = value & 0x20 != 0;
                self.m1_inte = value & 0x10 != 0;
                self.m0_inte = value & 0x08 != 0;
            }
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => {} // Read-only
            0xFF45 => {
                self.lyc = value;
                self.check_lyc_interrupt();
            }
            0xFF46 => panic!("OAM DMA should be handled by MMU"),
            0xFF47 => {
                self.palbr = value;
                self.update_palettes();
            }
            0xFF48 => {
                self.pal0r = value;
                self.update_palettes();
            }
            0xFF49 => {
                self.pal1r = value;
                self.update_palettes();
            }
            0xFF4A => self.winy = value,
            0xFF4B => self.winx = value,
            0xFF4C | 0xFF4E => {}
            0xFF4F..=0xFF6B if self.gbmode != GbMode::Color => {}
            0xFF4F => self.vrambank = (value & 0x01) as usize,
            0xFF68 => {
                self.cbgpal_ind = value & 0x3F;
                self.cbgpal_inc = value & 0x80 != 0;
            }
            0xFF69 => {
                let palnum = (self.cbgpal_ind >> 3) as usize;
                let colnum = ((self.cbgpal_ind >> 1) & 0x03) as usize;
                if self.cbgpal_ind & 0x01 == 0x00 {
                    self.cbgpal[palnum][colnum][0] = value & 0x1F;
                    self.cbgpal[palnum][colnum][1] =
                        (self.cbgpal[palnum][colnum][1] & 0x18) | (value >> 5);
                } else {
                    self.cbgpal[palnum][colnum][1] =
                        (self.cbgpal[palnum][colnum][1] & 0x07) | ((value & 0x3) << 3);
                    self.cbgpal[palnum][colnum][2] = (value >> 2) & 0x1F;
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
                let colnum = ((self.csprit_ind >> 1) & 0x03) as usize;
                if self.csprit_ind & 0x01 == 0x00 {
                    self.csprit[palnum][colnum][0] = value & 0x1F;
                    self.csprit[palnum][colnum][1] =
                        (self.csprit[palnum][colnum][1] & 0x18) | (value >> 5);
                } else {
                    self.csprit[palnum][colnum][1] =
                        (self.csprit[palnum][colnum][1] & 0x07) | ((value & 0x3) << 3);
                    self.csprit[palnum][colnum][2] = (value >> 2) & 0x1F;
                }
                if self.csprit_inc {
                    self.csprit_ind = (self.csprit_ind + 1) & 0x3F;
                }
            }
            _ => {}
        }
    }

    fn clear_screen(&mut self) {
        for v in self.data.iter_mut() {
            *v = 255;
        }
        self.updated = true;
    }

    fn update_palettes(&mut self) {
        for i in 0..4 {
            self.palb[i] = Self::get_mono_color(self.palbr, i);
            self.pal0[i] = Self::get_mono_color(self.pal0r, i);
            self.pal1[i] = Self::get_mono_color(self.pal1r, i);
        }
    }

    fn get_mono_color(palette: u8, index: usize) -> u8 {
        match (palette >> (2 * index)) & 0x03 {
            0 => 255,
            1 => 192,
            2 => 96,
            _ => 0,
        }
    }

    fn render_scanline(&mut self) {
        if self.first_frame {
            return;
        }

        // Clear line
        for x in 0..SCREEN_W {
            self.set_color(x, 255);
            self.bgprio[x] = PrioType::Normal;
        }

        self.draw_background();
        self.draw_sprites();
    }

    #[inline]
    fn set_color(&mut self, x: usize, color: u8) {
        let base = self.line as usize * SCREEN_W * 3 + x * 3;
        self.data[base] = color;
        self.data[base + 1] = color;
        self.data[base + 2] = color;
    }

    #[inline]
    fn set_rgb(&mut self, x: usize, r: u8, g: u8, b: u8) {
        // CGB color correction (Gambatte algorithm)
        let base = self.line as usize * SCREEN_W * 3 + x * 3;
        let r = r as u32;
        let g = g as u32;
        let b = b as u32;

        self.data[base] = ((r * 13 + g * 2 + b) >> 1) as u8;
        self.data[base + 1] = ((g * 3 + b) << 1) as u8;
        self.data[base + 2] = ((r * 3 + g * 2 + b * 11) >> 1) as u8;
    }

    fn draw_background(&mut self) {
        let draw_bg = self.gbmode == GbMode::Color || self.lcdc0;

        let wx_trigger = self.winx <= 166;
        let winy = if self.win_on && self.wy_trigger && wx_trigger {
            self.wy_pos += 1;
            self.wy_pos
        } else {
            -1
        };

        if winy < 0 && !draw_bg {
            return;
        }

        let win_tiley = (winy as u16 >> 3) & 31;
        let bgy = self.scy.wrapping_add(self.line);
        let bg_tiley = (bgy as u16 >> 3) & 31;

        for x in 0..SCREEN_W {
            let winx = -((self.winx as i32) - 7) + (x as i32);
            let bgx = self.scx as u32 + x as u32;

            let (tilemap_base, tiley, tilex, pixely, pixelx) = if winy >= 0 && winx >= 0 {
                (
                    self.win_tilemap,
                    win_tiley,
                    winx as u16 >> 3,
                    winy as u16 & 0x07,
                    winx as u8 & 0x07,
                )
            } else if draw_bg {
                (
                    self.bg_tilemap,
                    bg_tiley,
                    (bgx as u16 >> 3) & 31,
                    bgy as u16 & 0x07,
                    bgx as u8 & 0x07,
                )
            } else {
                continue;
            };

            let tile_addr = tilemap_base + tiley * 32 + tilex;
            let tilenr = self.rbvram0(tile_addr);

            let (palnr, vram1, xflip, yflip, prio) = if self.gbmode == GbMode::Color {
                let flags = self.rbvram1(tile_addr) as usize;
                (
                    flags & 0x07,
                    flags & (1 << 3) != 0,
                    flags & (1 << 5) != 0,
                    flags & (1 << 6) != 0,
                    flags & (1 << 7) != 0,
                )
            } else {
                (0, false, false, false, false)
            };

            let tile_data_addr = self.tilebase
                + (if self.tilebase == 0x8000 {
                    tilenr as u16
                } else {
                    (tilenr as i8 as i16 + 128) as u16
                }) * 16;

            let row_addr = if yflip {
                tile_data_addr + (14 - (pixely * 2))
            } else {
                tile_data_addr + (pixely * 2)
            };

            let (b1, b2) = if vram1 {
                (self.rbvram1(row_addr), self.rbvram1(row_addr + 1))
            } else {
                (self.rbvram0(row_addr), self.rbvram0(row_addr + 1))
            };

            let xbit = if xflip { pixelx } else { 7 - pixelx } as u32;
            let colnr = ((b1 >> xbit) & 1) | (((b2 >> xbit) & 1) << 1);

            self.bgprio[x] = if colnr == 0 {
                PrioType::Color0
            } else if prio {
                PrioType::PrioFlag
            } else {
                PrioType::Normal
            };

            if self.gbmode == GbMode::Color {
                let r = self.cbgpal[palnr][colnr as usize][0];
                let g = self.cbgpal[palnr][colnr as usize][1];
                let b = self.cbgpal[palnr][colnr as usize][2];
                self.set_rgb(x, r, g, b);
            } else {
                self.set_color(x, self.palb[colnr as usize]);
            }
        }
    }

    fn draw_sprites(&mut self) {
        if !self.sprite_on {
            return;
        }

        let line = self.line as i32;
        let sprite_size = self.sprite_size as i32;

        // Collect visible sprites (max 10 per line)
        let mut sprites = [(0i32, 0i32, 0u8); 10];
        let mut count = 0;

        for i in 0..40 {
            let addr = 0xFE00 + (i as u16) * 4;
            let y = self.rb(addr) as i32 - 16;
            if line < y || line >= y + sprite_size {
                continue;
            }
            let x = self.rb(addr + 1) as i32 - 8;
            sprites[count] = (x, y, i);
            count += 1;
            if count >= 10 {
                break;
            }
        }

        // Sort by priority
        let sprites = &mut sprites[..count];
        if self.gbmode == GbMode::Color {
            sprites.sort_unstable_by(cgb_sprite_order);
        } else {
            sprites.sort_unstable_by(dmg_sprite_order);
        }

        for &(sprite_x, sprite_y, i) in sprites.iter() {
            if sprite_x < -7 || sprite_x >= SCREEN_W as i32 {
                continue;
            }

            let addr = 0xFE00 + (i as u16) * 4;
            let tilenr = (self.rb(addr + 2) & if self.sprite_size == 16 { 0xFE } else { 0xFF }) as u16;
            let flags = self.rb(addr + 3) as usize;

            let use_pal1 = flags & (1 << 4) != 0;
            let xflip = flags & (1 << 5) != 0;
            let yflip = flags & (1 << 6) != 0;
            let below_bg = flags & (1 << 7) != 0;
            let cgb_palnr = flags & 0x07;
            let cgb_vram1 = flags & (1 << 3) != 0;

            let tiley = if yflip {
                (sprite_size - 1 - (line - sprite_y)) as u16
            } else {
                (line - sprite_y) as u16
            };

            let tile_addr = 0x8000u16 + tilenr * 16 + tiley * 2;
            let (b1, b2) = if cgb_vram1 && self.gbmode == GbMode::Color {
                (self.rbvram1(tile_addr), self.rbvram1(tile_addr + 1))
            } else {
                (self.rbvram0(tile_addr), self.rbvram0(tile_addr + 1))
            };

            for px in 0..8 {
                let screen_x = sprite_x + px;
                if screen_x < 0 || screen_x >= SCREEN_W as i32 {
                    continue;
                }

                let xbit = 1 << (if xflip { px } else { 7 - px } as u32);
                let colnr = ((b1 & xbit != 0) as usize) | (((b2 & xbit != 0) as usize) << 1);

                if colnr == 0 {
                    continue;
                }

                let sx = screen_x as usize;
                if self.gbmode == GbMode::Color {
                    if self.lcdc0
                        && (self.bgprio[sx] == PrioType::PrioFlag
                            || (below_bg && self.bgprio[sx] != PrioType::Color0))
                    {
                        continue;
                    }
                    let r = self.csprit[cgb_palnr][colnr][0];
                    let g = self.csprit[cgb_palnr][colnr][1];
                    let b = self.csprit[cgb_palnr][colnr][2];
                    self.set_rgb(sx, r, g, b);
                } else {
                    if below_bg && self.bgprio[sx] != PrioType::Color0 {
                        continue;
                    }
                    let color = if use_pal1 {
                        self.pal1[colnr]
                    } else {
                        self.pal0[colnr]
                    };
                    self.set_color(sx, color);
                }
            }
        }
    }

    /// Check if H-DMA can proceed
    pub fn may_hdma(&self) -> bool {
        self.hblanking
    }

    /// Serialize GPU state
    pub fn serialize(&self, output: &mut Vec<u8>) {
        output.push(self.mode);
        output.extend_from_slice(&self.mode_clock.to_le_bytes());
        output.push(self.line);
        output.push(self.lyc);
        output.push(self.lcd_on as u8);
        output.extend_from_slice(&self.win_tilemap.to_le_bytes());
        output.push(self.win_on as u8);
        output.extend_from_slice(&self.tilebase.to_le_bytes());
        output.extend_from_slice(&self.bg_tilemap.to_le_bytes());
        output.extend_from_slice(&self.sprite_size.to_le_bytes());
        output.push(self.sprite_on as u8);
        output.push(self.lcdc0 as u8);
        output.push(
            (self.lyc_inte as u8) << 3
                | (self.m0_inte as u8) << 2
                | (self.m1_inte as u8) << 1
                | (self.m2_inte as u8),
        );
        output.push(self.scy);
        output.push(self.scx);
        output.push(self.winy);
        output.push(self.winx);
        output.push(self.palbr);
        output.push(self.pal0r);
        output.push(self.pal1r);
        output.extend_from_slice(&self.vram[..]);
        output.extend_from_slice(&self.oam);
        // CGB palettes
        for pal in &self.cbgpal {
            for col in pal {
                output.extend_from_slice(col);
            }
        }
        for pal in &self.csprit {
            for col in pal {
                output.extend_from_slice(col);
            }
        }
        output.push(self.vrambank as u8);
    }

    /// Deserialize GPU state
    pub fn deserialize(&mut self, data: &[u8]) -> Result<usize, ()> {
        // Simplified - would need full implementation
        if data.len() < VRAM_SIZE + OAM_SIZE + 50 {
            return Err(());
        }
        // TODO: Full deserialization
        Ok(VRAM_SIZE + OAM_SIZE + 200)
    }
}

impl Default for GPU {
    fn default() -> Self {
        Self::new()
    }
}

fn dmg_sprite_order(a: &(i32, i32, u8), b: &(i32, i32, u8)) -> Ordering {
    if a.0 != b.0 {
        b.0.cmp(&a.0)
    } else {
        b.2.cmp(&a.2)
    }
}

fn cgb_sprite_order(a: &(i32, i32, u8), b: &(i32, i32, u8)) -> Ordering {
    b.2.cmp(&a.2)
}
