//! Storage Hardware Abstraction
//!
//! Abstracts the differences between:
//! - x86: ATA/IDE with PIO mode
//! - ARM: EMMC/SD card with SDIO

/// Block device trait for raw sector access
pub trait BlockDevice {
    /// Sector size in bytes (typically 512)
    const SECTOR_SIZE: usize = 512;
    
    /// Read sectors from device
    /// 
    /// # Arguments
    /// * `lba` - Logical block address (sector number)
    /// * `count` - Number of sectors to read
    /// * `buffer` - Buffer to read into (must be at least count * SECTOR_SIZE bytes)
    fn read_sectors(&self, lba: u64, count: u32, buffer: &mut [u8]) -> Result<(), &'static str>;
    
    /// Write sectors to device
    fn write_sectors(&self, lba: u64, count: u32, buffer: &[u8]) -> Result<(), &'static str>;
    
    /// Get total number of sectors
    fn sector_count(&self) -> u64;
    
    /// Check if device is present and ready
    fn is_ready(&self) -> bool;
}

/// Filesystem trait for FAT32 operations
pub trait Filesystem {
    /// Mount the filesystem
    fn mount(&mut self) -> Result<(), &'static str>;
    
    /// Check if filesystem is mounted
    fn is_mounted(&self) -> bool;
    
    /// Count ROM files (.gb, .gbc) in root directory
    fn count_roms(&self) -> usize;
    
    /// Get ROM filename at index
    fn get_rom_name(&self, index: usize, buffer: &mut [u8; 12]) -> bool;
    
    /// Find ROM file and return (cluster, size)
    fn find_rom(&self, index: usize) -> Option<(u32, u32)>;
    
    /// Read file data into buffer
    fn read_file(&self, cluster: u32, size: u32, buffer: &mut [u8]) -> Result<usize, &'static str>;
}

/// Save file operations
pub trait SaveStorage {
    /// Save SRAM data for a game
    fn save_sram(&self, rom_name: &str, data: &[u8]) -> Result<(), &'static str>;
    
    /// Load SRAM data for a game
    fn load_sram(&self, rom_name: &str, buffer: &mut [u8]) -> Result<usize, &'static str>;
    
    /// Check if save exists for a game
    fn has_save(&self, rom_name: &str) -> bool;
}
