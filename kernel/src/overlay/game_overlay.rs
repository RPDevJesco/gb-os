//! Game Overlay Rendering
//!
//! Renders game state overlay directly to VGA mode 13h framebuffer.
//! Uses embedded bitmap font and simple primitives - no external dependencies.
//!
//! # Display Layout (320x200 mode 13h)
//!
//! The Game Boy screen is 160x144 centered at x=80. This leaves
//! 80-pixel margins on each side for overlay information.
//!
//! ```text
//! +------+------------------+------+
//! |      |                  |      |
//! | Left |   Game Screen    | Right|
//! | Info |     160x144      | Info |
//! |      |   (at x=80)      |      |
//! +------+------------------+------+
//! ```

use crate::overlay::ram_layout::{Game, RamReader, Pokemon, TrainerData, PartyState, decode_text, StatusCondition};

// =============================================================================
// VGA Mode 13h Palette Indices
// =============================================================================

/// Standard VGA palette colors for overlay
pub mod colors {
    pub const BLACK: u8 = 0;
    pub const BLUE: u8 = 1;
    pub const GREEN: u8 = 2;
    pub const CYAN: u8 = 3;
    pub const RED: u8 = 4;
    pub const MAGENTA: u8 = 5;
    pub const BROWN: u8 = 6;
    pub const LIGHT_GRAY: u8 = 7;
    pub const DARK_GRAY: u8 = 8;
    pub const LIGHT_BLUE: u8 = 9;
    pub const LIGHT_GREEN: u8 = 10;
    pub const LIGHT_CYAN: u8 = 11;
    pub const LIGHT_RED: u8 = 12;
    pub const LIGHT_MAGENTA: u8 = 13;
    pub const YELLOW: u8 = 14;
    pub const WHITE: u8 = 15;

    // HP bar colors
    pub const HP_GREEN: u8 = 10;    // > 50%
    pub const HP_YELLOW: u8 = 14;   // 25-50%
    pub const HP_RED: u8 = 4;       // < 25%
    pub const HP_BG: u8 = 8;        // Background

    // Overlay background
    pub const OVERLAY_BG: u8 = 0;
    pub const OVERLAY_BORDER: u8 = 7;
    pub const OVERLAY_TEXT: u8 = 15;
    pub const OVERLAY_TEXT_DIM: u8 = 7;
}

// =============================================================================
// 8x8 Bitmap Font (CP437-style, partial)
// =============================================================================

/// 8x8 bitmap font data
/// Each character is 8 bytes, one per row, MSB is leftmost pixel
/// Covers ASCII 32-95 (space through underscore)
static FONT_8X8: [u8; 512] = [
    // Space (32)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // ! (33)
    0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x00,
    // " (34)
    0x6C, 0x6C, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00,
    // # (35)
    0x6C, 0x6C, 0xFE, 0x6C, 0xFE, 0x6C, 0x6C, 0x00,
    // $ (36)
    0x18, 0x3E, 0x60, 0x3C, 0x06, 0x7C, 0x18, 0x00,
    // % (37)
    0x00, 0x66, 0xAC, 0xD8, 0x36, 0x6A, 0xCC, 0x00,
    // & (38)
    0x38, 0x6C, 0x68, 0x76, 0xDC, 0xCC, 0x76, 0x00,
    // ' (39)
    0x18, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00,
    // ( (40)
    0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00,
    // ) (41)
    0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00,
    // * (42)
    0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00,
    // + (43)
    0x00, 0x18, 0x18, 0x7E, 0x18, 0x18, 0x00, 0x00,
    // , (44)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30,
    // - (45)
    0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00,
    // . (46)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00,
    // / (47)
    0x06, 0x0C, 0x18, 0x30, 0x60, 0xC0, 0x80, 0x00,
    // 0 (48)
    0x7C, 0xC6, 0xCE, 0xD6, 0xE6, 0xC6, 0x7C, 0x00,
    // 1 (49)
    0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00,
    // 2 (50)
    0x7C, 0xC6, 0x06, 0x1C, 0x70, 0xC6, 0xFE, 0x00,
    // 3 (51)
    0x7C, 0xC6, 0x06, 0x3C, 0x06, 0xC6, 0x7C, 0x00,
    // 4 (52)
    0x1C, 0x3C, 0x6C, 0xCC, 0xFE, 0x0C, 0x1E, 0x00,
    // 5 (53)
    0xFE, 0xC0, 0xFC, 0x06, 0x06, 0xC6, 0x7C, 0x00,
    // 6 (54)
    0x38, 0x60, 0xC0, 0xFC, 0xC6, 0xC6, 0x7C, 0x00,
    // 7 (55)
    0xFE, 0xC6, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x00,
    // 8 (56)
    0x7C, 0xC6, 0xC6, 0x7C, 0xC6, 0xC6, 0x7C, 0x00,
    // 9 (57)
    0x7C, 0xC6, 0xC6, 0x7E, 0x06, 0x0C, 0x78, 0x00,
    // : (58)
    0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00,
    // ; (59)
    0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x30,
    // < (60)
    0x0C, 0x18, 0x30, 0x60, 0x30, 0x18, 0x0C, 0x00,
    // = (61)
    0x00, 0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00,
    // > (62)
    0x30, 0x18, 0x0C, 0x06, 0x0C, 0x18, 0x30, 0x00,
    // ? (63)
    0x7C, 0xC6, 0x0C, 0x18, 0x18, 0x00, 0x18, 0x00,
    // @ (64)
    0x7C, 0xC6, 0xDE, 0xDE, 0xDE, 0xC0, 0x7C, 0x00,
    // A (65)
    0x38, 0x6C, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0x00,
    // B (66)
    0xFC, 0xC6, 0xC6, 0xFC, 0xC6, 0xC6, 0xFC, 0x00,
    // C (67)
    0x7C, 0xC6, 0xC0, 0xC0, 0xC0, 0xC6, 0x7C, 0x00,
    // D (68)
    0xF8, 0xCC, 0xC6, 0xC6, 0xC6, 0xCC, 0xF8, 0x00,
    // E (69)
    0xFE, 0xC0, 0xC0, 0xF8, 0xC0, 0xC0, 0xFE, 0x00,
    // F (70)
    0xFE, 0xC0, 0xC0, 0xF8, 0xC0, 0xC0, 0xC0, 0x00,
    // G (71)
    0x7C, 0xC6, 0xC0, 0xCE, 0xC6, 0xC6, 0x7E, 0x00,
    // H (72)
    0xC6, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0xC6, 0x00,
    // I (73)
    0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00,
    // J (74)
    0x1E, 0x06, 0x06, 0x06, 0xC6, 0xC6, 0x7C, 0x00,
    // K (75)
    0xC6, 0xCC, 0xD8, 0xF0, 0xD8, 0xCC, 0xC6, 0x00,
    // L (76)
    0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xFE, 0x00,
    // M (77)
    0xC6, 0xEE, 0xFE, 0xFE, 0xD6, 0xC6, 0xC6, 0x00,
    // N (78)
    0xC6, 0xE6, 0xF6, 0xDE, 0xCE, 0xC6, 0xC6, 0x00,
    // O (79)
    0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0x00,
    // P (80)
    0xFC, 0xC6, 0xC6, 0xFC, 0xC0, 0xC0, 0xC0, 0x00,
    // Q (81)
    0x7C, 0xC6, 0xC6, 0xC6, 0xD6, 0xDE, 0x7C, 0x06,
    // R (82)
    0xFC, 0xC6, 0xC6, 0xFC, 0xD8, 0xCC, 0xC6, 0x00,
    // S (83)
    0x7C, 0xC6, 0x60, 0x38, 0x0C, 0xC6, 0x7C, 0x00,
    // T (84)
    0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00,
    // U (85)
    0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0x00,
    // V (86)
    0xC6, 0xC6, 0xC6, 0xC6, 0x6C, 0x38, 0x10, 0x00,
    // W (87)
    0xC6, 0xC6, 0xD6, 0xFE, 0xFE, 0xEE, 0xC6, 0x00,
    // X (88)
    0xC6, 0x6C, 0x38, 0x38, 0x6C, 0xC6, 0xC6, 0x00,
    // Y (89)
    0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00,
    // Z (90)
    0xFE, 0x06, 0x0C, 0x18, 0x30, 0x60, 0xFE, 0x00,
    // [ (91)
    0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00,
    // \ (92)
    0xC0, 0x60, 0x30, 0x18, 0x0C, 0x06, 0x02, 0x00,
    // ] (93)
    0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00,
    // ^ (94)
    0x10, 0x38, 0x6C, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // _ (95)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFE,
];

/// Get font data for a character (returns 8 bytes)
fn get_char_bitmap(c: u8) -> &'static [u8] {
    let index = if c >= 32 && c < 96 {
        (c - 32) as usize
    } else if c >= b'a' && c <= b'z' {
        // Map lowercase to uppercase
        (c - b'a' + b'A' - 32) as usize
    } else {
        0 // Space for unknown
    };

    let start = index * 8;
    if start + 8 <= FONT_8X8.len() {
        &FONT_8X8[start..start + 8]
    } else {
        &FONT_8X8[0..8] // Fallback to space
    }
}

// =============================================================================
// Overlay Configuration
// =============================================================================

/// Screen dimensions for mode 13h
pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 200;

/// Game Boy screen dimensions
pub const GB_WIDTH: usize = 160;
pub const GB_HEIGHT: usize = 144;

/// Overlay region configuration
#[derive(Clone, Copy)]
pub struct OverlayConfig {
    /// X offset where overlay sidebar starts
    pub sidebar_x: usize,
    /// Width of sidebar
    pub sidebar_width: usize,
    /// Whether to show party info
    pub show_party: bool,
    /// Whether to show trainer info
    pub show_trainer: bool,
    /// Whether to show badge count
    pub show_badges: bool,
    /// Whether to show play time
    pub show_playtime: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            // Game is centered: starts at x=80, ends at x=240
            // Sidebar goes in the right margin: x=240 to x=320
            sidebar_x: 240,
            sidebar_width: 80, // 320 - 240 = 80 pixels
            show_party: true,
            show_trainer: true,
            show_badges: true,
            show_playtime: true,
        }
    }
}

// =============================================================================
// Overlay Renderer
// =============================================================================

/// Renders game overlay to framebuffer
pub struct OverlayRenderer {
    config: OverlayConfig,
    game: Game,
}

impl OverlayRenderer {
    pub fn new(game: Game) -> Self {
        Self {
            config: OverlayConfig::default(),
            game,
        }
    }

    pub fn with_config(game: Game, config: OverlayConfig) -> Self {
        Self { config, game }
    }

    /// Draw a single character at (x, y)
    fn draw_char(&self, fb: &mut [u8], x: usize, y: usize, c: u8, color: u8) {
        if x + 8 > SCREEN_WIDTH || y + 8 > SCREEN_HEIGHT {
            return;
        }

        let bitmap = get_char_bitmap(c);
        for (row, &bits) in bitmap.iter().enumerate() {
            let py = y + row;
            for col in 0..8 {
                if bits & (0x80 >> col) != 0 {
                    let px = x + col;
                    let offset = py * SCREEN_WIDTH + px;
                    if offset < fb.len() {
                        fb[offset] = color;
                    }
                }
            }
        }
    }

    /// Draw a string at (x, y)
    fn draw_string(&self, fb: &mut [u8], x: usize, y: usize, s: &[u8], color: u8) {
        let mut cx = x;
        for &c in s {
            if c == 0 { break; }
            self.draw_char(fb, cx, y, c, color);
            cx += 8;
            if cx + 8 > SCREEN_WIDTH { break; }
        }
    }

    /// Draw a string from a static str
    fn draw_str(&self, fb: &mut [u8], x: usize, y: usize, s: &str, color: u8) {
        self.draw_string(fb, x, y, s.as_bytes(), color);
    }

    /// Draw a number (right-aligned within width characters)
    fn draw_number(&self, fb: &mut [u8], x: usize, y: usize, mut n: u32, width: usize, color: u8) {
        let mut buf = [b' '; 10];
        let mut i = buf.len();

        if n == 0 {
            i -= 1;
            buf[i] = b'0';
        } else {
            while n > 0 && i > 0 {
                i -= 1;
                buf[i] = b'0' + (n % 10) as u8;
                n /= 10;
            }
        }

        // Right align
        let digits = buf.len() - i;
        let start = if digits < width { width - digits } else { 0 };

        for (j, &c) in buf[i..].iter().enumerate() {
            self.draw_char(fb, x + (start + j) * 8, y, c, color);
        }
    }

    /// Draw a filled rectangle
    fn draw_rect(&self, fb: &mut [u8], x: usize, y: usize, w: usize, h: usize, color: u8) {
        for py in y..y.saturating_add(h).min(SCREEN_HEIGHT) {
            for px in x..x.saturating_add(w).min(SCREEN_WIDTH) {
                let offset = py * SCREEN_WIDTH + px;
                if offset < fb.len() {
                    fb[offset] = color;
                }
            }
        }
    }

    /// Draw a box outline
    fn draw_box(&self, fb: &mut [u8], x: usize, y: usize, w: usize, h: usize, color: u8) {
        // Top and bottom
        for px in x..x.saturating_add(w).min(SCREEN_WIDTH) {
            if y < SCREEN_HEIGHT {
                fb[y * SCREEN_WIDTH + px] = color;
            }
            let by = y + h - 1;
            if by < SCREEN_HEIGHT {
                fb[by * SCREEN_WIDTH + px] = color;
            }
        }
        // Left and right
        for py in y..y.saturating_add(h).min(SCREEN_HEIGHT) {
            fb[py * SCREEN_WIDTH + x] = color;
            let rx = x + w - 1;
            if rx < SCREEN_WIDTH {
                fb[py * SCREEN_WIDTH + rx] = color;
            }
        }
    }

    /// Draw an HP bar
    fn draw_hp_bar(&self, fb: &mut [u8], x: usize, y: usize, current: u16, max: u16, width: usize) {
        // Background
        self.draw_rect(fb, x, y, width, 4, colors::HP_BG);

        // Calculate fill
        if max == 0 { return; }
        let percent = (current as u32 * 100) / max as u32;
        let fill_width = ((current as usize * (width - 2)) / max as usize).min(width - 2);

        // Choose color based on HP percentage
        let color = if percent > 50 {
            colors::HP_GREEN
        } else if percent > 25 {
            colors::HP_YELLOW
        } else {
            colors::HP_RED
        };

        // Fill
        if fill_width > 0 {
            self.draw_rect(fb, x + 1, y + 1, fill_width, 2, color);
        }
    }

    /// Draw party Pokemon slot
    fn draw_party_slot(&self, fb: &mut [u8], x: usize, y: usize, slot: usize, pokemon: Option<&Pokemon>) {
        match pokemon {
            Some(mon) => {
                // Species number and level
                // Format: "#001 L05" or species name if we have it
                let mut line1 = [b' '; 12];
                line1[0] = b'#';
                line1[1] = b'0' + (mon.species / 100) % 10;
                line1[2] = b'0' + (mon.species / 10) % 10;
                line1[3] = b'0' + mon.species % 10;
                line1[4] = b' ';
                line1[5] = b'L';
                if mon.level >= 100 {
                    line1[6] = b'0' + (mon.level / 100) % 10;
                    line1[7] = b'0' + (mon.level / 10) % 10;
                    line1[8] = b'0' + mon.level % 10;
                } else if mon.level >= 10 {
                    line1[6] = b'0' + (mon.level / 10) % 10;
                    line1[7] = b'0' + mon.level % 10;
                } else {
                    line1[6] = b'0' + mon.level;
                }

                self.draw_string(fb, x, y, &line1, colors::OVERLAY_TEXT);

                // HP bar
                self.draw_hp_bar(fb, x, y + 9, mon.hp_current, mon.hp_max, 60);

                // HP numbers
                let mut hp_str = [b' '; 9];
                // Format: "123/456"
                let mut pos = 0;
                let mut hp = mon.hp_current;
                if hp >= 100 { hp_str[pos] = b'0' + (hp / 100 % 10) as u8; pos += 1; }
                if hp >= 10 { hp_str[pos] = b'0' + (hp / 10 % 10) as u8; pos += 1; }
                hp_str[pos] = b'0' + (hp % 10) as u8; pos += 1;
                hp_str[pos] = b'/'; pos += 1;
                hp = mon.hp_max;
                if hp >= 100 { hp_str[pos] = b'0' + (hp / 100 % 10) as u8; pos += 1; }
                if hp >= 10 { hp_str[pos] = b'0' + (hp / 10 % 10) as u8; pos += 1; }
                hp_str[pos] = b'0' + (hp % 10) as u8;

                self.draw_string(fb, x + 64, y + 6, &hp_str, colors::OVERLAY_TEXT_DIM);

                // Status condition
                let status_str = mon.status.as_str();
                if !status_str.is_empty() {
                    self.draw_str(fb, x + 100, y, status_str, colors::LIGHT_RED);
                }
            }
            None => {
                // Empty slot
                self.draw_str(fb, x, y, "------", colors::OVERLAY_TEXT_DIM);
            }
        }
    }

    /// Draw badge display (8 badges as filled/empty squares)
    fn draw_badges(&self, fb: &mut [u8], x: usize, y: usize, badges: u8, label: &str) {
        self.draw_str(fb, x, y, label, colors::OVERLAY_TEXT_DIM);

        let badge_y = y + 10;
        for i in 0..8 {
            let bx = x + i * 10;
            let has_badge = badges & (1 << i) != 0;

            if has_badge {
                self.draw_rect(fb, bx, badge_y, 8, 8, colors::YELLOW);
            } else {
                self.draw_box(fb, bx, badge_y, 8, 8, colors::DARK_GRAY);
            }
        }
    }

    /// Clear the overlay region
    pub fn clear_overlay(&self, fb: &mut [u8]) {
        self.draw_rect(
            fb,
            self.config.sidebar_x,
            0,
            self.config.sidebar_width,
            SCREEN_HEIGHT,
            colors::OVERLAY_BG,
        );
    }

    /// Render the full overlay
    pub fn render(&self, fb: &mut [u8], reader: &RamReader) {
        let x = self.config.sidebar_x + 4; // 4px padding
        let mut y = 4usize;

        // Game title
        let title = match self.game {
            Game::Yellow => "POKEMON YELLOW",
            Game::Crystal => "POKEMON CRYSTAL",
            Game::Unknown => "UNKNOWN GAME",
        };
        self.draw_str(fb, x, y, title, colors::YELLOW);
        y += 12;

        // Trainer info
        if self.config.show_trainer {
            let trainer = reader.read_trainer();

            // Player name
            let name = decode_text(&trainer.name);
            self.draw_string(fb, x, y, &name, colors::OVERLAY_TEXT);
            y += 10;

            // Money
            self.draw_str(fb, x, y, "$", colors::LIGHT_GREEN);
            self.draw_number(fb, x + 8, y, trainer.money, 6, colors::OVERLAY_TEXT);
            y += 10;

            // Play time
            if self.config.show_playtime {
                let mut time_str = [b' '; 10];
                time_str[0] = b'0' + (trainer.play_hours / 100 % 10) as u8;
                time_str[1] = b'0' + (trainer.play_hours / 10 % 10) as u8;
                time_str[2] = b'0' + (trainer.play_hours % 10) as u8;
                time_str[3] = b':';
                time_str[4] = b'0' + trainer.play_minutes / 10;
                time_str[5] = b'0' + trainer.play_minutes % 10;
                time_str[6] = b':';
                time_str[7] = b'0' + trainer.play_seconds / 10;
                time_str[8] = b'0' + trainer.play_seconds % 10;
                self.draw_string(fb, x, y, &time_str, colors::OVERLAY_TEXT_DIM);
                y += 10;
            }

            // Pokedex
            self.draw_str(fb, x, y, "DEX:", colors::OVERLAY_TEXT_DIM);
            self.draw_number(fb, x + 32, y, trainer.pokedex_owned as u32, 3, colors::OVERLAY_TEXT);
            self.draw_str(fb, x + 56, y, "/", colors::OVERLAY_TEXT_DIM);
            self.draw_number(fb, x + 64, y, trainer.pokedex_seen as u32, 3, colors::OVERLAY_TEXT);
            y += 12;

            // Badges
            if self.config.show_badges {
                if self.game == Game::Crystal {
                    self.draw_badges(fb, x, y, trainer.badges, "JOHTO:");
                    y += 22;
                    self.draw_badges(fb, x, y, trainer.badges_kanto, "KANTO:");
                    y += 22;
                } else {
                    self.draw_badges(fb, x, y, trainer.badges, "BADGES:");
                    y += 22;
                }
            }
        }

        // Party
        if self.config.show_party {
            self.draw_str(fb, x, y, "PARTY", colors::OVERLAY_TEXT);
            y += 10;

            let party = reader.read_party();
            for i in 0..6 {
                self.draw_party_slot(fb, x, y, i, party.pokemon[i].as_ref());
                y += 20;
            }
        }

        // Battle info (if in battle)
        if reader.in_battle() {
            if let Some((species, hp, level)) = reader.read_enemy_pokemon() {
                y = SCREEN_HEIGHT - 24;
                self.draw_str(fb, x, y, "ENEMY:", colors::LIGHT_RED);
                self.draw_str(fb, x + 48, y, "#", colors::OVERLAY_TEXT);
                self.draw_number(fb, x + 56, y, species as u32, 3, colors::OVERLAY_TEXT);
                self.draw_str(fb, x + 80, y, "L", colors::OVERLAY_TEXT);
                self.draw_number(fb, x + 88, y, level as u32, 3, colors::OVERLAY_TEXT);
            }
        }
    }
}

// =============================================================================
// Convenience Functions
// =============================================================================

/// Render overlay to framebuffer
///
/// # Arguments
/// * `fb` - Framebuffer slice (320*200 = 64000 bytes for mode 13h)
/// * `reader` - RamReader configured for the current game
/// * `game` - Which game is running
pub fn render_overlay(fb: &mut [u8], reader: &RamReader, game: Game) {
    let renderer = OverlayRenderer::new(game);
    renderer.clear_overlay(fb);
    renderer.render(fb, reader);
}

/// Check if overlay should be rendered (game is supported)
pub fn is_game_supported(game: Game) -> bool {
    matches!(game, Game::Yellow | Game::Crystal)
}
