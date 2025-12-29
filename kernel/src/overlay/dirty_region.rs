//! Dirty Region Tracking for Efficient Overlay Rendering
//!
//! Instead of clearing and redrawing entire panels every frame,
//! this module tracks what actually changed and only updates those regions.
//!
//! # Strategy
//! 1. Cache previous frame's data (trainer info, party state, etc.)
//! 2. Compare current data to cached data
//! 3. Only clear and redraw regions where data changed
//! 4. Use per-element dirty flags to minimize writes

use crate::gui::layout::{element, Region, GB_X, GB_WIDTH, GB_BOTTOM};
use crate::graphics::vga_mode13h::{SCREEN_WIDTH, SCREEN_HEIGHT};
use super::ram_layout::{Game, PartyState, TrainerData, Pokemon};

// =============================================================================
// Dirty Region Flags
// =============================================================================

/// Bitflags for which overlay elements need redrawing
#[derive(Clone, Copy, Default)]
pub struct DirtyFlags {
    pub bits: u16,
}

impl DirtyFlags {
    pub const NONE: u16 = 0;
    pub const GAME_TITLE: u16 = 1 << 0;
    pub const TRAINER_NAME: u16 = 1 << 1;
    pub const MONEY: u16 = 1 << 2;
    pub const POKEDEX: u16 = 1 << 3;
    pub const PARTY_HEADER: u16 = 1 << 4;
    pub const PARTY_SLOT_0: u16 = 1 << 5;
    pub const PARTY_SLOT_1: u16 = 1 << 6;
    pub const PARTY_SLOT_2: u16 = 1 << 7;
    pub const PARTY_SLOT_3: u16 = 1 << 8;
    pub const PARTY_SLOT_4: u16 = 1 << 9;
    pub const PARTY_SLOT_5: u16 = 1 << 10;
    pub const LOCATION: u16 = 1 << 11;
    pub const BADGES: u16 = 1 << 12;
    pub const PLAYTIME: u16 = 1 << 13;
    pub const BOTTOM_BAR: u16 = 1 << 14;
    pub const ALL: u16 = 0x7FFF;

    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    pub const fn all() -> Self {
        Self { bits: Self::ALL }
    }

    #[inline]
    pub fn set(&mut self, flag: u16) {
        self.bits |= flag;
    }

    #[inline]
    pub fn clear_flag(&mut self, flag: u16) {
        self.bits &= !flag;
    }

    #[inline]
    pub const fn is_set(&self, flag: u16) -> bool {
        (self.bits & flag) != 0
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.bits == 0
    }

    #[inline]
    pub fn reset(&mut self) {
        self.bits = 0;
    }

    #[inline]
    pub fn set_all(&mut self) {
        self.bits = Self::ALL;
    }

    /// Get the party slot flag for a given index (0-5)
    #[inline]
    pub const fn party_slot_flag(index: usize) -> u16 {
        match index {
            0 => Self::PARTY_SLOT_0,
            1 => Self::PARTY_SLOT_1,
            2 => Self::PARTY_SLOT_2,
            3 => Self::PARTY_SLOT_3,
            4 => Self::PARTY_SLOT_4,
            5 => Self::PARTY_SLOT_5,
            _ => 0,
        }
    }

    /// Check if any party slot is dirty
    #[inline]
    pub const fn any_party_dirty(&self) -> bool {
        const PARTY_MASK: u16 = DirtyFlags::PARTY_SLOT_0
            | DirtyFlags::PARTY_SLOT_1
            | DirtyFlags::PARTY_SLOT_2
            | DirtyFlags::PARTY_SLOT_3
            | DirtyFlags::PARTY_SLOT_4
            | DirtyFlags::PARTY_SLOT_5;
        (self.bits & PARTY_MASK) != 0
    }
}

// =============================================================================
// Cached State
// =============================================================================

/// Cached Pokemon data for comparison
#[derive(Clone, Copy)]
pub struct CachedPokemon {
    pub species: u8,
    pub level: u8,
    pub hp_current: u16,
    pub hp_max: u16,
    pub present: bool,
}

impl Default for CachedPokemon {
    fn default() -> Self {
        Self {
            species: 0,
            level: 0,
            hp_current: 0,
            hp_max: 0,
            present: false,
        }
    }
}

impl CachedPokemon {
    pub fn from_pokemon(pokemon: Option<&Pokemon>) -> Self {
        match pokemon {
            Some(p) => Self {
                species: p.species,
                level: p.level,
                hp_current: p.hp_current,
                hp_max: p.hp_max,
                present: true,
            },
            None => Self::default(),
        }
    }

    /// Check if sidebar display changed (species, level)
    pub fn sidebar_changed(&self, other: &Self) -> bool {
        self.present != other.present
            || self.species != other.species
            || self.level != other.level
    }

    /// Check if HP bar changed
    pub fn hp_changed(&self, other: &Self) -> bool {
        self.present != other.present
            || self.hp_current != other.hp_current
            || self.hp_max != other.hp_max
    }
}

/// Cached trainer data for comparison
#[derive(Clone, Copy)]
pub struct CachedTrainer {
    pub name: [u8; 11],
    pub money: u32,
    pub badges: u8,
    pub badges_kanto: u8,
    pub pokedex_owned: u8,
    pub play_hours: u16,
}

impl Default for CachedTrainer {
    fn default() -> Self {
        Self {
            name: [0; 11],
            money: 0,
            badges: 0,
            badges_kanto: 0,
            pokedex_owned: 0,
            play_hours: 0,
        }
    }
}

impl CachedTrainer {
    pub fn from_trainer(trainer: &TrainerData) -> Self {
        Self {
            name: trainer.name,
            money: trainer.money,
            badges: trainer.badges,
            badges_kanto: trainer.badges_kanto,
            pokedex_owned: trainer.pokedex_owned,
            play_hours: trainer.play_hours,
        }
    }
}

/// Cached location data
#[derive(Clone, Copy, Default)]
pub struct CachedLocation {
    pub map: u8,
    pub x: u8,
    pub y: u8,
}

/// Full overlay state cache
pub struct OverlayCache {
    pub trainer: CachedTrainer,
    pub party: [CachedPokemon; 6],
    pub location: CachedLocation,
    pub game: Game,
    pub initialized: bool,
    frame_count: u32,
}

impl Default for OverlayCache {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayCache {
    pub const fn new() -> Self {
        Self {
            trainer: CachedTrainer {
                name: [0; 11],
                money: 0,
                badges: 0,
                badges_kanto: 0,
                pokedex_owned: 0,
                play_hours: 0,
            },
            party: [CachedPokemon {
                species: 0,
                level: 0,
                hp_current: 0,
                hp_max: 0,
                present: false,
            }; 6],
            location: CachedLocation { map: 0, x: 0, y: 0 },
            game: Game::Unknown,
            initialized: false,
            frame_count: 0,
        }
    }

    /// Compare with new state and return dirty flags
    pub fn compare_and_update(
        &mut self,
        trainer: &TrainerData,
        party: &PartyState,
        location: (u8, u8, u8),
        game: Game,
    ) -> DirtyFlags {
        let mut dirty = DirtyFlags::new();

        // First frame - everything is dirty
        if !self.initialized {
            dirty.set_all();
            self.initialized = true;
            // Still update cache below
        }

        self.frame_count = self.frame_count.wrapping_add(1);

        // Check game change
        if core::mem::discriminant(&self.game) != core::mem::discriminant(&game) {
            dirty.set(DirtyFlags::GAME_TITLE);
            self.game = game;
        }

        // Check trainer name
        if self.trainer.name != trainer.name {
            dirty.set(DirtyFlags::TRAINER_NAME);
            self.trainer.name = trainer.name;
        }

        // Check money
        if self.trainer.money != trainer.money {
            dirty.set(DirtyFlags::MONEY);
            self.trainer.money = trainer.money;
        }

        // Check pokedex
        if self.trainer.pokedex_owned != trainer.pokedex_owned {
            dirty.set(DirtyFlags::POKEDEX);
            self.trainer.pokedex_owned = trainer.pokedex_owned;
        }

        // Check badges
        if self.trainer.badges != trainer.badges || self.trainer.badges_kanto != trainer.badges_kanto {
            dirty.set(DirtyFlags::BADGES);
            self.trainer.badges = trainer.badges;
            self.trainer.badges_kanto = trainer.badges_kanto;
        }

        // Check playtime (only hours for display)
        if self.trainer.play_hours != trainer.play_hours {
            dirty.set(DirtyFlags::PLAYTIME);
            self.trainer.play_hours = trainer.play_hours;
        }

        // Check location
        let (map, x, y) = location;
        if self.location.map != map || self.location.x != x || self.location.y != y {
            dirty.set(DirtyFlags::LOCATION);
            self.location = CachedLocation { map, x, y };
        }

        // Check each party slot
        for i in 0..6 {
            let new_pokemon = CachedPokemon::from_pokemon(party.pokemon[i].as_ref());
            let old_pokemon = &self.party[i];

            // Check if sidebar info changed (species/level in right panel)
            if old_pokemon.sidebar_changed(&new_pokemon) {
                dirty.set(DirtyFlags::party_slot_flag(i));
            }

            // Check if HP changed (bottom bar)
            if old_pokemon.hp_changed(&new_pokemon) {
                dirty.set(DirtyFlags::BOTTOM_BAR);
            }

            self.party[i] = new_pokemon;
        }

        dirty
    }

    /// Force full redraw on next frame
    pub fn invalidate(&mut self) {
        self.initialized = false;
    }

    /// Get frame count (for debugging/throttling)
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }
}

// =============================================================================
// Dirty Rectangles
// =============================================================================

/// A rectangle that needs to be cleared before redrawing
#[derive(Clone, Copy, Debug)]
pub struct DirtyRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl DirtyRect {
    pub const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self { x, y, width, height }
    }

    /// Clear this rect in the framebuffer using fill
    #[inline]
    pub fn clear(&self, fb: &mut [u8], color: u8) {
        let end_x = (self.x + self.width).min(SCREEN_WIDTH);
        let end_y = (self.y + self.height).min(SCREEN_HEIGHT);

        for py in self.y..end_y {
            let row_start = py * SCREEN_WIDTH + self.x;
            let row_end = py * SCREEN_WIDTH + end_x;
            if row_end <= fb.len() {
                // Use slice fill - much faster than per-pixel
                fb[row_start..row_end].fill(color);
            }
        }
    }
}

/// Collection of dirty rectangles (fixed capacity for no_std)
pub struct DirtyRects {
    rects: [Option<DirtyRect>; 16],
    count: usize,
}

impl Default for DirtyRects {
    fn default() -> Self {
        Self::new()
    }
}

impl DirtyRects {
    pub const fn new() -> Self {
        Self {
            rects: [None; 16],
            count: 0,
        }
    }

    pub fn clear(&mut self) {
        self.count = 0;
        for r in &mut self.rects {
            *r = None;
        }
    }

    pub fn add(&mut self, rect: DirtyRect) {
        if self.count < 16 {
            self.rects[self.count] = Some(rect);
            self.count += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &DirtyRect> {
        self.rects[..self.count].iter().filter_map(|r| r.as_ref())
    }

    /// Clear all dirty rectangles in the framebuffer
    pub fn clear_all(&self, fb: &mut [u8], color: u8) {
        for rect in self.iter() {
            rect.clear(fb, color);
        }
    }
}

// =============================================================================
// Pre-computed Element Regions
// =============================================================================

/// Pre-computed regions for each overlay element
/// These are calculated once at startup based on layout constants
pub struct ElementRegions {
    // Right sidebar elements
    pub game_title: DirtyRect,
    pub trainer_name: DirtyRect,
    pub money: DirtyRect,
    pub pokedex: DirtyRect,
    pub party_header: DirtyRect,
    pub party_slots: [DirtyRect; 6],

    // Left sidebar elements
    pub location: DirtyRect,
    pub badges: DirtyRect,
    pub playtime: DirtyRect,

    // Bottom bar
    pub bottom_bar: DirtyRect,
}

impl ElementRegions {
    /// Calculate regions based on current layout
    pub fn calculate() -> Self {
        let right = Region::right_sidebar();
        let left = Region::left_sidebar();
        let padding = 4usize;
        let left_padding = 2usize;

        // Right sidebar: starts at right.x + padding
        let rx = right.x + padding;
        let rw = right.width - padding * 2;
        let mut ry = right.y + padding;

        let game_title = DirtyRect::new(rx, ry, rw, element::SECTION_HEADER);
        ry += element::SECTION_HEADER;

        let trainer_name = DirtyRect::new(rx, ry, rw, element::TEXT_4X6);
        ry += element::TEXT_4X6;

        let money = DirtyRect::new(rx, ry, rw, element::TEXT_4X6);
        ry += element::TEXT_4X6;

        ry += 2; // gap
        let pokedex = DirtyRect::new(rx, ry, rw, element::TEXT_4X6);
        ry += element::TEXT_4X6;

        ry += 4; // gap
        let party_header = DirtyRect::new(rx, ry, rw, element::TEXT_4X6);
        ry += element::TEXT_4X6;

        let mut party_slots = [DirtyRect::new(0, 0, 0, 0); 6];
        for slot in &mut party_slots {
            *slot = DirtyRect::new(rx, ry, rw, element::TEXT_4X6);
            ry += element::TEXT_4X6;
        }

        // Left sidebar
        let lx = left.x + left_padding;
        let lw = left.width - left_padding * 2;
        let mut ly = left.y + left_padding;

        // Location takes 3 lines
        let location = DirtyRect::new(lx, ly, lw, element::TEXT_4X6 * 3);
        ly += element::TEXT_4X6 * 3;

        ly += 4; // gap
        let badges = DirtyRect::new(lx, ly, lw, element::TEXT_4X6 * 2);
        ly += element::TEXT_4X6 * 2;

        ly += 4; // gap
        let playtime = DirtyRect::new(lx, ly, lw, element::TEXT_4X6);

        // Bottom bar: under GB screen
        let bottom_h = SCREEN_HEIGHT - GB_BOTTOM;
        let bottom_bar = DirtyRect::new(GB_X, GB_BOTTOM, GB_WIDTH, bottom_h);

        Self {
            game_title,
            trainer_name,
            money,
            pokedex,
            party_header,
            party_slots,
            location,
            badges,
            playtime,
            bottom_bar,
        }
    }

    /// Get dirty rects from dirty flags
    pub fn get_dirty_rects(&self, flags: &DirtyFlags) -> DirtyRects {
        let mut rects = DirtyRects::new();

        if flags.is_set(DirtyFlags::GAME_TITLE) {
            rects.add(self.game_title);
        }
        if flags.is_set(DirtyFlags::TRAINER_NAME) {
            rects.add(self.trainer_name);
        }
        if flags.is_set(DirtyFlags::MONEY) {
            rects.add(self.money);
        }
        if flags.is_set(DirtyFlags::POKEDEX) {
            rects.add(self.pokedex);
        }
        if flags.is_set(DirtyFlags::PARTY_HEADER) {
            rects.add(self.party_header);
        }

        for i in 0..6 {
            if flags.is_set(DirtyFlags::party_slot_flag(i)) {
                rects.add(self.party_slots[i]);
            }
        }

        if flags.is_set(DirtyFlags::LOCATION) {
            rects.add(self.location);
        }
        if flags.is_set(DirtyFlags::BADGES) {
            rects.add(self.badges);
        }
        if flags.is_set(DirtyFlags::PLAYTIME) {
            rects.add(self.playtime);
        }
        if flags.is_set(DirtyFlags::BOTTOM_BAR) {
            rects.add(self.bottom_bar);
        }

        rects
    }
}

// =============================================================================
// Global State (for bare-metal single-threaded use)
// =============================================================================

static mut OVERLAY_CACHE: OverlayCache = OverlayCache::new();
static mut ELEMENT_REGIONS: Option<ElementRegions> = None;

/// Initialize the dirty tracking system
/// Call once at startup after layout is configured
pub fn init() {
    unsafe {
        ELEMENT_REGIONS = Some(ElementRegions::calculate());
        OVERLAY_CACHE = OverlayCache::new();
    }
}

/// Get the global overlay cache
pub fn cache() -> &'static mut OverlayCache {
    unsafe { &mut OVERLAY_CACHE }
}

/// Get the element regions
pub fn regions() -> &'static ElementRegions {
    unsafe {
        ELEMENT_REGIONS
            .as_ref()
            .expect("dirty_region::init() not called")
    }
}

/// Force full redraw on next frame
pub fn invalidate_all() {
    unsafe {
        OVERLAY_CACHE.invalidate();
    }
}

/// Check if system is initialized
pub fn is_initialized() -> bool {
    unsafe { ELEMENT_REGIONS.is_some() }
}
