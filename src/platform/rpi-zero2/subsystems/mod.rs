//! Subsystem Modules
//!
//! FAT32 filesystem and input handling.
//! Depends on: drivers

pub mod fat32;
pub mod input;
#[path = "../../../rom_selector/mod.rs"]
pub(crate) mod rom_selector;

// Now use it
use rom_selector::{run_selector, Selection};

// Re-exports for convenience
pub use fat32::{Fat32, RomEntry, DirEnumerator, Fat32FileSystem, MAX_FILENAME_LEN};
pub use input::{GpiButtonState, GbJoypad, RomSelectorInput, button};
