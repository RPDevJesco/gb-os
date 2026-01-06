//! Game Overlay - Enhanced Three-Panel Layout
//!
//! LEFT PANEL:  Map name, Badges, Lead Pokemon info (name, moves, stats)
//! BOTTOM:      Party HP bars with levels
//! RIGHT PANEL: Player info (normal) / Enemy info (battle)
//!
//! Features:
//! - Dirty region tracking for efficient updates
//! - Battle mode detection with enemy display
//! - Gen-specific badge layouts (Gen1 vs Gen2)
//! - Move PP display with current/max values

use crate::overlay::ram_layout::{decode_text, Game, Pokemon, BattlePokemon, RamReader};
use crate::overlay::pokemon_names::get_name as get_pokemon_name;
use crate::overlay::move_names::get_move_name;
use crate::overlay::map_names::{get_gen1_map_name, get_gen2_map_name};
// use crate::overlay::item_names::{get_gen1_item_name, get_gen2_item_name};  // TODO: Enable when bag reading is implemented
use crate::overlay::move_pp::{get_actual_max_pp, extract_pp_ups, extract_current_pp};
use crate::overlay::catch_rate::{get_catch_rate, get_catch_tier};
use crate::gui::layout::{element, LayoutCursor, Region, GB_X, GB_WIDTH, GB_BOTTOM};

#[cfg(target_arch = "x86")]
use crate::graphics::vga_mode13h::{colors, SCREEN_HEIGHT, SCREEN_WIDTH};

use crate::gui::font_4x6;
#[cfg(not(target_arch = "x86"))]
use font_4x6::colors;
#[cfg(not(target_arch = "x86"))]
pub const SCREEN_WIDTH: usize = 320;
#[cfg(not(target_arch = "x86"))]
pub const SCREEN_HEIGHT: usize = 200;

// =============================================================================
// Colors
// =============================================================================

const HP_GREEN: u8 = colors::GREEN;
const HP_YELLOW: u8 = colors::YELLOW;
const HP_RED: u8 = colors::RED;
const HP_BG: u8 = colors::DARK_GRAY;
const TEXT: u8 = colors::WHITE;
const TEXT_DIM: u8 = colors::LIGHT_GRAY;
const TEXT_HIGHLIGHT: u8 = colors::CYAN;
const BG: u8 = colors::BLACK;
const BADGE_EMPTY: u8 = colors::DARK_GRAY;
const BADGE_FILLED: u8 = colors::YELLOW;
const BADGE_KANTO: u8 = colors::LIGHT_BLUE;

const CHAR_W: usize = font_4x6::CELL_WIDTH;

// =============================================================================
// Configuration
// =============================================================================

#[derive(Clone, Copy)]
pub struct OverlayConfig {
    pub padding: usize,
    pub show_party: bool,
    pub show_trainer: bool,
    pub show_badges: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            padding: 2,
            show_party: true,
            show_trainer: true,
            show_badges: true,
        }
    }
}

// =============================================================================
// Overlay Renderer
// =============================================================================

pub struct OverlayRenderer {
    game: Game,
}

impl OverlayRenderer {
    pub fn new(game: Game) -> Self {
        Self { game }
    }

    // =========================================================================
    // Drawing Primitives
    // =========================================================================

    fn draw_rect(&self, fb: &mut [u8], x: usize, y: usize, w: usize, h: usize, color: u8) {
        for row in y..(y + h).min(SCREEN_HEIGHT) {
            let start = row * SCREEN_WIDTH + x;
            let end = (start + w).min(row * SCREEN_WIDTH + SCREEN_WIDTH);
            if start < fb.len() && end <= fb.len() {
                for pixel in &mut fb[start..end] {
                    *pixel = color;
                }
            }
        }
    }

    fn draw_char(&self, fb: &mut [u8], x: usize, y: usize, ch: u8, color: u8) {
        let glyph = font_4x6::get_char_bitmap(ch);
        for (row_idx, &row_bits) in glyph.iter().enumerate() {
            let py = y + row_idx;
            if py >= SCREEN_HEIGHT { break; }
            for col in 0..font_4x6::CHAR_WIDTH {
                // Top 4 bits contain pixel data, MSB = leftmost
                if (row_bits >> (7 - col)) & 1 == 1 {
                    let px = x + col;
                    if px < SCREEN_WIDTH {
                        let offset = py * SCREEN_WIDTH + px;
                        if offset < fb.len() {
                            fb[offset] = color;
                        }
                    }
                }
            }
        }
    }

    fn draw_str(&self, fb: &mut [u8], x: usize, y: usize, s: &str, color: u8) {
        let mut cx = x;
        for ch in s.bytes() {
            if cx + font_4x6::CHAR_WIDTH > SCREEN_WIDTH { break; }
            self.draw_char(fb, cx, y, ch, color);
            cx += CHAR_W;
        }
    }

    fn draw_str_right(&self, fb: &mut [u8], right_x: usize, y: usize, s: &str, color: u8) {
        let width = s.len() * CHAR_W;
        let x = right_x.saturating_sub(width);
        self.draw_str(fb, x, y, s, color);
    }

    fn draw_number(&self, fb: &mut [u8], x: usize, y: usize, num: u32, color: u8) {
        let mut buf = [0u8; 10];
        let s = format_number(num, &mut buf);
        self.draw_str(fb, x, y, s, color);
    }

    fn draw_number_padded(&self, fb: &mut [u8], x: usize, y: usize, num: u32, width: usize, color: u8) {
        let mut buf = [0u8; 10];
        let s = format_number(num, &mut buf);
        // Pad with spaces
        let pad = width.saturating_sub(s.len());
        let pad_x = x + pad * CHAR_W;
        self.draw_str(fb, pad_x, y, s, color);
    }

    fn hp_color(percent: u32) -> u8 {
        if percent > 50 { HP_GREEN }
        else if percent > 20 { HP_YELLOW }
        else { HP_RED }
    }

    // =========================================================================
    // Badge Drawing
    // =========================================================================

    fn draw_badge_grid(&self, fb: &mut [u8], x: usize, y: usize, badges: u8, color: u8) {
        // Draw 2 rows of 4 badges each
        const BADGE_SIZE: usize = 5;
        const BADGE_GAP: usize = 2;

        for row in 0..2 {
            for col in 0..4 {
                let badge_idx = row * 4 + col;
                let has_badge = (badges >> badge_idx) & 1 == 1;

                let bx = x + col * (BADGE_SIZE + BADGE_GAP);
                let by = y + row * (BADGE_SIZE + BADGE_GAP);

                let fill_color = if has_badge { color } else { BADGE_EMPTY };
                self.draw_rect(fb, bx, by, BADGE_SIZE, BADGE_SIZE, fill_color);
            }
        }
    }

    // =========================================================================
    // LEFT PANEL - Map, Badges, Lead Pokemon
    // =========================================================================

    pub fn render_left_panel(&self, fb: &mut [u8], reader: &RamReader) {
        let left = Region::left_sidebar();
        let mut cursor = left.cursor(2);

        // Clear panel
        self.draw_rect(fb, left.x, left.y, left.width, left.height, BG);

        // --- Map Name ---
        let (map, map_group, _, _) = self.read_location(reader);
        let map_name = if self.game.is_gen1() {
            get_gen1_map_name(map)
        } else {
            get_gen2_map_name(map_group, map)
        };

        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, map_name, TEXT_HIGHLIGHT);

        cursor.space(4);

        // --- Badges ---
        let trainer = reader.read_trainer();

        if self.game.is_gen2() {
            // Gen 2: Johto + Kanto badges
            let y = cursor.take(element::TEXT_4X6);
            self.draw_str(fb, cursor.x, y, "JOHTO", TEXT_DIM);
            let y = cursor.take(14);
            self.draw_badge_grid(fb, cursor.x, y, trainer.badges, BADGE_FILLED);

            cursor.space(2);
            let y = cursor.take(element::TEXT_4X6);
            self.draw_str(fb, cursor.x, y, "KANTO", TEXT_DIM);
            let y = cursor.take(14);
            self.draw_badge_grid(fb, cursor.x, y, trainer.badges_kanto, BADGE_KANTO);
        } else {
            // Gen 1: 8 badges in 2x4 grid
            let y = cursor.take(element::TEXT_4X6);
            self.draw_str(fb, cursor.x, y, "BADGES", TEXT_DIM);
            let y = cursor.take(14);
            self.draw_badge_grid(fb, cursor.x, y, trainer.badges, BADGE_FILLED);
        }

        cursor.space(4);

        // --- Lead Pokemon Info ---
        let party = reader.read_party();
        if let Some(lead) = party.pokemon[0].as_ref() {
            self.render_pokemon_details(fb, &mut cursor, lead, true);
        }
    }

    fn render_pokemon_details(&self, fb: &mut [u8], cursor: &mut LayoutCursor, pokemon: &Pokemon, show_friendship: bool) {
        // Pokemon Name
        let name = get_pokemon_name(pokemon.species);
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, name, TEXT);

        // Level
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "LV", TEXT_DIM);
        self.draw_number(fb, cursor.x + 12, y, pokemon.level as u32, TEXT);

        cursor.space(2);

        // Moves with PP - show all 4 slots
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "MOVES", TEXT_DIM);

        for i in 0..4 {
            let y = cursor.take(element::TEXT_4X6);
            let move_id = pokemon.moves[i];

            if move_id == 0 {
                // Empty slot
                self.draw_str(fb, cursor.x, y, "---", TEXT_DIM);
            } else {
                let move_name = get_move_name(move_id);

                // Truncate move name (max 5 chars to fit PP)
                let display_name: &str = if move_name.len() > 5 {
                    &move_name[..5]
                } else {
                    move_name
                };
                self.draw_str(fb, cursor.x, y, display_name, TEXT);

                // PP display: current/max - keep within left panel (ends at x=80)
                let raw_pp = pokemon.pp[i];
                let current_pp = extract_current_pp(raw_pp);
                let pp_ups = extract_pp_ups(raw_pp);
                let max_pp = get_actual_max_pp(move_id, pp_ups);

                // PP at column 30 from cursor.x (leaves room before x=80 game boundary)
                let pp_x = cursor.x + 30;
                self.draw_number(fb, pp_x, y, current_pp as u32, TEXT_DIM);
                self.draw_str(fb, pp_x + 12, y, "/", TEXT_DIM);
                self.draw_number(fb, pp_x + 17, y, max_pp as u32, TEXT_DIM);
            }
        }

        cursor.space(2);

        // Stats - compact format to fit in left panel (ends at x=80)
        // Using swap_bytes as workaround for byte-order issue in stat reading
        let hp_cur = swap_bytes(pokemon.hp_current);
        let hp_max = swap_bytes(pokemon.hp_max);
        let atk = swap_bytes(pokemon.attack);
        let def = swap_bytes(pokemon.defense);
        let spd = swap_bytes(pokemon.speed);
        let spa = swap_bytes(pokemon.special);
        let spd_def = swap_bytes(pokemon.special_def);

        // HP line
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "HP", TEXT_DIM);
        self.draw_number(fb, cursor.x + 12, y, hp_cur as u32, TEXT);
        self.draw_str(fb, cursor.x + 27, y, "/", TEXT_DIM);
        self.draw_number(fb, cursor.x + 32, y, hp_max as u32, TEXT);

        // ATK/DEF on same line
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "A", TEXT_DIM);
        self.draw_number(fb, cursor.x + 6, y, atk as u32, TEXT);
        self.draw_str(fb, cursor.x + 26, y, "D", TEXT_DIM);
        self.draw_number(fb, cursor.x + 32, y, def as u32, TEXT);

        // SPD + SPC/SPA
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "S", TEXT_DIM);
        self.draw_number(fb, cursor.x + 6, y, spd as u32, TEXT);

        if self.game.is_gen2() {
            self.draw_str(fb, cursor.x + 26, y, "SA", TEXT_DIM);
            self.draw_number(fb, cursor.x + 38, y, spa as u32, TEXT);

            // SD + Friendship
            let y = cursor.take(element::TEXT_4X6);
            self.draw_str(fb, cursor.x, y, "SD", TEXT_DIM);
            self.draw_number(fb, cursor.x + 12, y, spd_def as u32, TEXT);

            if show_friendship {
                self.draw_str(fb, cursor.x + 32, y, "F", TEXT_DIM);
                self.draw_number(fb, cursor.x + 38, y, pokemon.friendship as u32, TEXT);
            }
        } else {
            // Gen 1: Special only
            self.draw_str(fb, cursor.x + 26, y, "SP", TEXT_DIM);
            self.draw_number(fb, cursor.x + 38, y, spa as u32, TEXT);
        }
    }

    // =========================================================================
    // RIGHT PANEL - Player info (normal) / Enemy info (battle)
    // =========================================================================

    pub fn render_right_panel(&self, fb: &mut [u8], reader: &RamReader) {
        let right = Region::right_sidebar();
        let mut cursor = right.cursor(2);

        // Clear panel
        self.draw_rect(fb, right.x, right.y, right.width, right.height, BG);

        let in_battle = reader.in_battle();

        if in_battle {
            self.render_battle_info(fb, &mut cursor, reader);
        } else {
            self.render_player_info(fb, &mut cursor, reader);
        }
    }

    fn render_player_info(&self, fb: &mut [u8], cursor: &mut LayoutCursor, reader: &RamReader) {
        let trainer = reader.read_trainer();

        // Player Name
        let y = cursor.take(element::TEXT_4X6);
        let decoded = decode_text(&trainer.name);
        let name_str = bytes_to_str(&decoded);
        self.draw_str(fb, cursor.x, y, name_str, TEXT_HIGHLIGHT);

        cursor.space(2);

        // Money
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "$", TEXT_DIM);
        self.draw_number(fb, cursor.x + 8, y, trainer.money, TEXT);

        cursor.space(4);

        // Bag Contents Header
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "BAG", TEXT_DIM);

        cursor.space(2);

        // Read and display bag items
        // For now, show placeholder - bag reading needs RAM addresses
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "(Items)", TEXT_DIM);

        // TODO: Implement actual bag reading from RAM
        // This would require adding bag addresses to ram_layout.rs:
        // - Gen 1: ITEM_BAG_COUNT, ITEM_BAG_DATA
        // - Gen 2: ITEM_POCKET_COUNT, KEY_POCKET_COUNT, etc.
    }

    fn render_battle_info(&self, fb: &mut [u8], cursor: &mut LayoutCursor, reader: &RamReader) {
        // Battle Type Header - use in_battle() since read_battle_type() may not work for all games
        let y = cursor.take(element::TEXT_4X6);
        let battle_type = self.read_battle_type(reader);
        let header = match battle_type {
            1 => "WILD",
            2 => "TRAINER",
            _ => "BATTLE",  // Default to "BATTLE" if type unknown but we know we're in battle
        };
        self.draw_str(fb, cursor.x, y, header, TEXT_HIGHLIGHT);

        cursor.space(2);

        // Enemy Pokemon
        if let Some(enemy) = reader.read_battle_enemy_pokemon() {
            self.render_enemy_pokemon(fb, cursor, &enemy, reader);
        }
    }

    fn render_enemy_pokemon(&self, fb: &mut [u8], cursor: &mut LayoutCursor, enemy: &BattlePokemon, reader: &RamReader) {
        // Enemy Name
        let name = get_pokemon_name(enemy.species);
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, name, TEXT);

        // Level
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "LV", TEXT_DIM);
        self.draw_number(fb, cursor.x + 12, y, enemy.level as u32, TEXT);

        // HP - apply byte swap fix
        let hp_cur = swap_bytes(enemy.hp_current);
        let hp_max = swap_bytes(enemy.hp_max);
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "HP", TEXT_DIM);
        self.draw_number(fb, cursor.x + 12, y, hp_cur as u32, TEXT);
        self.draw_str(fb, cursor.x + 27, y, "/", TEXT_DIM);
        self.draw_number(fb, cursor.x + 32, y, hp_max as u32, TEXT);

        cursor.space(2);

        // Enemy Moves
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "MOVES", TEXT_DIM);

        for i in 0..4 {
            let move_id = enemy.moves[i];
            if move_id == 0 { continue; }

            let y = cursor.take(element::TEXT_4X6);
            let move_name = get_move_name(move_id);
            // Truncate if needed (8 chars max for right panel)
            let display_name: &str = if move_name.len() > 8 {
                &move_name[..8]
            } else {
                move_name
            };
            self.draw_str(fb, cursor.x, y, display_name, TEXT);
        }

        cursor.space(2);

        // Wild Pokemon: Show catch rate
        // Trainer Pokemon: Show trainer's party count
        let battle_type = self.read_battle_type(reader);
        if battle_type == 1 {
            // Wild battle - show catch rate
            let catch_rate = get_catch_rate(enemy.species);
            let tier = get_catch_tier(enemy.species);

            let y = cursor.take(element::TEXT_4X6);
            self.draw_str(fb, cursor.x, y, "CATCH", TEXT_DIM);

            let y = cursor.take(element::TEXT_4X6);
            self.draw_number(fb, cursor.x, y, catch_rate as u32, TEXT);
            self.draw_str(fb, cursor.x + 20, y, tier, TEXT_DIM);
        } else if battle_type == 2 {
            // Trainer battle - show enemy party count
            let enemy_count = self.read_enemy_party_count(reader);
            let y = cursor.take(element::TEXT_4X6);
            self.draw_str(fb, cursor.x, y, "PARTY", TEXT_DIM);
            self.draw_number(fb, cursor.x + 30, y, enemy_count as u32, TEXT);
        }
    }

    // =========================================================================
    // BOTTOM PANEL - Party HP bars
    // =========================================================================

    pub fn render_bottom_panel(&self, fb: &mut [u8], reader: &RamReader) {
        let region = Region::bottom_sidebar();
        let party = reader.read_party();
        let slot_width = 26usize;
        let bar_width = 22usize;

        // Clear bottom area
        self.draw_rect(fb, region.x, region.y, region.width, region.height, BG);

        // Center the party slots within the region
        let total_width = 6 * slot_width;
        let start_x = region.x + (region.width - total_width) / 2;

        for i in 0..6 {
            let slot_x = start_x + i * slot_width;

            if let Some(pokemon) = party.pokemon[i].as_ref() {
                let bar_y = region.y + 4;
                let hp_pct = if pokemon.hp_max > 0 {
                    (pokemon.hp_current as u32 * 100) / pokemon.hp_max as u32
                } else {
                    0
                };

                let fill =
                    ((pokemon.hp_current as usize * 20) / pokemon.hp_max.max(1) as usize).min(20);

                // HP bar background
                self.draw_rect(fb, slot_x, bar_y, bar_width, 4, HP_BG);

                // HP bar fill
                if fill > 0 {
                    self.draw_rect(fb, slot_x + 1, bar_y + 1, fill, 2, Self::hp_color(hp_pct));
                }

                // Level below bar
                let lv_y = bar_y + 6;
                self.draw_number(fb, slot_x, lv_y, pokemon.level as u32, TEXT_DIM);
            } else {
                // Empty slot
                self.draw_str(fb, slot_x, region.y + 4, "--", TEXT_DIM);
            }
        }
    }

    // =========================================================================
    // Main Render
    // =========================================================================

    pub fn render_full(&self, fb: &mut [u8], reader: &RamReader) {
        self.render_left_panel(fb, reader);
        self.render_right_panel(fb, reader);
        self.render_bottom_panel(fb, reader);
    }

    pub fn clear_overlay(&self, fb: &mut [u8]) {
        let right = Region::right_sidebar();
        self.draw_rect(fb, right.x, right.y, right.width, right.height, BG);

        let left = Region::left_sidebar();
        self.draw_rect(fb, left.x, left.y, left.width, left.height, BG);

        let bottom_h = SCREEN_HEIGHT - GB_BOTTOM;
        self.draw_rect(fb, GB_X, GB_BOTTOM, GB_WIDTH, bottom_h, BG);
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    fn read_location(&self, reader: &RamReader) -> (u8, u8, u8, u8) {
        let (map, x, y) = reader.read_location();
        // For Gen 2, the map value is the map number, and we need group from a different location
        // The RamReader already handles this, but we might need group for map name lookup
        let group = if self.game.is_gen2() {
            // In Gen 2, map group is stored separately
            reader.read_map_group()
        } else {
            0
        };
        (map, group, x, y)
    }

    fn read_battle_type(&self, reader: &RamReader) -> u8 {
        // Returns: 0 = no battle, 1 = wild, 2 = trainer
        reader.read_battle_type()
    }

    fn read_enemy_party_count(&self, reader: &RamReader) -> u8 {
        reader.read_enemy_party_count()
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Swap bytes in a u16 - workaround for byte-order issues in stat reading
#[inline]
fn swap_bytes(val: u16) -> u16 {
    ((val & 0xFF) << 8) | ((val >> 8) & 0xFF)
}

fn format_number(num: u32, buf: &mut [u8; 10]) -> &str {
    if num == 0 {
        return "0";
    }

    let mut n = num;
    let mut idx = buf.len();

    while n > 0 && idx > 0 {
        idx -= 1;
        buf[idx] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    // Safety: buf contains only ASCII digits
    unsafe { core::str::from_utf8_unchecked(&buf[idx..]) }
}

fn bytes_to_str(bytes: &[u8]) -> &str {
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    // Safety: decode_text produces ASCII
    unsafe { core::str::from_utf8_unchecked(&bytes[..len]) }
}

// =============================================================================
// Public API
// =============================================================================

/// Check if game is supported for overlay
pub fn is_game_supported(game: Game) -> bool {
    !matches!(game, Game::Unknown)
}

/// Legacy full render (for compatibility)
pub fn render_overlay(fb: &mut [u8], reader: &RamReader, game: Game) {
    if !is_game_supported(game) { return; }
    let renderer = OverlayRenderer::new(game);
    renderer.render_full(fb, reader);
}

// =============================================================================
// Global State for Efficient Rendering
// =============================================================================

use core::sync::atomic::{AtomicBool, Ordering};

static OVERLAY_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize overlay system (call once at startup)
pub fn init_overlay() {
    OVERLAY_INITIALIZED.store(true, Ordering::SeqCst);
}

/// Force full redraw on next frame
pub fn invalidate_overlay() {
    // Could set a flag here if using dirty tracking
}

/// Render overlay with efficient dirty tracking
pub fn render_overlay_efficient(fb: &mut [u8], reader: &RamReader, game: Game) {
    if !is_game_supported(game) { return; }

    // For now, just do full render
    // In future, can add dirty tracking here
    let renderer = OverlayRenderer::new(game);
    renderer.render_full(fb, reader);
}
