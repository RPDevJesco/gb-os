pub mod menu;

/// Maximum filename length we support
pub const MAX_FILENAME_LEN: usize = 128;

/// Maximum ROMs visible at once (for pagination)
pub const PAGE_SIZE: usize = 16;

/// A ROM entry with fixed-size filename buffer
#[derive(Clone, Copy)]
pub struct RomEntry {
    /// Filename (null-terminated or full)
    pub name: [u8; MAX_FILENAME_LEN],
    /// Actual length of name
    pub name_len: usize,
    /// Starting cluster (for FAT32)
    pub cluster: u32,
    /// File size in bytes
    pub size: u32,
    /// Is Game Boy Color ROM
    pub is_gbc: bool,
}

impl RomEntry {
    /// Create an empty ROM entry
    pub const fn empty() -> Self {
        Self {
            name: [0u8; MAX_FILENAME_LEN],
            name_len: 0,
            cluster: 0,
            size: 0,
            is_gbc: false,
        }
    }

    /// Get filename as string slice
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("<invalid>")
    }
}

/// Result of ROM selection
#[derive(Clone, Copy)]
pub struct Selection {
    pub cluster: u32,
    pub size: u32,
}

/// Filesystem abstraction
pub trait FileSystem {
    /// Reset enumeration to beginning of ROM directory
    fn reset_enumeration(&mut self);

    /// Get next ROM entry (.gb/.gbc files only)
    /// Returns false when no more files
    fn next_rom(&mut self, entry: &mut RomEntry) -> bool;
}

/// Display abstraction
pub trait Display {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn clear(&mut self, color: u32);
    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32);
    fn draw_text(&mut self, x: u32, y: u32, text: &[u8], fg: u32, bg: u32);
    fn present(&mut self);
    fn wait_vblank(&self);
}

/// Input abstraction
pub trait Input {
    fn poll(&mut self) -> ButtonEvent;
}

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonEvent {
    None,
    Up,
    Down,
    Left,
    Right,
    Select,
    Back,
}

// Re-export menu entry point
pub use menu::run_selector;
