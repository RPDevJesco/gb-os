//! Storage Subsystem
//!
//! Integrates PCI enumeration and ATA drivers to provide disk access.
//! Includes safe initialization with extensive debug output.

pub mod pci;
pub mod ata;
pub mod fat32;

use crate::arch::x86::io::outb;

// =============================================================================
// Debug Output
// =============================================================================

/// Draw colored debug bar at specified row
fn debug_bar(row: usize, stage: usize, color: u8) {
    unsafe {
        let vga = 0xA0000 as *mut u8;
        let row_offset = row * 320;
        let start = row_offset + stage * 12;
        for i in 0..10 {
            core::ptr::write_volatile(vga.add(start + i), color);
        }
    }
}

/// Draw a character at position (for simple text output in mode 13h)
fn debug_char(x: usize, y: usize, ch: u8, color: u8) {
    // Simple 8x8 font rendering would go here
    // For now, just draw a colored pixel
    unsafe {
        let vga = 0xA0000 as *mut u8;
        let offset = y * 320 + x;
        core::ptr::write_volatile(vga.add(offset), color);
    }
}

/// Show a hex value as colored pixels (debug)
fn debug_hex(row: usize, value: u32) {
    unsafe {
        let vga = 0xA0000 as *mut u8;
        let row_offset = row * 320 + 200;  // Right side of screen
        for i in 0..8 {
            let nibble = ((value >> (28 - i * 4)) & 0xF) as u8;
            let color = if nibble < 10 { 0x02 + nibble } else { 0x04 + nibble - 10 };
            for j in 0..3 {
                core::ptr::write_volatile(vga.add(row_offset + i * 4 + j), color);
            }
        }
    }
}

// =============================================================================
// Color Constants (VGA Mode 13h Palette)
// =============================================================================

mod colors {
    pub const BLACK: u8 = 0x00;
    pub const BLUE: u8 = 0x01;
    pub const GREEN: u8 = 0x02;
    pub const CYAN: u8 = 0x03;
    pub const RED: u8 = 0x04;
    pub const MAGENTA: u8 = 0x05;
    pub const BROWN: u8 = 0x06;
    pub const LIGHT_GRAY: u8 = 0x07;
    pub const DARK_GRAY: u8 = 0x08;
    pub const LIGHT_BLUE: u8 = 0x09;
    pub const LIGHT_GREEN: u8 = 0x0A;
    pub const LIGHT_CYAN: u8 = 0x0B;
    pub const LIGHT_RED: u8 = 0x0C;
    pub const LIGHT_MAGENTA: u8 = 0x0D;
    pub const YELLOW: u8 = 0x0E;
    pub const WHITE: u8 = 0x0F;
}

// =============================================================================
// Initialization
// =============================================================================

/// Storage initialization result
pub struct StorageInitResult {
    /// Number of PCI devices found
    pub pci_devices: usize,
    /// Whether IDE controller was found
    pub ide_found: bool,
    /// Number of ATA devices found
    pub ata_devices: usize,
    /// Error stage (0 = no error)
    pub error_stage: u8,
    /// Error message
    pub error_msg: Option<&'static str>,
}

/// Initialize storage subsystem safely with debug output
///
/// Debug bar layout (row 5):
/// - Bar 0: PCI init start (yellow) -> done (green/red)
/// - Bar 1: IDE search (yellow) -> found (green/red)
/// - Bar 2: ATA init (yellow) -> done (green/red)
/// - Bar 3: Overall status
pub fn init() -> StorageInitResult {
    let mut result = StorageInitResult {
        pci_devices: 0,
        ide_found: false,
        ata_devices: 0,
        error_stage: 0,
        error_msg: None,
    };

    // Stage 0: Starting
    debug_bar(5, 0, colors::YELLOW);

    // Stage 1: PCI Enumeration
    debug_bar(5, 1, colors::YELLOW);

    result.pci_devices = pci::enumerate();

    if result.pci_devices == 0 {
        debug_bar(5, 1, colors::RED);
        result.error_stage = 1;
        result.error_msg = Some("No PCI devices found");
        // Continue anyway - might work with legacy ports
    } else {
        debug_bar(5, 1, colors::GREEN);
    }

    // Show PCI device count
    debug_hex(5, result.pci_devices as u32);

    // Stage 2: Find IDE Controller
    debug_bar(5, 2, colors::YELLOW);

    if let Some(ide) = pci::find_ide_controller() {
        result.ide_found = true;
        debug_bar(5, 2, colors::GREEN);

        // Show IDE controller info
        debug_hex(6, ((ide.vendor_id as u32) << 16) | ide.device_id as u32);
    } else {
        debug_bar(5, 2, colors::BROWN);  // Brown = not found but continue
        // IDE might still work at legacy ports without PCI detection
    }

    // Stage 3: ATA Device Detection
    debug_bar(5, 3, colors::YELLOW);

    result.ata_devices = ata::init();

    if result.ata_devices == 0 {
        debug_bar(5, 3, colors::BROWN);  // Brown = no devices (not fatal)
    } else {
        debug_bar(5, 3, colors::GREEN);
    }

    // Show ATA device count
    debug_hex(7, result.ata_devices as u32);

    // Stage 4: Overall status
    if result.ata_devices > 0 {
        debug_bar(5, 4, colors::GREEN);
    } else if result.pci_devices > 0 {
        debug_bar(5, 4, colors::YELLOW);  // PCI works but no disks
    } else {
        debug_bar(5, 4, colors::RED);
    }

    // Mark init complete
    debug_bar(5, 0, colors::GREEN);

    result
}

/// Safe test function - reads first sector from first ATA device
/// Returns true if successful
pub fn test_read() -> bool {
    debug_bar(8, 0, colors::YELLOW);  // Starting test

    // Find an ATA device
    let device = match ata::find_ata_disk() {
        Some(d) => d,
        None => {
            debug_bar(8, 0, colors::RED);
            return false;
        }
    };

    debug_bar(8, 1, colors::YELLOW);  // Found device

    // Read sector 0 (MBR)
    let mut buffer = [0u8; 512];

    match ata::read_sectors(device, 0, 1, &mut buffer) {
        Ok(_) => {
            debug_bar(8, 1, colors::GREEN);

            // Verify MBR signature (0x55AA at offset 510-511)
            if buffer[510] == 0x55 && buffer[511] == 0xAA {
                debug_bar(8, 2, colors::GREEN);  // Valid MBR

                // Show first 4 bytes of MBR
                let first_dword = (buffer[0] as u32)
                    | ((buffer[1] as u32) << 8)
                    | ((buffer[2] as u32) << 16)
                    | ((buffer[3] as u32) << 24);
                debug_hex(8, first_dword);
            } else {
                debug_bar(8, 2, colors::YELLOW);  // Read worked but not MBR
                debug_hex(8, (buffer[510] as u32) << 8 | buffer[511] as u32);
            }

            true
        }
        Err(_) => {
            debug_bar(8, 1, colors::RED);
            false
        }
    }
}

// =============================================================================
// Information Functions
// =============================================================================

/// Get total number of detected storage devices
pub fn device_count() -> usize {
    ata::device_count()
}

/// Print device info to debug display
pub fn show_device_info() {
    for i in 0..4 {
        if let Some(device) = ata::get_device(i) {
            let row = 10 + i;

            // Device type indicator
            let type_color = match device.device_type {
                ata::DeviceType::Ata => colors::GREEN,
                ata::DeviceType::Atapi => colors::CYAN,
                _ => colors::DARK_GRAY,
            };
            debug_bar(row, 0, type_color);

            // Capacity (show as colored bars)
            let capacity_mb = device.capacity_mb();
            let bars = (capacity_mb / 1024).min(15) as usize; // 1 bar per GB, max 15
            for b in 0..bars {
                debug_bar(row, 1 + b, colors::LIGHT_BLUE);
            }
        }
    }
}

// =============================================================================
// High-Level API
// =============================================================================

/// Read sectors from device index
pub fn read_sectors(
    device_index: usize,
    lba: u64,
    count: u8,
    buffer: &mut [u8]
) -> Result<usize, &'static str> {
    let device = ata::get_device(device_index)
        .ok_or("Invalid device index")?;

    ata::read_sectors(device, lba, count, buffer)
}

// =============================================================================
// FAT32 Integration
// =============================================================================

/// Mount FAT32 filesystem from first ATA device
/// Returns true if successful
pub fn mount_fat32() -> bool {
    // Debug: show we're starting
    debug_bar(9, 0, colors::YELLOW);  // Starting mount

    // Show device count
    let count = ata::device_count();
    debug_hex(11, count as u32);

    // Find first ATA device index
    let device_index = match find_ata_device_index() {
        Some(idx) => {
            debug_bar(9, 0, colors::GREEN);  // Found device
            debug_hex(12, idx as u32);  // Show which index
            idx
        }
        None => {
            debug_bar(9, 0, colors::RED);  // No ATA device found
            return false;
        }
    };

    debug_bar(9, 1, colors::YELLOW);  // Found device, attempting mount

    match fat32::mount(device_index) {
        Ok(()) => {
            debug_bar(9, 1, colors::GREEN);
            debug_bar(9, 2, colors::GREEN);
            true
        }
        Err(_) => {
            debug_bar(9, 1, colors::RED);
            false
        }
    }
}

/// Find the index of the first ATA device
fn find_ata_device_index() -> Option<usize> {
    // Debug: check each slot
    for i in 0..4 {
        // Show we're checking this slot
        debug_bar(13 + i, 0, colors::YELLOW);

        if let Some(device) = ata::get_device(i) {
            // Show device type
            let type_val = match device.device_type {
                ata::DeviceType::None => 0,
                ata::DeviceType::Unknown => 1,
                ata::DeviceType::Ata => 2,
                ata::DeviceType::Atapi => 3,
            };
            debug_hex(13 + i, type_val);

            if device.device_type == ata::DeviceType::Ata {
                debug_bar(13 + i, 0, colors::GREEN);  // Found ATA!
                return Some(i);
            } else {
                debug_bar(13 + i, 0, colors::BROWN);  // Not ATA
            }
        } else {
            debug_bar(13 + i, 0, colors::DARK_GRAY);  // No device in slot
        }
    }
    None
}

/// List ROM files (.gb, .gbc) in root directory
/// Returns count of files found
pub fn list_rom_files() -> usize {
    if !fat32::is_mounted() {
        return 0;
    }

    let count = fat32::get_fs().count_roms();

    // Show count on debug bar
    debug_bar(9, 3, if count > 0 { colors::GREEN } else { colors::BROWN });
    debug_hex(9, count as u32);

    count
}

/// Test FAT32 - mount and verify ROM detection
pub fn test_fat32() -> bool {
    debug_bar(9, 0, colors::YELLOW);

    if !mount_fat32() {
        return false;
    }

    let rom_count = fat32::get_fs().count_roms();

    debug_bar(9, 3, if rom_count > 0 { colors::GREEN } else { colors::BROWN });
    debug_hex(9, rom_count as u32);

    rom_count > 0
}
