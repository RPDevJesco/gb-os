//! ROM Browser - Select Game Boy ROMs from FAT32 storage
//!
//! Displays a list of available .gb/.gbc files and allows
//! the user to select which one to boot.

use crate::drivers::keyboard::{self, KeyCode};
use crate::graphics::vga_mode13h::{self, colors, SCREEN_WIDTH};
use crate::gui::font_8x8;
use crate::storage::fat32;

// ============================================================================
// UI Layout Constants
// ============================================================================

const TITLE_Y: usize = 15;
const LIST_START_Y: usize = 45;
const LIST_ITEM_HEIGHT: usize = 12;
const LIST_X: usize = 40;
const MAX_VISIBLE_ITEMS: usize = 10;

/// Border dimensions
const BORDER_X: usize = 20;
const BORDER_Y: usize = 10;
const BORDER_W: usize = 280;
const BORDER_H: usize = 180;
const BORDER_THICKNESS: usize = 3;

/// Debug rows preserved at bottom of screen
const DEBUG_ROWS: usize = 5;

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
            let vga = vga_mode13h::VGA_ADDR;
            let is_mounted = fat32::is_mounted();
            core::ptr::write_volatile(vga.add(195 * 320), if is_mounted { 0x0A } else { 0x04 });
            core::ptr::write_volatile(
                vga.add(195 * 320 + 1),
                if is_mounted { 0x0A } else { 0x04 },
            );
        }

        let rom_count = fat32::get_fs().count_roms();

        // Debug: show rom_count on row 195
        unsafe {
            let vga = vga_mode13h::VGA_ADDR;
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
                    continue; // Only handle key press, not release
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
                unsafe {
                    core::arch::asm!("nop");
                }
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

    #[inline(never)]
    fn draw_screen(&self) {
        // Clear screen (preserve debug rows)
        vga_mode13h::fill_screen_partial(colors::BLACK, DEBUG_ROWS);

        self.draw_border();
        self.draw_title();
        self.draw_rom_count();
        self.draw_list();
        self.draw_instructions();
    }

    #[inline(never)]
    fn draw_no_roms_screen(&self) {
        // Clear screen (preserve debug rows)
        vga_mode13h::fill_screen_partial(colors::BLACK, DEBUG_ROWS);

        self.draw_border();
        self.draw_title();

        font_8x8::draw_string_centered_vga(90, "NO ROMS FOUND", colors::WHITE);
        font_8x8::draw_string_centered_vga(110, "ADD .GB FILES TO", colors::LIGHT_GRAY);
        font_8x8::draw_string_centered_vga(125, "YOUR HARD DRIVE", colors::LIGHT_GRAY);

        // Debug: show mount status AFTER screen is drawn
        unsafe {
            let vga = vga_mode13h::VGA_ADDR;
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
            unsafe {
                core::arch::asm!("hlt");
            }
        }
    }

    #[inline(never)]
    fn draw_border(&self) {
        vga_mode13h::draw_thick_border(
            BORDER_X,
            BORDER_Y,
            BORDER_W,
            BORDER_H,
            BORDER_THICKNESS,
            colors::DARK_GRAY,
        );
    }

    #[inline(never)]
    fn draw_title(&self) {
        font_8x8::draw_string_centered_vga(TITLE_Y, "GB-OS", colors::GREEN);
        font_8x8::draw_string_centered_vga(TITLE_Y + 12, "ROM SELECTOR", colors::LIGHT_GRAY);
    }

    #[inline(never)]
    fn draw_rom_count(&self) {
        let mut buf = [0u8; 20];
        let count_str = self.format_count(&mut buf);
        font_8x8::draw_string_centered_vga(LIST_START_Y - 12, count_str, colors::DARK_GRAY);
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

    #[inline(never)]
    fn draw_list(&self) {
        // Clear list area
        vga_mode13h::fill_rect(
            LIST_X - 5,
            LIST_START_Y - 2,
            240,
            MAX_VISIBLE_ITEMS * LIST_ITEM_HEIGHT + 4,
            colors::BLACK,
        );

        // Compiler fence - force synchronization
        unsafe {
            core::ptr::write_volatile(0xA0000 as *mut u8, 0x00);
        }

        let visible_count = (self.rom_count - self.scroll_offset).min(MAX_VISIBLE_ITEMS);

        for i in 0..visible_count {
            let rom_index = self.scroll_offset + i;
            let y = LIST_START_Y + i * LIST_ITEM_HEIGHT;
            let is_selected = rom_index == self.selected;

            // Draw selection highlight
            if is_selected {
                vga_mode13h::fill_rect(
                    LIST_X - 4,
                    y - 1,
                    232,
                    LIST_ITEM_HEIGHT,
                    colors::HIGHLIGHT_BG,
                );
            }

            // Get ROM name
            let mut name_buf = [0u8; 12];
            if fat32::get_fs().get_rom_name(rom_index, &mut name_buf) {
                // Convert to string (stop at null or end)
                let name_len = name_buf.iter().position(|&c| c == 0).unwrap_or(12);
                let name = unsafe { core::str::from_utf8_unchecked(&name_buf[..name_len]) };

                // Draw selector arrow
                if is_selected {
                    font_8x8::draw_char_vga(LIST_X, y, b'>', colors::WHITE);
                }

                // Draw filename
                let text_color = if is_selected {
                    colors::WHITE
                } else {
                    colors::LIGHT_GRAY
                };
                font_8x8::draw_string_vga(LIST_X + 12, y, name, text_color);
            }
        }

        // Draw scroll indicators if needed
        if self.scroll_offset > 0 {
            font_8x8::draw_char_vga(SCREEN_WIDTH - 35, LIST_START_Y, b'^', colors::LIGHT_GRAY);
        }
        if self.scroll_offset + MAX_VISIBLE_ITEMS < self.rom_count {
            let y = LIST_START_Y + (MAX_VISIBLE_ITEMS - 1) * LIST_ITEM_HEIGHT;
            font_8x8::draw_char_vga(SCREEN_WIDTH - 35, y, font_8x8::DOWN_ARROW, colors::LIGHT_GRAY);
        }
    }

    fn draw_instructions(&self) {
        font_8x8::draw_string_centered_vga(175, "UP/DOWN:SELECT  ENTER:BOOT", colors::DARK_GRAY);
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
