//! Overlay Module - Enhanced Version
//!
//! Provides game-aware overlays for streaming and debugging.
//! Reads game state directly from emulator RAM without side effects.
//!
//! # Supported Games
//! - Gen 1: Pokemon Red, Blue, Yellow
//! - Gen 2: Pokemon Gold, Silver, Crystal
//!
//! # Components
//! - `ram_layout`: Memory addresses and data structures for supported games
//! - `game_overlay`: Rendering logic for overlays (three-panel layout)
//! - `dirty_region`: Dirty region tracking for efficient updates
//! - `pokemon_names`: Pokemon species name lookup (251 Pokemon)
//! - `move_names`: Move name lookup (251 moves)
//! - `map_names`: Map/location name lookup (Gen 1 & Gen 2)
//! - `item_names`: Item name lookup for bag display
//! - `move_pp`: Move PP data (base PP, max PP calculation)
//! - `catch_rate`: Pokemon catch rate data for wild battle display
//!
//! # Layout
//!
//! ```text
//! +--------+------------------+--------+
//! | LEFT   |                  | RIGHT  |
//! | PANEL  |    GAME BOY      | PANEL  |
//! |        |     SCREEN       |        |
//! | Map    |                  | Player |
//! | Badges |                  | Name   |
//! | Lead   |                  | Money  |
//! | Pokemon|                  | Bag    |
//! | -Name  |                  |        |
//! | -Moves |                  | (or in |
//! | -Stats |                  | battle:|
//! |        |                  | Enemy) |
//! +--------+------------------+--------+
//! |        | BOTTOM: Party HP bars     |
//! +--------+---------------------------+
//! ```
//!
//! # Usage (with Double Buffering)
//!
//! ```rust,ignore
//! use crate::overlay::{Game, RamReader, render_overlay_efficient, init_overlay};
//! use crate::graphics::double_buffer;
//!
//! // Initialize once at startup
//! double_buffer::init();
//! init_overlay();
//!
//! // Detect game from ROM name
//! let game = Game::detect(&device.romname());
//!
//! // In main loop:
//! double_buffer::blit_gb_to_backbuffer(device.get_pal_data());
//! let reader = RamReader::new(device.mmu(), game);
//! render_overlay_efficient(double_buffer::back_buffer(), &reader, game);
//! double_buffer::flip_vsync();
//! ```

pub mod ram_layout;
pub mod game_overlay;
pub mod dirty_region;
pub mod pokemon_names;
pub mod move_names;
pub mod map_names;      // NEW
pub mod item_names;     // NEW
pub mod move_pp;        // NEW
pub mod catch_rate;     // NEW

// Re-export RAM layout types
pub use ram_layout::{
    decode_text, BattlePokemon, Game, PartyState, Pokemon,
    PokemonType, RamReader, StatusCondition, TrainerData
};

// Re-export overlay renderer and functions
pub use game_overlay::{
    init_overlay,
    invalidate_overlay,
    is_game_supported,
    render_overlay,            // Legacy (full redraw)
    render_overlay_efficient,  // Optimized
    OverlayConfig,
    OverlayRenderer,
};

// Re-export name lookups
pub use move_names::get_move_name;
pub use pokemon_names::get_name as get_pokemon_name;
pub use map_names::{get_gen1_map_name, get_gen2_map_name};
pub use item_names::{get_gen1_item_name, get_gen2_item_name};
pub use move_pp::{get_base_pp, get_max_pp, get_actual_max_pp};
pub use catch_rate::{get_catch_rate, get_catch_tier};
