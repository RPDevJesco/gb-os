//! VGA Palette Management for GBC Color Support
//!
//! Programs the VGA DAC (Digital-to-Analog Converter) to display
//! Game Boy Color palettes in mode 13h.
//!
//! # VGA Palette Layout
//!
//! ```text
//! Index   Purpose
//! 0-31    GBC Background palettes (8 palettes × 4 colors)
//! 32-63   GBC Sprite palettes (8 palettes × 4 colors)
//! 64-79   DMG grayscale (16 shades for Classic mode)
//! 80-95   UI colors (border, overlay text, etc.)
//! 96-255  Reserved / unused
//! ```
//!
//! # GBC to VGA Color Conversion
//!
//! GBC uses 5-bit RGB (0-31 per channel)
//! VGA DAC uses 6-bit RGB (0-63 per channel)
//! Conversion: vga_component = gbc_component * 2

use crate::arch::x86::io::{outb, inb};

// VGA DAC ports
const DAC_WRITE_INDEX: u16 = 0x3C8;  // Write: set palette index for writing
const DAC_DATA: u16 = 0x3C9;          // Write RGB components (3 writes per color)
const DAC_READ_INDEX: u16 = 0x3C7;    // Write: set palette index for reading

// Palette layout constants
pub const PAL_GBC_BG_START: u8 = 0;      // GBC BG palettes 0-7
pub const PAL_GBC_SPRITE_START: u8 = 32; // GBC sprite palettes 0-7
pub const PAL_DMG_START: u8 = 64;        // DMG grayscale
pub const PAL_UI_START: u8 = 80;         // UI elements

// UI color indices (for easy reference)
pub const COLOR_BLACK: u8 = PAL_UI_START;
pub const COLOR_WHITE: u8 = PAL_UI_START + 1;
pub const COLOR_GB_BORDER: u8 = PAL_UI_START + 2;
pub const COLOR_OVERLAY_BG: u8 = PAL_UI_START + 3;
pub const COLOR_OVERLAY_TEXT: u8 = PAL_UI_START + 4;
pub const COLOR_OVERLAY_SHADOW: u8 = PAL_UI_START + 5;

/// Set a single VGA palette entry
///
/// # Arguments
/// * `index` - Palette index (0-255)
/// * `r`, `g`, `b` - Color components (0-63 for VGA DAC)
#[inline]
pub fn set_palette_entry(index: u8, r: u8, g: u8, b: u8) {
    unsafe {
        outb(DAC_WRITE_INDEX, index);
        outb(DAC_DATA, r & 0x3F);  // Mask to 6 bits
        outb(DAC_DATA, g & 0x3F);
        outb(DAC_DATA, b & 0x3F);
    }
}

/// Set a VGA palette entry from GBC 5-bit RGB values
///
/// Converts 5-bit GBC components to 6-bit VGA components
#[inline]
pub fn set_palette_entry_gbc(index: u8, r: u8, g: u8, b: u8) {
    // GBC uses 5 bits (0-31), VGA uses 6 bits (0-63)
    // Simple conversion: multiply by 2 (or shift left by 1)
    // For better accuracy: (val * 63) / 31, but *2 is close enough and faster
    set_palette_entry(index, r << 1, g << 1, b << 1);
}

/// Set a VGA palette entry from 8-bit RGB values
///
/// Converts 8-bit components to 6-bit VGA components
#[inline]
pub fn set_palette_entry_rgb8(index: u8, r: u8, g: u8, b: u8) {
    // 8-bit to 6-bit: shift right by 2
    set_palette_entry(index, r >> 2, g >> 2, b >> 2);
}

/// Sync GBC background palettes to VGA DAC
///
/// # Arguments
/// * `cbgpal` - GBC background palette data [8 palettes][4 colors][3 RGB bytes]
///              RGB values are 5-bit (0-31 range)
pub fn sync_gbc_bg_palettes(cbgpal: &[[[u8; 3]; 4]; 8]) {
    for pal in 0..8 {
        for col in 0..4 {
            let index = PAL_GBC_BG_START + (pal * 4 + col) as u8;
            let rgb = &cbgpal[pal][col];
            // 5-bit (0-31) to 6-bit (0-63): shift left by 1
            set_palette_entry(index, rgb[0] << 1, rgb[1] << 1, rgb[2] << 1);
        }
    }
}

/// Sync GBC sprite palettes to VGA DAC
///
/// # Arguments
/// * `csprit` - GBC sprite palette data [8 palettes][4 colors][3 RGB bytes]
///              RGB values are 5-bit (0-31 range)
pub fn sync_gbc_sprite_palettes(csprit: &[[[u8; 3]; 4]; 8]) {
    for pal in 0..8 {
        for col in 0..4 {
            let index = PAL_GBC_SPRITE_START + (pal * 4 + col) as u8;
            let rgb = &csprit[pal][col];
            // 5-bit (0-31) to 6-bit (0-63): shift left by 1
            set_palette_entry(index, rgb[0] << 1, rgb[1] << 1, rgb[2] << 1);
        }
    }
}

/// Sync DMG grayscale palette
///
/// # Arguments
/// * `palb` - Background palette (4 shades)
/// * `pal0` - Object palette 0 (4 shades)
/// * `pal1` - Object palette 1 (4 shades)
pub fn sync_dmg_palettes(palb: &[u8; 4], pal0: &[u8; 4], pal1: &[u8; 4]) {
    // DMG uses 8-bit grayscale values (255, 192, 96, 0)
    // Map to VGA grayscale
    for i in 0..4 {
        let gray = palb[i];
        set_palette_entry_rgb8(PAL_DMG_START + i as u8, gray, gray, gray);
    }
    for i in 0..4 {
        let gray = pal0[i];
        set_palette_entry_rgb8(PAL_DMG_START + 4 + i as u8, gray, gray, gray);
    }
    for i in 0..4 {
        let gray = pal1[i];
        set_palette_entry_rgb8(PAL_DMG_START + 8 + i as u8, gray, gray, gray);
    }
}

/// Initialize UI palette entries
///
/// Sets up colors used for borders, overlay, text, etc.
pub fn init_ui_palette() {
    // Basic colors
    set_palette_entry(COLOR_BLACK, 0, 0, 0);
    set_palette_entry(COLOR_WHITE, 63, 63, 63);

    // Game Boy border (classic gray-green)
    set_palette_entry(COLOR_GB_BORDER, 20, 24, 20);

    // Overlay colors
    set_palette_entry(COLOR_OVERLAY_BG, 4, 4, 8);       // Dark blue-ish
    set_palette_entry(COLOR_OVERLAY_TEXT, 63, 63, 63); // White
    set_palette_entry(COLOR_OVERLAY_SHADOW, 0, 0, 0);  // Black

    // Additional UI colors (6-15 in UI range)
    set_palette_entry(PAL_UI_START + 6, 63, 0, 0);     // Red (errors)
    set_palette_entry(PAL_UI_START + 7, 0, 63, 0);     // Green (success)
    set_palette_entry(PAL_UI_START + 8, 63, 63, 0);    // Yellow (warnings)
    set_palette_entry(PAL_UI_START + 9, 0, 32, 63);    // Blue (info)
    set_palette_entry(PAL_UI_START + 10, 32, 32, 32);  // Gray (disabled)

    // HP bar colors for Pokemon overlay
    set_palette_entry(PAL_UI_START + 11, 0, 63, 0);    // HP green (full)
    set_palette_entry(PAL_UI_START + 12, 63, 63, 0);   // HP yellow (medium)
    set_palette_entry(PAL_UI_START + 13, 63, 0, 0);    // HP red (low)
    set_palette_entry(PAL_UI_START + 14, 16, 16, 16);  // HP background
}

/// Initialize the full VGA palette for GBC emulation
///
/// Call this once at startup before emulation begins
pub fn init_palette() {
    // Clear all palette entries to black first
    for i in 0..=255u8 {
        set_palette_entry(i, 0, 0, 0);
    }

    // GBC palettes (0-63) start black - games will set their own colors
    // (Already black from the loop above)

    // Initialize DMG grayscale
    let dmg_shades: [u8; 4] = [63, 42, 21, 0]; // White to black
    for i in 0..4 {
        set_palette_entry(PAL_DMG_START + i, dmg_shades[i as usize], dmg_shades[i as usize], dmg_shades[i as usize]);
    }
    for i in 0..4 {
        set_palette_entry(PAL_DMG_START + 4 + i, dmg_shades[i as usize], dmg_shades[i as usize], dmg_shades[i as usize]);
    }
    for i in 0..4 {
        set_palette_entry(PAL_DMG_START + 8 + i, dmg_shades[i as usize], dmg_shades[i as usize], dmg_shades[i as usize]);
    }

    // Initialize UI colors
    init_ui_palette();
}

/// Calculate VGA palette index for a GBC background pixel
///
/// # Arguments
/// * `palette` - GBC palette number (0-7)
/// * `color` - Color within palette (0-3)
#[inline]
pub const fn gbc_bg_index(palette: u8, color: u8) -> u8 {
    PAL_GBC_BG_START + palette * 4 + color
}

/// Calculate VGA palette index for a GBC sprite pixel
///
/// # Arguments
/// * `palette` - GBC palette number (0-7)
/// * `color` - Color within palette (0-3)
#[inline]
pub const fn gbc_sprite_index(palette: u8, color: u8) -> u8 {
    PAL_GBC_SPRITE_START + palette * 4 + color
}

/// Calculate VGA palette index for a DMG pixel
///
/// # Arguments
/// * `palette_type` - 0=BG, 1=OBJ0, 2=OBJ1
/// * `color` - Grayscale value (0-3)
#[inline]
pub const fn dmg_index(palette_type: u8, color: u8) -> u8 {
    PAL_DMG_START + palette_type * 4 + color
}
