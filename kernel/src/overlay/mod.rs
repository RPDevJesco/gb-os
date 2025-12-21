//! Overlay Module
//!
//! Provides game-aware overlays for streaming and debugging.
//! Reads game state directly from emulator RAM without side effects.
//!
//! # Supported Games
//! - Pokemon Yellow (Gen 1)
//! - Pokemon Crystal (Gen 2)
//!
//! # Components
//! - `ram_layout`: Memory addresses and data structures for supported games
//! - `game_overlay`: Rendering logic for overlays
//! - `sprites`: Embedded sprite data for Pokemon, items, badges (TODO)
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::gameboy::overlay::{Game, RamReader, render_overlay};
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

pub mod ram_layout;
pub mod game_overlay;
// pub mod sprites; // TODO

// Re-export RAM layout types
pub use ram_layout::{Game, Pokemon, PartyState, TrainerData, RamReader};
pub use ram_layout::{decode_text, StatusCondition, PokemonType};

// Re-export overlay renderer
pub use game_overlay::{OverlayRenderer, OverlayConfig, render_overlay, is_game_supported};
pub use game_overlay::colors;
