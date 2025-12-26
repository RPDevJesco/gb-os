//! Game Overlay Rendering
//!
//! Renders game state overlay directly to VGA mode 13h framebuffer.
//! Uses the layout system for consistent element positioning.
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

use crate::overlay::ram_layout::{decode_text, Game, Pokemon, RamReader, StatusCondition};
use crate::gui::font_4x6;
use crate::gui::layout::{self, element, LayoutCursor, Region, GB_WIDTH, GB_HEIGHT};
use crate::graphics::vga_mode13h::{colors, SCREEN_HEIGHT, SCREEN_WIDTH};

// =============================================================================
// Color Aliases
// =============================================================================

const HP_GREEN: u8 = colors::GREEN;
const HP_YELLOW: u8 = colors::YELLOW;
const HP_RED: u8 = colors::RED;
const HP_BG: u8 = colors::DARK_GRAY;
const OVERLAY_TEXT: u8 = colors::WHITE;
const OVERLAY_TEXT_DIM: u8 = colors::LIGHT_GRAY;
const OVERLAY_BG: u8 = colors::BLACK;

// Font metrics
const CHAR_W: usize = font_4x6::CELL_WIDTH; // 5
const CHAR_H: usize = font_4x6::CELL_HEIGHT; // 7

// =============================================================================
// Overlay Configuration
// =============================================================================

/// Overlay region configuration
#[derive(Clone, Copy)]
pub struct OverlayConfig {
    /// Region where overlay renders
    pub region: Region,
    /// Padding inside the region
    pub padding: usize,
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
            region: Region::right_sidebar(),
            padding: 4,
            show_party: true,
            show_trainer: true,
            show_badges: true,
            show_playtime: true,
        }
    }
}

impl OverlayConfig {
    /// Create a layout cursor for this config
    pub fn cursor(&self) -> LayoutCursor {
        self.region.cursor(self.padding)
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

    // =========================================================================
    // Drawing Primitives
    // =========================================================================

    /// Draw a single character at (x, y)
    fn draw_char(&self, fb: &mut [u8], x: usize, y: usize, c: u8, color: u8) {
        if x + 4 > SCREEN_WIDTH || y + 6 > SCREEN_HEIGHT {
            return;
        }

        let bitmap = font_4x6::get_char_bitmap(c);
        for (row, &bits) in bitmap.iter().enumerate() {
            let py = y + row;
            for col in 0..4 {
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
            if c == 0 {
                break;
            }
            self.draw_char(fb, cx, y, c, color);
            cx += CHAR_W;
            if cx + 4 > SCREEN_WIDTH {
                break;
            }
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
            self.draw_char(fb, x + (start + j) * CHAR_W, y, c, color);
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

    // =========================================================================
    // Component Drawing
    // =========================================================================

    /// Draw an HP bar
    fn draw_hp_bar(&self, fb: &mut [u8], x: usize, y: usize, current: u16, max: u16, width: usize) {
        // Background
        self.draw_rect(fb, x, y, width, 4, HP_BG);

        // Calculate fill
        if max == 0 {
            return;
        }
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
    fn draw_party_slot(&self, fb: &mut [u8], x: usize, y: usize, pokemon: Option<&Pokemon>) {
        match pokemon {
            Some(mon) => {
                // Species number and level: "#001 L05"
                let mut line1 = [b' '; 10];
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
                self.draw_hp_bar(fb, x, y + 8, mon.hp_current, mon.hp_max, 48);

                // HP numbers: "123/456"
                let mut hp_str = [b' '; 9];
                let mut pos = 0;
                let mut hp = mon.hp_current;
                if hp >= 100 {
                    hp_str[pos] = b'0' + (hp / 100 % 10) as u8;
                    pos += 1;
                }
                if hp >= 10 {
                    hp_str[pos] = b'0' + (hp / 10 % 10) as u8;
                    pos += 1;
                }
                hp_str[pos] = b'0' + (hp % 10) as u8;
                pos += 1;
                hp_str[pos] = b'/';
                pos += 1;
                hp = mon.hp_max;
                if hp >= 100 {
                    hp_str[pos] = b'0' + (hp / 100 % 10) as u8;
                    pos += 1;
                }
                if hp >= 10 {
                    hp_str[pos] = b'0' + (hp / 10 % 10) as u8;
                    pos += 1;
                }
                hp_str[pos] = b'0' + (hp % 10) as u8;

                self.draw_string(fb, x + 50, y + 6, &hp_str, OVERLAY_TEXT_DIM);

                // Status condition
                let status_str = mon.status.as_str();
                if !status_str.is_empty() {
                    self.draw_str(fb, x + 50, y, status_str, colors::LIGHT_RED);
                }
            }
            None => {
                self.draw_str(fb, x, y, "---", OVERLAY_TEXT_DIM);
            }
        }
    }

    /// Draw badge display (8 badges as 2 rows of 4 filled/empty squares)
    fn draw_badges(&self, fb: &mut [u8], x: usize, y: usize, badges: u8, label: &str) {
        self.draw_str(fb, x, y, label, OVERLAY_TEXT_DIM);

        // 2 rows of 4 badges each
        for row in 0..2 {
            let badge_y = y + 8 + row * 9; // 8px for label, 9px per row (7px badge + 2px gap)
            for col in 0..4 {
                let badge_idx = row * 4 + col;
                let bx = x + col * 9;
                let has_badge = badges & (1 << badge_idx) != 0;

                if has_badge {
                    self.draw_rect(fb, bx, badge_y, 7, 7, colors::YELLOW);
                } else {
                    self.draw_box(fb, bx, badge_y, 7, 7, colors::DARK_GRAY);
                }
            }
        }
    }

    // =========================================================================
    // Layout-Based Rendering
    // =========================================================================

    /// Clear the overlay region
    pub fn clear_overlay(&self, fb: &mut [u8]) {
        let r = &self.config.region;
        self.draw_rect(fb, r.x, r.y, r.width, r.height, OVERLAY_BG);
    }

    /// Render game title section
    fn render_title(&self, fb: &mut [u8], cursor: &mut LayoutCursor) {
        let title = match self.game {
            Game::Yellow => "YELLOW",
            Game::Crystal => "CRYSTAL",
            Game::Unknown => "------",
        };
        let y = cursor.take(element::SECTION_HEADER);
        self.draw_str(fb, cursor.x, y, title, colors::YELLOW);
    }

    /// Render trainer info section
    fn render_trainer_info(&self, fb: &mut [u8], cursor: &mut LayoutCursor, reader: &RamReader) {
        if !self.config.show_trainer {
            // Skip space that trainer info would have taken
            cursor.skip(element::TEXT_4X6 * 4 + element::GAP_SMALL); // name + money + time + dex
            if self.config.show_badges {
                cursor.skip(if self.game == Game::Crystal {
                    element::BADGE_ROW * 2
                } else {
                    element::BADGE_ROW
                });
            }
            return;
        }

        let trainer = reader.read_trainer();

        // Player name
        let y = cursor.take(element::TEXT_4X6);
        let name = decode_text(&trainer.name);
        self.draw_string(fb, cursor.x, y, &name, OVERLAY_TEXT);
        cursor.space(1);

        // Money
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "$", colors::LIGHT_GREEN);
        self.draw_number(fb, cursor.x + CHAR_W, y, trainer.money, 6, OVERLAY_TEXT);
        cursor.space(1);

        // Play time
        if self.config.show_playtime {
            let y = cursor.take(element::TEXT_4X6);
            let mut time_str = [b' '; 9];
            time_str[0] = b'0' + (trainer.play_hours / 100 % 10) as u8;
            time_str[1] = b'0' + (trainer.play_hours / 10 % 10) as u8;
            time_str[2] = b'0' + (trainer.play_hours % 10) as u8;
            time_str[3] = b':';
            time_str[4] = b'0' + trainer.play_minutes / 10;
            time_str[5] = b'0' + trainer.play_minutes % 10;
            time_str[6] = b':';
            time_str[7] = b'0' + trainer.play_seconds / 10;
            time_str[8] = b'0' + trainer.play_seconds % 10;
            self.draw_string(fb, cursor.x, y, &time_str, OVERLAY_TEXT_DIM);
            cursor.space(1);
        }

        // Pokedex
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "DEX:", OVERLAY_TEXT_DIM);
        self.draw_number(fb, cursor.x + 20, y, trainer.pokedex_owned as u32, 3, OVERLAY_TEXT);
        self.draw_str(fb, cursor.x + 40, y, "/", OVERLAY_TEXT_DIM);
        self.draw_number(fb, cursor.x + 45, y, trainer.pokedex_seen as u32, 3, OVERLAY_TEXT);
        cursor.space(element::GAP_SMALL);

        // Badges
        if self.config.show_badges {
            if self.game == Game::Crystal {
                let y = cursor.take(element::BADGE_ROW);
                self.draw_badges(fb, cursor.x, y, trainer.badges, "JOHTO:");
                let y = cursor.take(element::BADGE_ROW);
                self.draw_badges(fb, cursor.x, y, trainer.badges_kanto, "KANTO:");
            } else {
                let y = cursor.take(element::BADGE_ROW);
                self.draw_badges(fb, cursor.x, y, trainer.badges, "BADGES:");
            }
        } else {
            // Skip badge space to maintain layout
            cursor.skip(if self.game == Game::Crystal {
                element::BADGE_ROW * 2
            } else {
                element::BADGE_ROW
            });
        }
    }

    /// Render party section
    fn render_party(&self, fb: &mut [u8], cursor: &mut LayoutCursor, reader: &RamReader) {
        if !self.config.show_party {
            // Skip party space
            cursor.skip(element::TEXT_4X6 + element::PARTY_SLOT * 6);
            return;
        }

        // Header
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "PARTY", OVERLAY_TEXT);
        cursor.space(1);

        // Party slots
        let party = reader.read_party();
        for i in 0..6 {
            if !cursor.fits(element::PARTY_SLOT) {
                break;
            }
            let y = cursor.take(element::PARTY_SLOT);
            self.draw_party_slot(fb, cursor.x, y, party.pokemon[i].as_ref());
        }
    }

    /// Render battle info (anchored to bottom)
    fn render_battle_info(&self, fb: &mut [u8], cursor: &mut LayoutCursor, reader: &RamReader) {
        if !reader.in_battle() {
            return;
        }

        if let Some((species, _hp, level)) = reader.read_enemy_pokemon() {
            // Position at bottom of region
            cursor.from_bottom(20);
            let y = cursor.take(element::TEXT_4X6);

            self.draw_str(fb, cursor.x, y, "ENEMY:", colors::LIGHT_RED);
            self.draw_str(fb, cursor.x + 35, y, "#", OVERLAY_TEXT);
            self.draw_number(fb, cursor.x + 40, y, species as u32, 3, OVERLAY_TEXT);
            self.draw_str(fb, cursor.x + 55, y, "L", OVERLAY_TEXT);
            self.draw_number(fb, cursor.x + 60, y, level as u32, 3, OVERLAY_TEXT);
        }
    }

    /// Render the full overlay
    pub fn render(&self, fb: &mut [u8], reader: &RamReader) {
        let mut cursor = self.config.cursor();

        self.render_title(fb, &mut cursor);
        self.render_trainer_info(fb, &mut cursor, reader);
        self.render_party(fb, &mut cursor, reader);
        self.render_battle_info(fb, &mut cursor, reader);
    }
}

// =============================================================================
// Convenience Functions
// =============================================================================

/// Render overlay to framebuffer
pub fn render_overlay(fb: &mut [u8], reader: &RamReader, game: Game) {
    let renderer = OverlayRenderer::new(game);
    renderer.clear_overlay(fb);
    renderer.render(fb, reader);
}

/// Check if overlay should be rendered (game is supported)
pub fn is_game_supported(game: Game) -> bool {
    matches!(game, Game::Yellow | Game::Crystal)
}
