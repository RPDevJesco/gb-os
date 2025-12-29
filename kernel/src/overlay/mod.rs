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
//! - `pokemon_names`: Pokemon species name lookup (251 Pokemon)
//! - `move_names`: Move name lookup (251 moves)
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::overlay::{Game, RamReader, render_overlay};
//!
//! // Detect game from ROM name
//! let game = Game::detect(&device.romname());
//!
//! // Create reader (borrows MMU immutably via peek)
//! let reader = RamReader::new(&device.cpu.mmu, game);
//!
//! // Render to framebuffer
//! render_overlay(&mut framebuffer, &reader, game);
//! ```
//!
//! # Game Detection
//!
//! The `Game::detect()` function reads the ROM title from the cartridge header
//! and matches against known game titles:
//!
//! - "POKEMON RED" → `Game::Red`
//! - "POKEMON BLUE" → `Game::Blue`
//! - "POKEMON YELLOW" → `Game::Yellow`
//! - "POKEMON GOLD" → `Game::Gold`
//! - "POKEMON SILVER" → `Game::Silver`
//! - "POKEMON CRYSTAL" or "PM_CRYSTAL" → `Game::Crystal`

pub mod ram_layout;
pub mod game_overlay;
pub mod pokemon_names;
pub mod move_names;

// Re-export RAM layout types
pub use ram_layout::{Game, Pokemon, BattlePokemon, PartyState, TrainerData, RamReader};
pub use ram_layout::{decode_text, StatusCondition, PokemonType};

// Re-export overlay renderer
pub use game_overlay::{OverlayRenderer, OverlayConfig, render_overlay, is_game_supported};

// Re-export name lookups
pub use pokemon_names::get_name as get_pokemon_name;
pub use move_names::get_move_name;
