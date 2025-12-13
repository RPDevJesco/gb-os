//! ROM Selection Menu
//!
//! Displays a list of ROMs from the FAT16 partition and lets the user select one.
//! Uses VGA mode 13h for display.

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use crate::fs::fat16::RomFile;
use crate::drivers::keyboard::{self, KeyCode};

/// Menu colors (VGA palette indices)
const COLOR_BG: u8 = 0;           // Black
const COLOR_BORDER: u8 = 7;       // Light gray
const COLOR_TITLE: u8 = 15;       // White
const COLOR_NORMAL: u8 = 7;       // Light gray
const COLOR_SELECTED: u8 = 14;    // Yellow
const COLOR_HIGHLIGHT_BG: u8 = 1; // Blue
const COLOR_INSTRUCTIONS: u8 = 8; // Dark gray

/// VGA framebuffer
const VGA_BUFFER: *mut u8 = 0xA0000 as *mut u8;
const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 200;

/// Menu configuration
const MENU_X: usize = 40;
const MENU_Y: usize = 30;
const MENU_WIDTH: usize = 240;
const MENU_HEIGHT: usize = 140;
const MENU_PADDING: usize = 8;
const LINE_HEIGHT: usize = 10;
const MAX_VISIBLE_ITEMS: usize = 10;

/// 8x8 bitmap font (simplified - just uppercase letters, numbers, and basic symbols)
/// Each character is 8 bytes, one per row
static FONT_DATA: &[u8] = include_bytes!("font8x8.bin");

/// ROM selection menu
pub struct RomMenu {
    roms: Vec<RomFile>,
    selected: usize,
    scroll_offset: usize,
}

impl RomMenu {
    /// Create a new ROM menu
    pub fn new(roms: Vec<RomFile>) -> Self {
        Self {
            roms,
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Run the menu and return the selected ROM index
    pub fn run(&mut self) -> Option<usize> {
        if self.roms.is_empty() {
            self.show_no_roms_message();
            return None;
        }

        // Clear screen
        self.clear_screen(COLOR_BG);

        loop {
            self.draw();

            // Wait for key press
            if let Some(key) = self.wait_for_key() {
                match key {
                    KeyCode::Up => {
                        if self.selected > 0 {
                            self.selected -= 1;
                            if self.selected < self.scroll_offset {
                                self.scroll_offset = self.selected;
                            }
                        }
                    }
                    KeyCode::Down => {
                        if self.selected < self.roms.len() - 1 {
                            self.selected += 1;
                            if self.selected >= self.scroll_offset + MAX_VISIBLE_ITEMS {
                                self.scroll_offset = self.selected - MAX_VISIBLE_ITEMS + 1;
                            }
                        }
                    }
                    KeyCode::Enter => {
                        return Some(self.selected);
                    }
                    KeyCode::Escape => {
                        return None;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Draw the menu
    fn draw(&self) {
        // Draw border
        self.draw_box(MENU_X, MENU_Y, MENU_WIDTH, MENU_HEIGHT, COLOR_BORDER);

        // Draw title
        let title = "SELECT ROM";
        let title_x = MENU_X + (MENU_WIDTH - title.len() * 8) / 2;
        self.draw_string(title_x, MENU_Y + 4, title, COLOR_TITLE);

        // Draw separator line
        self.draw_hline(MENU_X + 4, MENU_Y + 16, MENU_WIDTH - 8, COLOR_BORDER);

        // Draw ROM list
        let list_y = MENU_Y + 22;
        let visible_count = core::cmp::min(MAX_VISIBLE_ITEMS, self.roms.len());

        for i in 0..visible_count {
            let rom_idx = self.scroll_offset + i;
            if rom_idx >= self.roms.len() {
                break;
            }

            let y = list_y + i * LINE_HEIGHT;
            let is_selected = rom_idx == self.selected;

            // Draw selection highlight
            if is_selected {
                self.fill_rect(
                    MENU_X + 4,
                    y,
                    MENU_WIDTH - 8,
                    LINE_HEIGHT,
                    COLOR_HIGHLIGHT_BG
                );
            }

            // Draw ROM name
            let rom = &self.roms[rom_idx];
            let display_name = self.truncate_name(&rom.name, 28);
            let color = if is_selected { COLOR_SELECTED } else { COLOR_NORMAL };

            // Draw selection marker
            if is_selected {
                self.draw_string(MENU_X + 6, y + 1, ">", color);
            }

            self.draw_string(MENU_X + 16, y + 1, &display_name, color);

            // Draw file size on the right
            let size_str = self.format_size(rom.size);
            let size_x = MENU_X + MENU_WIDTH - 8 - size_str.len() * 8;
            self.draw_string(size_x, y + 1, &size_str, COLOR_INSTRUCTIONS);
        }

        // Draw scroll indicators if needed
        if self.scroll_offset > 0 {
            self.draw_string(MENU_X + MENU_WIDTH - 16, list_y - 2, "^", COLOR_BORDER);
        }
        if self.scroll_offset + MAX_VISIBLE_ITEMS < self.roms.len() {
            let y = list_y + MAX_VISIBLE_ITEMS * LINE_HEIGHT;
            self.draw_string(MENU_X + MENU_WIDTH - 16, y, "v", COLOR_BORDER);
        }

        // Draw instructions
        let instructions = "UP/DOWN:Select ENTER:Play ESC:Cancel";
        let inst_y = MENU_Y + MENU_HEIGHT - 12;
        self.draw_string(MENU_X + 8, inst_y, instructions, COLOR_INSTRUCTIONS);

        // Draw ROM count
        let count_str = self.format_count();
        self.draw_string(MENU_X + 8, MENU_Y + MENU_HEIGHT + 4, &count_str, COLOR_INSTRUCTIONS);
    }

    /// Show message when no ROMs found
    fn show_no_roms_message(&self) {
        self.clear_screen(COLOR_BG);
        self.draw_box(MENU_X, MENU_Y, MENU_WIDTH, 60, COLOR_BORDER);

        let msg1 = "NO ROMS FOUND";
        let msg2 = "Add .GB or .GBC files";
        let msg3 = "to the ROMS partition";

        self.draw_string(MENU_X + (MENU_WIDTH - msg1.len() * 8) / 2, MENU_Y + 12, msg1, COLOR_SELECTED);
        self.draw_string(MENU_X + (MENU_WIDTH - msg2.len() * 8) / 2, MENU_Y + 28, msg2, COLOR_NORMAL);
        self.draw_string(MENU_X + (MENU_WIDTH - msg3.len() * 8) / 2, MENU_Y + 40, msg3, COLOR_NORMAL);

        // Wait for any key
        self.wait_for_key();
    }

    /// Truncate name to fit display
    fn truncate_name(&self, name: &str, max_len: usize) -> String {
        if name.len() <= max_len {
            String::from(name)
        } else {
            let mut s = String::from(&name[..max_len - 3]);
            s.push_str("...");
            s
        }
    }

    /// Format file size for display
    fn format_size(&self, size: u32) -> String {
        if size >= 1024 * 1024 {
            let mb = size / (1024 * 1024);
            let mut s = String::new();
            self.write_num(&mut s, mb);
            s.push_str("MB");
            s
        } else if size >= 1024 {
            let kb = size / 1024;
            let mut s = String::new();
            self.write_num(&mut s, kb);
            s.push_str("KB");
            s
        } else {
            let mut s = String::new();
            self.write_num(&mut s, size);
            s.push('B');
            s
        }
    }

    /// Format ROM count
    fn format_count(&self) -> String {
        let mut s = String::new();
        self.write_num(&mut s, self.roms.len() as u32);
        s.push_str(" ROM");
        if self.roms.len() != 1 {
            s.push('S');
        }
        s.push_str(" FOUND");
        s
    }

    /// Write number to string
    fn write_num(&self, s: &mut String, mut n: u32) {
        if n == 0 {
            s.push('0');
            return;
        }

        let mut digits = [0u8; 10];
        let mut i = 0;
        while n > 0 {
            digits[i] = (n % 10) as u8;
            n /= 10;
            i += 1;
        }

        while i > 0 {
            i -= 1;
            s.push((b'0' + digits[i]) as char);
        }
    }

    /// Wait for a key press
    fn wait_for_key(&self) -> Option<KeyCode> {
        // Clear any pending keys
        while keyboard::get_key().is_some() {}

        loop {
            if let Some(key) = keyboard::get_key() {
                // Only return on key press, not release
                if key.pressed {
                    return Some(key.keycode);
                }
            }

            // Small delay to avoid busy spinning
            for _ in 0..10000 {
                unsafe { core::arch::asm!("pause"); }
            }
        }
    }

    /// Clear screen with color
    fn clear_screen(&self, color: u8) {
        unsafe {
            for i in 0..(SCREEN_WIDTH * SCREEN_HEIGHT) {
                *VGA_BUFFER.add(i) = color;
            }
        }
    }

    /// Draw a box outline
    fn draw_box(&self, x: usize, y: usize, width: usize, height: usize, color: u8) {
        // Top and bottom
        self.draw_hline(x, y, width, color);
        self.draw_hline(x, y + height - 1, width, color);

        // Left and right
        self.draw_vline(x, y, height, color);
        self.draw_vline(x + width - 1, y, height, color);
    }

    /// Draw horizontal line
    fn draw_hline(&self, x: usize, y: usize, width: usize, color: u8) {
        if y >= SCREEN_HEIGHT {
            return;
        }
        unsafe {
            for i in 0..width {
                if x + i < SCREEN_WIDTH {
                    *VGA_BUFFER.add(y * SCREEN_WIDTH + x + i) = color;
                }
            }
        }
    }

    /// Draw vertical line
    fn draw_vline(&self, x: usize, y: usize, height: usize, color: u8) {
        if x >= SCREEN_WIDTH {
            return;
        }
        unsafe {
            for i in 0..height {
                if y + i < SCREEN_HEIGHT {
                    *VGA_BUFFER.add((y + i) * SCREEN_WIDTH + x) = color;
                }
            }
        }
    }

    /// Fill rectangle
    fn fill_rect(&self, x: usize, y: usize, width: usize, height: usize, color: u8) {
        for row in 0..height {
            if y + row < SCREEN_HEIGHT {
                self.draw_hline(x, y + row, width, color);
            }
        }
    }

    /// Draw a character at position
    fn draw_char(&self, x: usize, y: usize, c: char, color: u8) {
        let idx = c as usize;

        // Only handle printable ASCII
        if idx < 32 || idx > 127 {
            return;
        }

        let font_idx = (idx - 32) * 8;

        // Check if we have font data for this character
        if font_idx + 8 > FONT_DATA.len() {
            return;
        }

        unsafe {
            for row in 0..8 {
                if y + row >= SCREEN_HEIGHT {
                    break;
                }

                let byte = FONT_DATA[font_idx + row];
                for col in 0..8 {
                    if x + col >= SCREEN_WIDTH {
                        break;
                    }

                    if byte & (0x80 >> col) != 0 {
                        *VGA_BUFFER.add((y + row) * SCREEN_WIDTH + x + col) = color;
                    }
                }
            }
        }
    }

    /// Draw a string at position
    fn draw_string(&self, x: usize, y: usize, s: &str, color: u8) {
        let mut cx = x;
        for c in s.chars() {
            if cx + 8 > SCREEN_WIDTH {
                break;
            }
            self.draw_char(cx, y, c.to_ascii_uppercase(), color);
            cx += 8;
        }
    }
}

/// Show ROM selection menu and return selected ROM
pub fn show_rom_menu(roms: Vec<RomFile>) -> Option<usize> {
    let mut menu = RomMenu::new(roms);
    menu.run()
}
