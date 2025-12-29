//! Overlay Module
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
//! - `game_overlay`: Rendering logic for overlays
//! - `dirty_region`: Dirty region tracking for efficient updates (NEW)
//! - `pokemon_names`: Pokemon species name lookup (251 Pokemon)
//! - `move_names`: Move name lookup (251 moves)
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
pub mod dirty_region;  // NEW
pub mod pokemon_names;
pub mod move_names;

// Re-export RAM layout types
pub use ram_layout::{
    decode_text, BattlePokemon, Game, PartyState, Pokemon,
    PokemonType, RamReader, StatusCondition, TrainerData
};

// Re-export overlay renderer and functions
pub use game_overlay::{
    init_overlay,              // NEW
    invalidate_overlay,        // NEW
    is_game_supported,
    render_overlay,            // Legacy (full redraw)
    render_overlay_efficient,  // NEW (dirty tracking)
    OverlayConfig,
    OverlayRenderer,
};

// Re-export name lookups
pub use move_names::get_move_name;
pub use pokemon_names::get_name as get_pokemon_name;
