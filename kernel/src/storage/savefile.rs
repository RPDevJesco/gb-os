//! Save File Management
//!
//! Persists Game Boy cartridge SRAM to disk so game saves survive power cycles.
//! Uses a dedicated save area on disk (bypasses filesystem for simplicity).
//!
//! # How It Works
//!
//! 1. Game writes to cartridge RAM (0xA000-0xBFFF) - this is the game "saving"
//! 2. We periodically dump that RAM to disk sectors
//! 3. On ROM load, we restore any existing save back to RAM
//!
//! # Disk Layout
//!
//! Save area starts at sector 0x10000 (32MB offset):
//! - Slot 0: Sectors 0x10000-0x1003F (32KB)
//! - Slot 1: Sectors 0x10040-0x1007F
//! - Up to 16 ROM saves supported
//!
//! Each slot has a header sector followed by raw SRAM data.

extern crate alloc;

use alloc::vec::Vec;
use crate::arch::x86::io::{inb, outb, inw, outw};
use crate::storage::ata::{self, AtaDevice, Channel, Drive, cmd, status};

// =============================================================================
// Constants
// =============================================================================

/// Start sector for save area (32MB into disk - safe offset)
const SAVE_AREA_START: u64 = 0x10000;

/// Sectors per save slot (64 * 512 = 32KB)
const SECTORS_PER_SLOT: u64 = 64;

/// Maximum save slots
const MAX_SAVE_SLOTS: usize = 16;

/// Magic bytes for save header
const SAVE_MAGIC: [u8; 4] = [b'G', b'B', b'S', b'V'];

/// Sector size
const SECTOR_SIZE: usize = 512;

// =============================================================================
// Save Header
// =============================================================================

/// Save file header (first sector of save slot)
#[repr(C)]
pub struct SaveHeader {
    /// Magic bytes "GBSV"
    pub magic: [u8; 4],
    /// ROM name hash for verification
    pub rom_hash: u32,
    /// RAM size in bytes
    pub ram_size: u32,
    /// Timestamp (PIT ticks)
    pub timestamp: u32,
    /// ROM name (16 bytes, null-padded)
    pub rom_name: [u8; 16],
    /// Reserved for future use
    pub reserved: [u8; 480],
}

impl SaveHeader {
    /// Create a new save header
    pub fn new(rom_name: &str, ram_size: usize) -> Self {
        let mut header = SaveHeader {
            magic: SAVE_MAGIC,
            rom_hash: hash_rom_name(rom_name),
            ram_size: ram_size as u32,
            timestamp: crate::arch::x86::pit::ticks(),
            rom_name: [0u8; 16],
            reserved: [0u8; 480],
        };

        // Copy ROM name
        let name_bytes = rom_name.as_bytes();
        let copy_len = name_bytes.len().min(16);
        header.rom_name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

        header
    }

    /// Check if header is valid
    pub fn is_valid(&self) -> bool {
        self.magic == SAVE_MAGIC && self.ram_size > 0 && self.ram_size <= 0x8000
    }

    /// Check if this save matches a ROM
    pub fn matches_rom(&self, rom_name: &str) -> bool {
        self.is_valid() && self.rom_hash == hash_rom_name(rom_name)
    }

    /// Convert to bytes
    pub fn to_bytes(&self) -> [u8; SECTOR_SIZE] {
        let mut bytes = [0u8; SECTOR_SIZE];
        bytes[0..4].copy_from_slice(&self.magic);
        bytes[4..8].copy_from_slice(&self.rom_hash.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.ram_size.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.timestamp.to_le_bytes());
        bytes[16..32].copy_from_slice(&self.rom_name);
        bytes
    }

    /// Parse from bytes
    pub fn from_bytes(bytes: &[u8; SECTOR_SIZE]) -> Self {
        let mut header = SaveHeader {
            magic: [0u8; 4],
            rom_hash: 0,
            ram_size: 0,
            timestamp: 0,
            rom_name: [0u8; 16],
            reserved: [0u8; 480],
        };

        header.magic.copy_from_slice(&bytes[0..4]);
        header.rom_hash = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        header.ram_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        header.timestamp = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        header.rom_name.copy_from_slice(&bytes[16..32]);

        header
    }
}

// =============================================================================
// Hash Function
// =============================================================================

/// Simple hash function for ROM names
fn hash_rom_name(name: &str) -> u32 {
    let mut hash: u32 = 0x811c9dc5; // FNV-1a offset basis
    for byte in name.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193); // FNV prime
    }
    hash
}

// =============================================================================
// ATA Write Implementation
// =============================================================================

/// Write sectors to an ATA device
///
/// - `device`: Device to write to
/// - `lba`: Starting logical block address
/// - `count`: Number of sectors to write (1-256, 0 means 256)
/// - `buffer`: Buffer containing data to write (must be count * 512 bytes)
pub fn write_sectors(
    device: &AtaDevice,
    lba: u64,
    count: u8,
    buffer: &[u8]
) -> Result<usize, &'static str> {
    use crate::storage::ata::DeviceType;

    if device.device_type != DeviceType::Ata {
        return Err("Not an ATA device");
    }

    let sector_size = device.sector_size as usize;
    let actual_count = if count == 0 { 256 } else { count as usize };

    if buffer.len() < actual_count * sector_size {
        return Err("Buffer too small");
    }

    // Check LBA range
    if lba >= device.sectors {
        return Err("LBA out of range");
    }

    let base = device.channel.base_port();

    // Select drive
    if !select_drive(device.channel, device.drive) {
        return Err("Drive select timeout");
    }

    // Wait for drive ready
    if !wait_ready(device.channel, 100_000) {
        return Err("Drive not ready");
    }

    unsafe {
        // LBA28 mode (sufficient for our save area)
        let drive_byte = device.drive.select_byte() | ((lba >> 24) & 0x0F) as u8;
        outb(base + 6, drive_byte);
        outb(base + 2, count);
        outb(base + 3, (lba & 0xFF) as u8);
        outb(base + 4, ((lba >> 8) & 0xFF) as u8);
        outb(base + 5, ((lba >> 16) & 0xFF) as u8);
        outb(base + 7, cmd::WRITE_SECTORS);
    }

    // Write each sector
    let mut offset = 0;
    for _ in 0..actual_count {
        // Wait for DRQ
        if !wait_drq(device.channel, 100_000) {
            return Err("Timeout waiting for DRQ");
        }

        // Write sector data (256 words = 512 bytes)
        let words = sector_size / 2;
        for _ in 0..words {
            let word = (buffer[offset] as u16) | ((buffer[offset + 1] as u16) << 8);
            unsafe { outw(base, word); }
            offset += 2;
        }

        // Small delay between sectors
        io_delay(device.channel);
    }

    // Flush cache
    unsafe {
        outb(base + 7, cmd::FLUSH_CACHE);
    }

    // Wait for completion
    if !wait_ready(device.channel, 500_000) {
        return Err("Flush timeout");
    }

    Ok(actual_count)
}

// Helper functions (reuse from ata module or copy here)

fn select_drive(channel: Channel, drive: Drive) -> bool {
    let base = channel.base_port();

    unsafe {
        outb(base + 6, drive.select_byte());
    }

    // Wait for drive to be selected (read status 15 times as delay)
    for _ in 0..15 {
        io_delay(channel);
    }

    // Check BSY cleared
    for _ in 0..10000 {
        let s = unsafe { inb(channel.control_port()) };
        if s & status::BSY == 0 {
            return true;
        }
    }

    false
}

fn wait_ready(channel: Channel, timeout: u32) -> bool {
    for _ in 0..timeout {
        let s = unsafe { inb(channel.control_port()) };
        if s & status::BSY == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn wait_drq(channel: Channel, timeout: u32) -> bool {
    for _ in 0..timeout {
        let s = unsafe { inb(channel.control_port()) };
        if s & status::BSY == 0 {
            if s & status::DRQ != 0 {
                return true;
            }
            if s & (status::ERR | status::DF) != 0 {
                return false;
            }
        }
        core::hint::spin_loop();
    }
    false
}

fn io_delay(channel: Channel) {
    // Read alternate status register 4 times for 400ns delay
    for _ in 0..4 {
        unsafe { inb(channel.control_port()); }
    }
}

// =============================================================================
// Save/Load Operations
// =============================================================================

/// Save state result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveResult {
    Success,
    NoDevice,
    NoBattery,
    WriteError,
    InvalidData,
}

/// Load state result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadResult {
    Success,
    NoDevice,
    NoSaveFound,
    ReadError,
    HashMismatch,
    SizeMismatch,
}

/// Find save slot for a ROM (returns slot index if found)
pub fn find_save_slot(rom_name: &str) -> Option<usize> {
    let device = ata::find_ata_disk()?;
    let target_hash = hash_rom_name(rom_name);

    let mut sector = [0u8; SECTOR_SIZE];

    for slot in 0..MAX_SAVE_SLOTS {
        let lba = SAVE_AREA_START + (slot as u64 * SECTORS_PER_SLOT);

        if ata::read_sectors(device, lba, 1, &mut sector).is_ok() {
            let header = SaveHeader::from_bytes(&sector);
            if header.is_valid() && header.rom_hash == target_hash {
                return Some(slot);
            }
        }
    }

    None
}

/// Find first empty save slot
fn find_empty_slot() -> Option<usize> {
    let device = ata::find_ata_disk()?;

    let mut sector = [0u8; SECTOR_SIZE];

    for slot in 0..MAX_SAVE_SLOTS {
        let lba = SAVE_AREA_START + (slot as u64 * SECTORS_PER_SLOT);

        if ata::read_sectors(device, lba, 1, &mut sector).is_ok() {
            let header = SaveHeader::from_bytes(&sector);
            if !header.is_valid() {
                return Some(slot);
            }
        } else {
            // Read failed - might be uninitialized, use it
            return Some(slot);
        }
    }

    // All slots full - overwrite oldest (slot 0 for simplicity)
    Some(0)
}

/// Save game RAM to disk
pub fn save_game(rom_name: &str, ram_data: &[u8]) -> SaveResult {
    // Get device
    let device = match ata::find_ata_disk() {
        Some(d) => d,
        None => return SaveResult::NoDevice,
    };

    if ram_data.is_empty() {
        return SaveResult::InvalidData;
    }

    // Find or allocate slot
    let slot = find_save_slot(rom_name).or_else(|| find_empty_slot());
    let slot = match slot {
        Some(s) => s,
        None => return SaveResult::WriteError,
    };

    let base_lba = SAVE_AREA_START + (slot as u64 * SECTORS_PER_SLOT);

    // Create and write header
    let header = SaveHeader::new(rom_name, ram_data.len());
    let header_bytes = header.to_bytes();

    if write_sectors(device, base_lba, 1, &header_bytes).is_err() {
        return SaveResult::WriteError;
    }

    // Write RAM data
    let sectors_needed = (ram_data.len() + SECTOR_SIZE - 1) / SECTOR_SIZE;
    let mut padded_data = alloc::vec![0u8; sectors_needed * SECTOR_SIZE];
    padded_data[..ram_data.len()].copy_from_slice(ram_data);

    for (i, chunk) in padded_data.chunks(SECTOR_SIZE).enumerate() {
        let lba = base_lba + 1 + i as u64;
        let mut sector = [0u8; SECTOR_SIZE];
        sector[..chunk.len()].copy_from_slice(chunk);

        if write_sectors(device, lba, 1, &sector).is_err() {
            return SaveResult::WriteError;
        }
    }

    SaveResult::Success
}

/// Load game RAM from disk
pub fn load_game(rom_name: &str, ram_buffer: &mut [u8]) -> LoadResult {
    // Get device
    let device = match ata::find_ata_disk() {
        Some(d) => d,
        None => return LoadResult::NoDevice,
    };

    // Find save slot
    let slot = match find_save_slot(rom_name) {
        Some(s) => s,
        None => return LoadResult::NoSaveFound,
    };

    let base_lba = SAVE_AREA_START + (slot as u64 * SECTORS_PER_SLOT);

    // Read and verify header
    let mut sector = [0u8; SECTOR_SIZE];
    if ata::read_sectors(device, base_lba, 1, &mut sector).is_err() {
        return LoadResult::ReadError;
    }

    let header = SaveHeader::from_bytes(&sector);

    if !header.matches_rom(rom_name) {
        return LoadResult::HashMismatch;
    }

    let save_size = header.ram_size as usize;
    if save_size > ram_buffer.len() {
        return LoadResult::SizeMismatch;
    }

    // Read RAM data
    let sectors_needed = (save_size + SECTOR_SIZE - 1) / SECTOR_SIZE;
    let mut offset = 0;

    for i in 0..sectors_needed {
        let lba = base_lba + 1 + i as u64;

        if ata::read_sectors(device, lba, 1, &mut sector).is_err() {
            return LoadResult::ReadError;
        }

        let copy_len = (save_size - offset).min(SECTOR_SIZE);
        ram_buffer[offset..offset + copy_len].copy_from_slice(&sector[..copy_len]);
        offset += copy_len;
    }

    LoadResult::Success
}

/// Check if a save exists for a ROM
pub fn has_save(rom_name: &str) -> bool {
    find_save_slot(rom_name).is_some()
}

/// Delete save for a ROM
pub fn delete_save(rom_name: &str) -> bool {
    let device = match ata::find_ata_disk() {
        Some(d) => d,
        None => return false,
    };

    let slot = match find_save_slot(rom_name) {
        Some(s) => s,
        None => return false,
    };

    let lba = SAVE_AREA_START + (slot as u64 * SECTORS_PER_SLOT);

    // Zero out the header to invalidate the slot
    let zeros = [0u8; SECTOR_SIZE];
    write_sectors(device, lba, 1, &zeros).is_ok()
}

// =============================================================================
// Device Integration - convenience functions for use with gameboy::Device
// =============================================================================

use crate::gameboy::Device;

/// Save the current cartridge RAM to disk
/// Call this periodically or when the game signals a save
pub fn save_sram(device: &Device) -> SaveResult {
    if !device.ram_is_battery_backed() {
        return SaveResult::NoBattery;
    }

    let rom_name = device.romname();
    let ram_data = device.dumpram();

    if ram_data.is_empty() {
        return SaveResult::InvalidData;
    }

    save_game(&rom_name, &ram_data)
}

/// Load saved RAM into the cartridge
/// Call this after creating the Device but before starting emulation
pub fn load_sram(device: &mut Device) -> LoadResult {
    if !device.ram_is_battery_backed() {
        return LoadResult::NoSaveFound;
    }

    let rom_name = device.romname();

    // Allocate buffer for max possible RAM size (32KB)
    let mut ram_buffer = alloc::vec![0u8; 0x8000];

    match load_game(&rom_name, &mut ram_buffer) {
        LoadResult::Success => {
            // Load into device - the MBC will validate size
            match device.loadram(&ram_buffer) {
                Ok(_) => LoadResult::Success,
                Err(_) => LoadResult::SizeMismatch,
            }
        }
        other => other,
    }
}

/// Check if a save file exists for this ROM
pub fn has_save_for(device: &Device) -> bool {
    has_save(&device.romname())
}

/// Debounced save state tracker
/// When the game writes to SRAM, we wait for writes to settle before persisting
pub struct SaveTracker {
    /// Frames since last RAM write detected
    frames_since_write: u32,
    /// Whether we're waiting to save (RAM was modified)
    pending_save: bool,
}

/// How many frames to wait after last write before persisting
/// ~2 seconds at 60fps - gives time for save operation to complete
const SAVE_DEBOUNCE_FRAMES: u32 = 120;

impl SaveTracker {
    pub const fn new() -> Self {
        Self {
            frames_since_write: 0,
            pending_save: false,
        }
    }

    /// Call this every frame. Returns true if a save should be performed now.
    ///
    /// Logic:
    /// 1. If game wrote to RAM this frame, reset debounce timer
    /// 2. If we have a pending save and timer expired, trigger save
    pub fn tick(&mut self, device: &mut Device) -> bool {
        if !device.ram_is_battery_backed() {
            return false;
        }

        // Check if RAM was written this frame
        if device.check_and_reset_ram_updated() {
            // RAM was just written - start/reset debounce timer
            self.frames_since_write = 0;
            self.pending_save = true;
            return false;
        }

        // If we have a pending save, count frames
        if self.pending_save {
            self.frames_since_write += 1;

            // If enough time passed with no writes, save now
            if self.frames_since_write >= SAVE_DEBOUNCE_FRAMES {
                self.pending_save = false;
                self.frames_since_write = 0;
                return true;
            }
        }

        false
    }
}

/// Call every frame - handles debounced saving automatically
/// Returns true if a save was performed
pub fn update(tracker: &mut SaveTracker, device: &mut Device) -> bool {
    if tracker.tick(device) {
        save_sram(device) == SaveResult::Success
    } else {
        false
    }
}
