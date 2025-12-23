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
use crate::gui::font_8x8;
use crate::graphics::vga_mode13h::{self, colors, SCREEN_WIDTH, SCREEN_HEIGHT};

// Overlay-specific aliases
const HP_GREEN: u8 = colors::GREEN;
const HP_YELLOW: u8 = colors::YELLOW;
const HP_RED: u8 = colors::RED;
const HP_BG: u8 = colors::DARK_GRAY;
const OVERLAY_TEXT: u8 = colors::WHITE;
const OVERLAY_TEXT_DIM: u8 = colors::LIGHT_GRAY;
const OVERLAY_BG: u8 = colors::BLACK;

// =============================================================================
// Overlay Configuration
// =============================================================================

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

        let bitmap = font_8x8::get_char_bitmap(c);
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
        self.draw_rect(fb, x, y, width, 4, HP_BG);

        // Calculate fill
        if max == 0 { return; }
        let percent = (current as u32 * 100) / max as u32;
        let fill_width = ((current as usize * (width - 2)) / max as usize).min(width - 2);

        // Choose color based on HP percentage
        let color = if percent > 50 {
            HP_GREEN
        } else if percent > 25 {
            HP_YELLOW
        } else {
            HP_RED
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

                self.draw_string(fb, x, y, &line1, OVERLAY_TEXT);

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

                self.draw_string(fb, x + 64, y + 6, &hp_str, OVERLAY_TEXT_DIM);

                // Status condition
                let status_str = mon.status.as_str();
                if !status_str.is_empty() {
                    self.draw_str(fb, x + 100, y, status_str, colors::LIGHT_RED);
                }
            }
            None => {
                // Empty slot
                self.draw_str(fb, x, y, "------", OVERLAY_TEXT_DIM);
            }
        }
    }

    /// Draw badge display (8 badges as filled/empty squares)
    fn draw_badges(&self, fb: &mut [u8], x: usize, y: usize, badges: u8, label: &str) {
        self.draw_str(fb, x, y, label, OVERLAY_TEXT_DIM);

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
            OVERLAY_BG,
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
            self.draw_string(fb, x, y, &name, OVERLAY_TEXT);
            y += 10;

            // Money
            self.draw_str(fb, x, y, "$", colors::LIGHT_GREEN);
            self.draw_number(fb, x + 8, y, trainer.money, 6, OVERLAY_TEXT);
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
                self.draw_string(fb, x, y, &time_str, OVERLAY_TEXT_DIM);
                y += 10;
            }

            // Pokedex
            self.draw_str(fb, x, y, "DEX:", OVERLAY_TEXT_DIM);
            self.draw_number(fb, x + 32, y, trainer.pokedex_owned as u32, 3, OVERLAY_TEXT);
            self.draw_str(fb, x + 56, y, "/", OVERLAY_TEXT_DIM);
            self.draw_number(fb, x + 64, y, trainer.pokedex_seen as u32, 3, OVERLAY_TEXT);
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
            self.draw_str(fb, x, y, "PARTY", OVERLAY_TEXT);
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
                self.draw_str(fb, x + 48, y, "#", OVERLAY_TEXT);
                self.draw_number(fb, x + 56, y, species as u32, 3, OVERLAY_TEXT);
                self.draw_str(fb, x + 80, y, "L", OVERLAY_TEXT);
                self.draw_number(fb, x + 88, y, level as u32, 3, OVERLAY_TEXT);
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
