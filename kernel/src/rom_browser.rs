//! ROM Browser - Select Game Boy ROMs from FAT32 storage
//!
//! Displays a list of available .gb/.gbc files and allows
//! the user to select which one to boot.

use crate::drivers::keyboard::{self, KeyCode};
use crate::storage::fat32;

// ============================================================================
// Constants
// ============================================================================

/// VGA Mode 13h parameters
const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 200;
const VGA_ADDR: *mut u8 = 0xA0000 as *mut u8;

/// UI Layout
const TITLE_Y: usize = 15;
const LIST_START_Y: usize = 45;
const LIST_ITEM_HEIGHT: usize = 12;
const LIST_X: usize = 40;
const MAX_VISIBLE_ITEMS: usize = 10;

/// Colors (VGA palette indices)
mod colors {
    pub const BLACK: u8 = 0x00;
    pub const DARK_GREEN: u8 = 0x02;
    pub const GREEN: u8 = 0x0A;
    pub const LIGHT_GREEN: u8 = 0x2A;  // GB screen green
    pub const DARK_GRAY: u8 = 0x08;
    pub const LIGHT_GRAY: u8 = 0x07;
    pub const WHITE: u8 = 0x0F;
    pub const HIGHLIGHT_BG: u8 = 0x02;  // Dark green background for selection
}

/// Simple 8x8 font (subset - uppercase, numbers, symbols)
/// Each character is 8 bytes, one per row, MSB is leftmost pixel
/// Characters: A-Z (0-25), 0-9 (26-35), space (36), . (37), : (38), / (39), - (40), > (41), ^ (42), v (43), _ (44)
#[rustfmt::skip]
static FONT_DATA: [u8; 45 * 8] = [
    // A
    0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x00,
    // B
    0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00,
    // C
    0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00,
    // D
    0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00,
    // E
    0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x7E, 0x00,
    // F
    0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x60, 0x00,
    // G
    0x3C, 0x66, 0x60, 0x6E, 0x66, 0x66, 0x3C, 0x00,
    // H
    0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00,
    // I
    0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00,
    // J
    0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38, 0x00,
    // K
    0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C, 0x66, 0x00,
    // L
    0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7E, 0x00,
    // M
    0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00,
    // N
    0x66, 0x76, 0x7E, 0x7E, 0x6E, 0x66, 0x66, 0x00,
    // O
    0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00,
    // P
    0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00,
    // Q
    0x3C, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x0E, 0x00,
    // R
    0x7C, 0x66, 0x66, 0x7C, 0x78, 0x6C, 0x66, 0x00,
    // S
    0x3C, 0x66, 0x60, 0x3C, 0x06, 0x66, 0x3C, 0x00,
    // T
    0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00,
    // U
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00,
    // V
    0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00,
    // W
    0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00,
    // X
    0x66, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x66, 0x00,
    // Y
    0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00,
    // Z
    0x7E, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x7E, 0x00,
    // 0
    0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00,
    // 1
    0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00,
    // 2
    0x3C, 0x66, 0x06, 0x0C, 0x30, 0x60, 0x7E, 0x00,
    // 3
    0x3C, 0x66, 0x06, 0x1C, 0x06, 0x66, 0x3C, 0x00,
    // 4
    0x06, 0x0E, 0x1E, 0x66, 0x7F, 0x06, 0x06, 0x00,
    // 5
    0x7E, 0x60, 0x7C, 0x06, 0x06, 0x66, 0x3C, 0x00,
    // 6
    0x3C, 0x66, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00,
    // 7
    0x7E, 0x66, 0x0C, 0x18, 0x18, 0x18, 0x18, 0x00,
    // 8
    0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00,
    // 9
    0x3C, 0x66, 0x66, 0x3E, 0x06, 0x66, 0x3C, 0x00,
    // Space (36)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // . (37)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00,
    // : (38)
    0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00,
    // / (39)
    0x02, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x40, 0x00,
    // - (40)
    0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00,
    // > (41)
    0x30, 0x18, 0x0C, 0x06, 0x0C, 0x18, 0x30, 0x00,
    // ^ (42)
    0x18, 0x3C, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
    // v (43) - down arrow
    0x00, 0x00, 0x00, 0x00, 0x66, 0x3C, 0x18, 0x00,
    // _ (44)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7E, 0x00,
];

// ============================================================================
// ROM Browser
// ============================================================================

pub struct RomBrowser {
    rom_count: usize,
    selected: usize,
    scroll_offset: usize,
}

impl RomBrowser {
    pub fn new() -> Self {
        // Debug: show mount status on row 195
        unsafe {
            let vga = 0xA0000 as *mut u8;
            // Check if mounted
            let is_mounted = fat32::is_mounted();
            core::ptr::write_volatile(vga.add(195 * 320), if is_mounted { 0x0A } else { 0x04 });
            core::ptr::write_volatile(vga.add(195 * 320 + 1), if is_mounted { 0x0A } else { 0x04 });
        }

        let rom_count = fat32::get_fs().count_roms();

        // Debug: show rom_count on row 195
        unsafe {
            let vga = 0xA0000 as *mut u8;
            core::ptr::write_volatile(vga.add(195 * 320 + 5), rom_count as u8);
            core::ptr::write_volatile(vga.add(195 * 320 + 6), rom_count as u8);
            // Green bar if count > 0, red if 0
            let color = if rom_count > 0 { 0x0A } else { 0x04 };
            for i in 10..30 {
                core::ptr::write_volatile(vga.add(195 * 320 + i), color);
            }
        }

        Self {
            rom_count,
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Run the browser UI loop
    /// Returns the selected ROM index, or None if no ROMs available
    pub fn run(&mut self) -> Option<usize> {
        if self.rom_count == 0 {
            self.draw_no_roms_screen();
            return None;
        }

        // Initial draw
        self.draw_screen();

        // Input loop
        loop {
            if let Some(key) = keyboard::get_key() {
                if !key.pressed {
                    continue;  // Only handle key press, not release
                }

                match key.keycode {
                    // Up arrow or W
                    KeyCode::Up | KeyCode::W => {
                        if self.selected > 0 {
                            self.selected -= 1;
                            self.adjust_scroll();
                            self.draw_list();
                        }
                    }
                    // Down arrow or S
                    KeyCode::Down | KeyCode::S => {
                        if self.selected < self.rom_count - 1 {
                            self.selected += 1;
                            self.adjust_scroll();
                            self.draw_list();
                        }
                    }
                    // Enter or Space
                    KeyCode::Enter | KeyCode::Space => {
                        return Some(self.selected);
                    }
                    _ => {}
                }
            }

            // Small delay to prevent busy-waiting
            for _ in 0..10000 {
                unsafe { core::arch::asm!("nop"); }
            }
        }
    }

    fn adjust_scroll(&mut self) {
        // Scroll up if selected is above visible area
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        // Scroll down if selected is below visible area
        if self.selected >= self.scroll_offset + MAX_VISIBLE_ITEMS {
            self.scroll_offset = self.selected - MAX_VISIBLE_ITEMS + 1;
        }
    }

    fn draw_screen(&self) {
        // Clear screen with dark color
        self.fill_screen(colors::BLACK);

        // Draw border
        self.draw_border();

        // Draw title
        self.draw_title();

        // Draw ROM count
        self.draw_rom_count();

        // Draw ROM list
        self.draw_list();

        // Draw instructions
        self.draw_instructions();
    }

    fn draw_no_roms_screen(&self) {
        self.fill_screen(colors::BLACK);
        self.draw_border();
        self.draw_title();

        // "NO ROMS FOUND" message
        self.draw_string_centered(90, "NO ROMS FOUND", colors::WHITE);
        self.draw_string_centered(110, "ADD .GB FILES TO", colors::LIGHT_GRAY);
        self.draw_string_centered(125, "YOUR HARD DRIVE", colors::LIGHT_GRAY);

        // Debug: show mount status AFTER screen is drawn
        unsafe {
            let vga = 0xA0000 as *mut u8;
            // Row 195: mounted status
            let is_mounted = fat32::is_mounted();
            let mount_color = if is_mounted { 0x0A } else { 0x04 };
            for i in 0..20 {
                core::ptr::write_volatile(vga.add(195 * 320 + i), mount_color);
            }

            // Row 196: rom_count value (show as bar length)
            for i in 0..self.rom_count.min(50) {
                core::ptr::write_volatile(vga.add(196 * 320 + i), 0x0A);
            }
            // If 0, show red
            if self.rom_count == 0 {
                for i in 0..20 {
                    core::ptr::write_volatile(vga.add(196 * 320 + i), 0x04);
                }
            }
        }

        // Halt here
        loop {
            unsafe { core::arch::asm!("hlt"); }
        }
    }

    fn draw_border(&self) {
        const BORDER_X: usize = 20;
        const BORDER_Y: usize = 10;
        const BORDER_W: usize = 280;
        const BORDER_H: usize = 180;
        const THICKNESS: usize = 3;

        // Top
        self.fill_rect(BORDER_X, BORDER_Y, BORDER_W, THICKNESS, colors::DARK_GRAY);
        // Bottom
        self.fill_rect(BORDER_X, BORDER_Y + BORDER_H - THICKNESS, BORDER_W, THICKNESS, colors::DARK_GRAY);
        // Left
        self.fill_rect(BORDER_X, BORDER_Y, THICKNESS, BORDER_H, colors::DARK_GRAY);
        // Right
        self.fill_rect(BORDER_X + BORDER_W - THICKNESS, BORDER_Y, THICKNESS, BORDER_H, colors::DARK_GRAY);
    }

    fn draw_title(&self) {
        self.draw_string_centered(TITLE_Y, "GB-OS", colors::GREEN);
        self.draw_string_centered(TITLE_Y + 12, "ROM SELECTOR", colors::LIGHT_GRAY);
    }

    fn draw_rom_count(&self) {
        let mut buf = [0u8; 20];
        let count_str = self.format_count(&mut buf);
        self.draw_string_centered(LIST_START_Y - 12, count_str, colors::DARK_GRAY);
    }

    fn format_count<'a>(&self, buf: &'a mut [u8; 20]) -> &'a str {
        // Format "X ROMS FOUND"
        let mut pos = 0;

        // Number
        if self.rom_count >= 10 {
            buf[pos] = b'0' + (self.rom_count / 10) as u8;
            pos += 1;
        }
        buf[pos] = b'0' + (self.rom_count % 10) as u8;
        pos += 1;

        // " ROMS FOUND"
        let suffix = b" ROMS FOUND";
        for &c in suffix {
            buf[pos] = c;
            pos += 1;
        }

        // Safe because we only use ASCII
        unsafe { core::str::from_utf8_unchecked(&buf[..pos]) }
    }

    fn draw_list(&self) {
        // Clear list area
        self.fill_rect(LIST_X - 5, LIST_START_Y - 2,
                       240, MAX_VISIBLE_ITEMS as usize * LIST_ITEM_HEIGHT + 4,
                       colors::BLACK);

        let visible_count = (self.rom_count - self.scroll_offset).min(MAX_VISIBLE_ITEMS);

        for i in 0..visible_count {
            let rom_index = self.scroll_offset + i;
            let y = LIST_START_Y + i * LIST_ITEM_HEIGHT;
            let is_selected = rom_index == self.selected;

            // Draw selection highlight
            if is_selected {
                self.fill_rect(LIST_X - 4, y - 1, 232, LIST_ITEM_HEIGHT, colors::HIGHLIGHT_BG);
            }

            // Get ROM name
            let mut name_buf = [0u8; 12];
            if fat32::get_fs().get_rom_name(rom_index, &mut name_buf) {
                // Convert to string (stop at null or end)
                let name_len = name_buf.iter().position(|&c| c == 0).unwrap_or(12);
                let name = unsafe { core::str::from_utf8_unchecked(&name_buf[..name_len]) };

                // Draw selector arrow
                if is_selected {
                    self.draw_char(LIST_X, y, b'>', colors::WHITE);
                }

                // Draw filename
                let text_color = if is_selected { colors::WHITE } else { colors::LIGHT_GRAY };
                self.draw_string(LIST_X + 12, y, name, text_color);
            }
        }

        // Draw scroll indicators if needed
        if self.scroll_offset > 0 {
            self.draw_char(SCREEN_WIDTH - 35, LIST_START_Y, b'^', colors::LIGHT_GRAY);
        }
        if self.scroll_offset + MAX_VISIBLE_ITEMS < self.rom_count {
            let y = LIST_START_Y + (MAX_VISIBLE_ITEMS - 1) * LIST_ITEM_HEIGHT;
            self.draw_char(SCREEN_WIDTH - 35, y, b'V', colors::LIGHT_GRAY);
        }
    }

    fn draw_instructions(&self) {
        self.draw_string_centered(175, "UP/DOWN:SELECT  ENTER:BOOT", colors::DARK_GRAY);
    }

    // ========================================================================
    // Drawing Primitives
    // ========================================================================

    fn fill_screen(&self, color: u8) {
        unsafe {
            // Only fill rows 0-194, preserve rows 195-199 for debug
            for i in 0..(SCREEN_WIDTH * 195) {
                core::ptr::write_volatile(VGA_ADDR.add(i), color);
            }
        }
    }

    fn fill_rect(&self, x: usize, y: usize, w: usize, h: usize, color: u8) {
        unsafe {
            for row in 0..h {
                let py = y + row;
                if py >= SCREEN_HEIGHT { break; }
                for col in 0..w {
                    let px = x + col;
                    if px >= SCREEN_WIDTH { break; }
                    let offset = py * SCREEN_WIDTH + px;
                    core::ptr::write_volatile(VGA_ADDR.add(offset), color);
                }
            }
        }
    }

    fn draw_char(&self, x: usize, y: usize, ch: u8, color: u8) {
        // Map character to font index
        let index = match ch {
            b'A'..=b'Z' => (ch - b'A') as usize,
            b'0'..=b'9' => (ch - b'0' + 26) as usize,
            b' ' => 36,
            b'.' => 37,
            b':' => 38,
            b'/' => 39,
            b'-' => 40,
            b'>' => 41,
            b'^' => 42,
            b'V' => 43,  // Down arrow (uppercase V)
            b'_' => 44,
            // Map lowercase to uppercase (except special chars handled above)
            b'a'..=b'z' => (ch - b'a') as usize,
            _ => 36,  // Default to space
        };

        // Get font bitmap for this character
        let start = index * 8;
        if start + 8 <= FONT_DATA.len() {
            self.draw_char_bitmap(x, y, &FONT_DATA[start..start + 8], color);
        }
    }

    fn draw_char_bitmap(&self, x: usize, y: usize, bitmap: &[u8], color: u8) {
        unsafe {
            for row in 0..8 {
                let py = y + row;
                if py >= SCREEN_HEIGHT { continue; }
                let bits = bitmap[row];
                for col in 0..8 {
                    if (bits >> (7 - col)) & 1 != 0 {
                        let px = x + col;
                        if px >= SCREEN_WIDTH { continue; }
                        let offset = py * SCREEN_WIDTH + px;
                        core::ptr::write_volatile(VGA_ADDR.add(offset), color);
                    }
                }
            }
        }
    }

    fn draw_string(&self, x: usize, y: usize, s: &str, color: u8) {
        let mut cx = x;
        for ch in s.bytes() {
            self.draw_char(cx, y, ch, color);
            cx += 8;  // Fixed-width font
        }
    }

    fn draw_string_centered(&self, y: usize, s: &str, color: u8) {
        let width = s.len() * 8;
        let x = (SCREEN_WIDTH - width) / 2;
        self.draw_string(x, y, s, color);
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Show ROM browser and return selected ROM index
/// Returns None if no ROMs found
pub fn select_rom() -> Option<usize> {
    let mut browser = RomBrowser::new();
    browser.run()
}
