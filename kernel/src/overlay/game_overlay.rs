//! Game Overlay - With Dirty Region Tracking
//!
//! Optimized to only redraw elements that actually changed,
//! eliminating the "clearing lines" flicker issue.

use crate::overlay::ram_layout::{decode_text, Game, Pokemon, RamReader};
use crate::overlay::dirty_region::{self, DirtyFlags, ElementRegions, OverlayCache};
use crate::gui::font_4x6;
use crate::gui::layout::{element, LayoutCursor, Region, GB_X, GB_WIDTH, GB_BOTTOM};
use crate::graphics::vga_mode13h::{colors, SCREEN_HEIGHT, SCREEN_WIDTH};

const HP_GREEN: u8 = colors::GREEN;
const HP_YELLOW: u8 = colors::YELLOW;
const HP_RED: u8 = colors::RED;
const HP_BG: u8 = colors::DARK_GRAY;
const TEXT: u8 = colors::WHITE;
const TEXT_DIM: u8 = colors::LIGHT_GRAY;
const BG: u8 = colors::BLACK;

const CHAR_W: usize = font_4x6::CELL_WIDTH;

// =============================================================================
// Configuration
// =============================================================================

#[derive(Clone, Copy)]
pub struct OverlayConfig {
    pub region: Region,
    pub padding: usize,
    pub show_party: bool,
    pub show_trainer: bool,
    pub show_badges: bool,
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
    pub fn cursor(&self) -> LayoutCursor {
        self.region.cursor(self.padding)
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

    #[inline]
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

    #[inline]
    fn draw_str(&self, fb: &mut [u8], x: usize, y: usize, s: &str, color: u8) {
        let mut cx = x;
        for c in s.bytes() {
            if cx + 4 > SCREEN_WIDTH {
                break;
            }
            self.draw_char(fb, cx, y, c, color);
            cx += CHAR_W;
        }
    }

    #[inline]
    fn draw_bytes(&self, fb: &mut [u8], x: usize, y: usize, s: &[u8], color: u8) {
        let mut cx = x;
        for &c in s {
            if c == 0 {
                break;
            }
            if cx + 4 > SCREEN_WIDTH {
                break;
            }
            self.draw_char(fb, cx, y, c, color);
            cx += CHAR_W;
        }
    }

    fn draw_number(&self, fb: &mut [u8], x: usize, y: usize, n: u32, color: u8) {
        let mut buf = [0u8; 10];
        let mut num = n;
        let mut i = buf.len();
        if num == 0 {
            i -= 1;
            buf[i] = b'0';
        } else {
            while num > 0 && i > 0 {
                i -= 1;
                buf[i] = b'0' + (num % 10) as u8;
                num /= 10;
            }
        }
        self.draw_bytes(fb, x, y, &buf[i..], color);
    }

    #[inline]
    fn draw_rect(&self, fb: &mut [u8], x: usize, y: usize, w: usize, h: usize, color: u8) {
        let end_x = (x + w).min(SCREEN_WIDTH);
        let end_y = (y + h).min(SCREEN_HEIGHT);
        for py in y..end_y {
            let row_start = py * SCREEN_WIDTH + x;
            let row_end = py * SCREEN_WIDTH + end_x;
            if row_end <= fb.len() {
                fb[row_start..row_end].fill(color);
            }
        }
    }

    #[inline]
    fn hp_color(percent: u32) -> u8 {
        if percent > 50 {
            HP_GREEN
        } else if percent > 25 {
            HP_YELLOW
        } else {
            HP_RED
        }
    }

    // =========================================================================
    // Element Rendering
    // =========================================================================

    fn render_game_title(&self, fb: &mut [u8], cursor: &mut LayoutCursor) {
        let (title, color) = match self.game {
            Game::Red => ("RED", colors::LIGHT_RED),
            Game::Blue => ("BLUE", colors::LIGHT_BLUE),
            Game::Yellow => ("YELLOW", colors::YELLOW),
            Game::Gold => ("GOLD", colors::YELLOW),
            Game::Silver => ("SILVER", colors::LIGHT_GRAY),
            Game::Crystal => ("CRYSTAL", colors::LIGHT_CYAN),
            Game::Unknown => ("???", colors::DARK_GRAY),
        };
        let y = cursor.take(element::SECTION_HEADER);
        self.draw_str(fb, cursor.x, y, title, color);
    }

    fn render_trainer_name(&self, fb: &mut [u8], cursor: &mut LayoutCursor, reader: &RamReader) {
        let trainer = reader.read_trainer();
        let y = cursor.take(element::TEXT_4X6);
        let name = decode_text(&trainer.name);
        self.draw_bytes(fb, cursor.x, y, &name, TEXT);
    }

    fn render_money(&self, fb: &mut [u8], cursor: &mut LayoutCursor, reader: &RamReader) {
        let trainer = reader.read_trainer();
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "$", colors::LIGHT_GREEN);
        self.draw_number(fb, cursor.x + 6, y, trainer.money, TEXT);
    }

    fn render_pokedex(&self, fb: &mut [u8], cursor: &mut LayoutCursor, reader: &RamReader) {
        cursor.space(2);
        let trainer = reader.read_trainer();
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "DEX", TEXT_DIM);
        self.draw_number(fb, cursor.x + 20, y, trainer.pokedex_owned as u32, TEXT);
    }

    fn render_party_header(&self, fb: &mut [u8], cursor: &mut LayoutCursor) {
        cursor.space(4);
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "PARTY", TEXT_DIM);
    }

    fn render_party_slot(&self, fb: &mut [u8], cursor: &mut LayoutCursor, pokemon: Option<&Pokemon>) {
        if !cursor.fits(element::TEXT_4X6) {
            return;
        }
        let y = cursor.take(element::TEXT_4X6);

        if let Some(pokemon) = pokemon {
            self.draw_str(fb, cursor.x, y, "#", TEXT_DIM);
            self.draw_number(fb, cursor.x + 6, y, pokemon.species as u32, TEXT);
            self.draw_str(fb, cursor.x + 30, y, "L", TEXT_DIM);
            self.draw_number(fb, cursor.x + 35, y, pokemon.level as u32, TEXT);
        } else {
            self.draw_str(fb, cursor.x, y, "---", TEXT_DIM);
        }
    }

    // =========================================================================
    // RIGHT SIDEBAR - Full render
    // =========================================================================
    fn render_right_full(&self, fb: &mut [u8], reader: &RamReader) {
        let region = Region::right_sidebar();
        let mut cursor = region.cursor(4);

        self.render_game_title(fb, &mut cursor);
        self.render_trainer_name(fb, &mut cursor, reader);
        self.render_money(fb, &mut cursor, reader);
        self.render_pokedex(fb, &mut cursor, reader);
        self.render_party_header(fb, &mut cursor);

        let party = reader.read_party();
        for i in 0..6 {
            self.render_party_slot(fb, &mut cursor, party.pokemon[i].as_ref());
        }
    }

    // =========================================================================
    // RIGHT SIDEBAR - Selective render based on dirty flags
    // =========================================================================
    fn render_right_dirty(&self, fb: &mut [u8], reader: &RamReader, dirty: &DirtyFlags) {
        let region = Region::right_sidebar();
        let mut cursor = region.cursor(4);

        if dirty.is_set(DirtyFlags::GAME_TITLE) {
            self.render_game_title(fb, &mut cursor);
        } else {
            cursor.skip(element::SECTION_HEADER);
        }

        if dirty.is_set(DirtyFlags::TRAINER_NAME) {
            self.render_trainer_name(fb, &mut cursor, reader);
        } else {
            cursor.skip(element::TEXT_4X6);
        }

        if dirty.is_set(DirtyFlags::MONEY) {
            self.render_money(fb, &mut cursor, reader);
        } else {
            cursor.skip(element::TEXT_4X6);
        }

        if dirty.is_set(DirtyFlags::POKEDEX) {
            self.render_pokedex(fb, &mut cursor, reader);
        } else {
            cursor.space(2);
            cursor.skip(element::TEXT_4X6);
        }

        if dirty.is_set(DirtyFlags::PARTY_HEADER) {
            self.render_party_header(fb, &mut cursor);
        } else {
            cursor.space(4);
            cursor.skip(element::TEXT_4X6);
        }

        let party = reader.read_party();
        for i in 0..6 {
            if dirty.is_set(DirtyFlags::party_slot_flag(i)) {
                self.render_party_slot(fb, &mut cursor, party.pokemon[i].as_ref());
            } else if cursor.fits(element::TEXT_4X6) {
                cursor.skip(element::TEXT_4X6);
            }
        }
    }

    // =========================================================================
    // LEFT SIDEBAR - Full render
    // =========================================================================
    fn render_left_full(&self, fb: &mut [u8], reader: &RamReader) {
        let region = Region::left_sidebar();
        let mut cursor = region.cursor(2);

        // Location
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "LOC", TEXT_DIM);

        let (map, x_pos, y_pos) = reader.read_location();
        let y = cursor.take(element::TEXT_4X6);
        self.draw_number(fb, cursor.x, y, map as u32, TEXT);

        // Coordinates
        let y = cursor.take(element::TEXT_4X6);
        self.draw_number(fb, cursor.x, y, x_pos as u32, TEXT);
        self.draw_str(fb, cursor.x + 15, y, ",", TEXT_DIM);
        self.draw_number(fb, cursor.x + 20, y, y_pos as u32, TEXT);

        // Badges
        cursor.space(4);
        let trainer = reader.read_trainer();
        let y = cursor.take(element::TEXT_4X6);
        self.draw_str(fb, cursor.x, y, "BADGES", TEXT_DIM);
        let y = cursor.take(element::TEXT_4X6);
        self.draw_number(fb, cursor.x, y, trainer.badges.count_ones() as u32, TEXT);

        if self.game.is_gen2() {
            self.draw_str(fb, cursor.x + 10, y, "+", TEXT_DIM);
            self.draw_number(fb, cursor.x + 15, y, trainer.badges_kanto.count_ones() as u32, TEXT);
        }

        // Play time
        cursor.space(4);
        let y = cursor.take(element::TEXT_4X6);
        self.draw_number(fb, cursor.x, y, trainer.play_hours as u32, TEXT);
        self.draw_str(fb, cursor.x + 20, y, "h", TEXT_DIM);
    }

    // =========================================================================
    // LEFT SIDEBAR - Selective render
    // =========================================================================
    fn render_left_dirty(&self, fb: &mut [u8], reader: &RamReader, dirty: &DirtyFlags) {
        let region = Region::left_sidebar();
        let mut cursor = region.cursor(2);

        if dirty.is_set(DirtyFlags::LOCATION) {
            let y = cursor.take(element::TEXT_4X6);
            self.draw_str(fb, cursor.x, y, "LOC", TEXT_DIM);

            let (map, x_pos, y_pos) = reader.read_location();
            let y = cursor.take(element::TEXT_4X6);
            self.draw_number(fb, cursor.x, y, map as u32, TEXT);

            let y = cursor.take(element::TEXT_4X6);
            self.draw_number(fb, cursor.x, y, x_pos as u32, TEXT);
            self.draw_str(fb, cursor.x + 15, y, ",", TEXT_DIM);
            self.draw_number(fb, cursor.x + 20, y, y_pos as u32, TEXT);
        } else {
            cursor.skip(element::TEXT_4X6 * 3);
        }

        cursor.space(4);
        if dirty.is_set(DirtyFlags::BADGES) {
            let trainer = reader.read_trainer();
            let y = cursor.take(element::TEXT_4X6);
            self.draw_str(fb, cursor.x, y, "BADGES", TEXT_DIM);
            let y = cursor.take(element::TEXT_4X6);
            self.draw_number(fb, cursor.x, y, trainer.badges.count_ones() as u32, TEXT);

            if self.game.is_gen2() {
                self.draw_str(fb, cursor.x + 10, y, "+", TEXT_DIM);
                self.draw_number(fb, cursor.x + 15, y, trainer.badges_kanto.count_ones() as u32, TEXT);
            }
        } else {
            cursor.skip(element::TEXT_4X6 * 2);
        }

        cursor.space(4);
        if dirty.is_set(DirtyFlags::PLAYTIME) {
            let trainer = reader.read_trainer();
            let y = cursor.take(element::TEXT_4X6);
            self.draw_number(fb, cursor.x, y, trainer.play_hours as u32, TEXT);
            self.draw_str(fb, cursor.x + 20, y, "h", TEXT_DIM);
        }
    }

    // =========================================================================
    // BOTTOM BAR - Party HP bars only
    // =========================================================================
    fn render_bottom(&self, fb: &mut [u8], reader: &RamReader) {
        let party = reader.read_party();
        let slot_width = 26usize;
        let bar_width = 22usize;

        for i in 0..6 {
            let slot_x = GB_X + 1 + i * slot_width;
            if slot_x + bar_width > GB_X + GB_WIDTH {
                break;
            }

            if let Some(pokemon) = party.pokemon[i].as_ref() {
                let bar_y = GB_BOTTOM + 4;
                let hp_pct = if pokemon.hp_max > 0 {
                    (pokemon.hp_current as u32 * 100) / pokemon.hp_max as u32
                } else {
                    0
                };

                let fill =
                    ((pokemon.hp_current as usize * 20) / pokemon.hp_max.max(1) as usize).min(20);

                self.draw_rect(fb, slot_x, bar_y, bar_width, 4, HP_BG);
                if fill > 0 {
                    self.draw_rect(fb, slot_x + 1, bar_y + 1, fill, 2, Self::hp_color(hp_pct));
                }

                let lv_y = bar_y + 6;
                self.draw_number(fb, slot_x, lv_y, pokemon.level as u32, TEXT_DIM);
            } else {
                self.draw_str(fb, slot_x, GB_BOTTOM + 4, "--", TEXT_DIM);
            }
        }
    }

    // =========================================================================
    // Full clear (only used for initial draw)
    // =========================================================================
    pub fn clear_overlay(&self, fb: &mut [u8]) {
        let right = Region::right_sidebar();
        self.draw_rect(fb, right.x, right.y, right.width, right.height, BG);

        let left = Region::left_sidebar();
        self.draw_rect(fb, left.x, left.y, left.width, left.height, BG);

        let bottom_h = SCREEN_HEIGHT - GB_BOTTOM;
        self.draw_rect(fb, GB_X, GB_BOTTOM, GB_WIDTH, bottom_h, BG);
    }

    // =========================================================================
    // Main render with dirty tracking
    // =========================================================================
    pub fn render_with_dirty_tracking(
        &self,
        fb: &mut [u8],
        reader: &RamReader,
        cache: &mut OverlayCache,
        regions: &ElementRegions,
    ) {
        // Read current state
        let trainer = reader.read_trainer();
        let party = reader.read_party();
        let location = reader.read_location();

        // Compare with cache and get dirty flags
        let dirty = cache.compare_and_update(&trainer, &party, location, self.game);

        // If nothing changed, we're done - big optimization!
        if dirty.is_empty() {
            return;
        }

        // Clear only the dirty regions
        let dirty_rects = regions.get_dirty_rects(&dirty);
        dirty_rects.clear_all(fb, BG);

        // Render only dirty elements
        self.render_right_dirty(fb, reader, &dirty);
        self.render_left_dirty(fb, reader, &dirty);

        if dirty.is_set(DirtyFlags::BOTTOM_BAR) {
            self.render_bottom(fb, reader);
        }
    }

    // =========================================================================
    // Legacy full render
    // =========================================================================
    pub fn render(&self, fb: &mut [u8], reader: &RamReader) {
        self.render_right_full(fb, reader);
        self.render_left_full(fb, reader);
        self.render_bottom(fb, reader);
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Initialize the overlay system (call once at startup)
pub fn init_overlay() {
    dirty_region::init();
}

/// Render overlay with dirty tracking (efficient - for double buffering)
/// This is the main entry point
pub fn render_overlay_efficient(fb: &mut [u8], reader: &RamReader, game: Game) {
    // Ensure initialized
    if !dirty_region::is_initialized() {
        dirty_region::init();
    }

    let renderer = OverlayRenderer::new(game);
    let cache = dirty_region::cache();
    let regions = dirty_region::regions();

    // First frame: do full clear
    if !cache.initialized {
        renderer.clear_overlay(fb);
    }

    renderer.render_with_dirty_tracking(fb, reader, cache, regions);
}

/// Legacy render_overlay (full clear + redraw every frame)
/// Keep for compatibility
pub fn render_overlay(fb: &mut [u8], reader: &RamReader, game: Game) {
    let renderer = OverlayRenderer::new(game);
    renderer.clear_overlay(fb);
    renderer.render(fb, reader);
}

/// Force a full redraw on the next frame
pub fn invalidate_overlay() {
    if dirty_region::is_initialized() {
        dirty_region::invalidate_all();
    }
}

pub fn is_game_supported(game: Game) -> bool {
    !matches!(game, Game::Unknown)
}
