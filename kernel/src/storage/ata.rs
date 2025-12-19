//! ATA/IDE Driver
//!
//! PIO-mode ATA driver for the Intel PIIX4 IDE controller.
//! Uses polling (not IRQs) for maximum compatibility and debuggability.
//!
//! Supports:
//! - ATA hard drives (IDENTIFY, READ SECTORS)
//! - ATAPI CD-ROM drives (IDENTIFY PACKET, READ(12))
//!
//! Based on Armada E500 PIIX4M documentation:
//! - Primary IDE: 0x1F0-0x1F7, control 0x3F6, IRQ 14
//! - Secondary IDE: 0x170-0x177, control 0x376, IRQ 15

use crate::arch::x86::io::{inb, outb, inw, outw, inl, outl};

// =============================================================================
// Port Addresses
// =============================================================================

/// Primary IDE channel ports
pub mod primary {
    pub const DATA: u16 = 0x1F0;
    pub const ERROR: u16 = 0x1F1;        // Read: error, Write: features
    pub const FEATURES: u16 = 0x1F1;
    pub const SECTOR_COUNT: u16 = 0x1F2;
    pub const LBA_LO: u16 = 0x1F3;       // Sector number / LBA bits 0-7
    pub const LBA_MID: u16 = 0x1F4;      // Cylinder low / LBA bits 8-15
    pub const LBA_HI: u16 = 0x1F5;       // Cylinder high / LBA bits 16-23
    pub const DRIVE_HEAD: u16 = 0x1F6;   // Drive/head / LBA bits 24-27
    pub const STATUS: u16 = 0x1F7;       // Read: status, Write: command
    pub const COMMAND: u16 = 0x1F7;
    pub const CONTROL: u16 = 0x3F6;      // Device control / alt status
    pub const ALT_STATUS: u16 = 0x3F6;
}

/// Secondary IDE channel ports
pub mod secondary {
    pub const DATA: u16 = 0x170;
    pub const ERROR: u16 = 0x171;
    pub const FEATURES: u16 = 0x171;
    pub const SECTOR_COUNT: u16 = 0x172;
    pub const LBA_LO: u16 = 0x173;
    pub const LBA_MID: u16 = 0x174;
    pub const LBA_HI: u16 = 0x175;
    pub const DRIVE_HEAD: u16 = 0x176;
    pub const STATUS: u16 = 0x177;
    pub const COMMAND: u16 = 0x177;
    pub const CONTROL: u16 = 0x376;
    pub const ALT_STATUS: u16 = 0x376;
}

// =============================================================================
// ATA Commands
// =============================================================================

pub mod cmd {
    pub const IDENTIFY: u8 = 0xEC;
    pub const IDENTIFY_PACKET: u8 = 0xA1;
    pub const READ_SECTORS: u8 = 0x20;
    pub const READ_SECTORS_EXT: u8 = 0x24;  // 48-bit LBA
    pub const WRITE_SECTORS: u8 = 0x30;
    pub const PACKET: u8 = 0xA0;
    pub const SET_FEATURES: u8 = 0xEF;
    pub const FLUSH_CACHE: u8 = 0xE7;
}

// =============================================================================
// Status Register Bits
// =============================================================================

pub mod status {
    pub const ERR: u8 = 0x01;   // Error occurred
    pub const IDX: u8 = 0x02;   // Index mark (obsolete)
    pub const CORR: u8 = 0x04;  // Corrected data (obsolete)
    pub const DRQ: u8 = 0x08;   // Data request - ready to transfer
    pub const SRV: u8 = 0x10;   // Service request
    pub const DF: u8 = 0x20;    // Drive fault
    pub const RDY: u8 = 0x40;   // Drive ready
    pub const BSY: u8 = 0x80;   // Drive busy
}

// =============================================================================
// Error Register Bits
// =============================================================================

pub mod error {
    pub const AMNF: u8 = 0x01;  // Address mark not found
    pub const TK0NF: u8 = 0x02; // Track 0 not found
    pub const ABRT: u8 = 0x04;  // Command aborted
    pub const MCR: u8 = 0x08;   // Media change request
    pub const IDNF: u8 = 0x10;  // ID not found
    pub const MC: u8 = 0x20;    // Media changed
    pub const UNC: u8 = 0x40;   // Uncorrectable data error
    pub const BBK: u8 = 0x80;   // Bad block detected
}

// =============================================================================
// Drive/Head Register
// =============================================================================

/// Select master drive with LBA mode
pub const DRIVE_MASTER_LBA: u8 = 0xE0;
/// Select slave drive with LBA mode
pub const DRIVE_SLAVE_LBA: u8 = 0xF0;

// =============================================================================
// Timeouts (in loop iterations, roughly ~1ms per 1000 iterations)
// =============================================================================

const TIMEOUT_BSY: u32 = 100_000;      // Wait for not-busy
const TIMEOUT_DRQ: u32 = 100_000;      // Wait for data ready
const TIMEOUT_IDENTIFY: u32 = 500_000; // IDENTIFY can be slow

// =============================================================================
// Channel Abstraction
// =============================================================================

/// IDE Channel (Primary or Secondary)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Primary,
    Secondary,
}

impl Channel {
    /// Get base I/O port for this channel
    pub fn base_port(&self) -> u16 {
        match self {
            Channel::Primary => primary::DATA,
            Channel::Secondary => secondary::DATA,
        }
    }

    /// Get control port for this channel
    pub fn control_port(&self) -> u16 {
        match self {
            Channel::Primary => primary::CONTROL,
            Channel::Secondary => secondary::CONTROL,
        }
    }

    /// Get IRQ for this channel
    pub fn irq(&self) -> u8 {
        match self {
            Channel::Primary => 14,
            Channel::Secondary => 15,
        }
    }
}

// =============================================================================
// Drive Selection
// =============================================================================

/// Drive on a channel (Master or Slave)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    Master,
    Slave,
}

impl Drive {
    /// Get drive/head register value for LBA mode
    pub fn select_byte(&self) -> u8 {
        match self {
            Drive::Master => DRIVE_MASTER_LBA,
            Drive::Slave => DRIVE_SLAVE_LBA,
        }
    }
}

// =============================================================================
// Device Type
// =============================================================================

/// Type of ATA device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// No device present
    None,
    /// Unknown device (present but didn't identify)
    Unknown,
    /// ATA hard disk
    Ata,
    /// ATAPI device (CD-ROM, etc.)
    Atapi,
}

// =============================================================================
// ATA Device Info
// =============================================================================

/// Information about an ATA/ATAPI device
#[derive(Debug, Clone)]
pub struct AtaDevice {
    pub channel: Channel,
    pub drive: Drive,
    pub device_type: DeviceType,
    pub model: [u8; 40],
    pub serial: [u8; 20],
    pub firmware: [u8; 8],
    pub sectors: u64,           // Total sectors (LBA48 or LBA28)
    pub supports_lba48: bool,
    pub sector_size: u32,       // Usually 512
}

impl AtaDevice {
    pub const fn empty() -> Self {
        Self {
            channel: Channel::Primary,
            drive: Drive::Master,
            device_type: DeviceType::None,
            model: [0; 40],
            serial: [0; 20],
            firmware: [0; 8],
            sectors: 0,
            supports_lba48: false,
            sector_size: 512,
        }
    }

    /// Get model string (trimmed)
    pub fn model_str(&self) -> &str {
        let end = self.model.iter().rposition(|&c| c != 0 && c != b' ').map(|i| i + 1).unwrap_or(0);
        core::str::from_utf8(&self.model[..end]).unwrap_or("Unknown")
    }

    /// Get capacity in bytes
    pub fn capacity_bytes(&self) -> u64 {
        self.sectors * self.sector_size as u64
    }

    /// Get capacity in MB
    pub fn capacity_mb(&self) -> u64 {
        self.capacity_bytes() / (1024 * 1024)
    }
}

// =============================================================================
// Global State
// =============================================================================

/// Maximum devices (2 channels × 2 drives)
const MAX_DEVICES: usize = 4;

/// Detected ATA devices
static mut ATA_DEVICES: [AtaDevice; MAX_DEVICES] = [
    AtaDevice::empty(),
    AtaDevice::empty(),
    AtaDevice::empty(),
    AtaDevice::empty(),
];

/// Number of detected devices
static mut ATA_DEVICE_COUNT: usize = 0;

// =============================================================================
// Low-Level I/O
// =============================================================================

/// Read status register (clears pending interrupt)
#[inline]
fn read_status(channel: Channel) -> u8 {
    unsafe { inb(channel.base_port() + 7) }
}

/// Read alternate status (doesn't clear interrupt)
#[inline]
fn read_alt_status(channel: Channel) -> u8 {
    unsafe { inb(channel.control_port()) }
}

/// Read error register
#[inline]
fn read_error(channel: Channel) -> u8 {
    unsafe { inb(channel.base_port() + 1) }
}

/// Write to control register
#[inline]
fn write_control(channel: Channel, value: u8) {
    unsafe { outb(channel.control_port(), value); }
}

/// Small delay by reading alt status (400ns)
#[inline]
fn io_delay(channel: Channel) {
    // Reading alt status 4 times provides ~400ns delay
    for _ in 0..4 {
        let _ = read_alt_status(channel);
    }
}

// =============================================================================
// Waiting Functions (with timeouts!)
// =============================================================================

/// Wait for BSY to clear
/// Returns true if successful, false on timeout
fn wait_not_busy(channel: Channel, timeout: u32) -> bool {
    for _ in 0..timeout {
        let status = read_alt_status(channel);
        if (status & status::BSY) == 0 {
            return true;
        }
        // Small spin
        core::hint::spin_loop();
    }
    false
}

/// Wait for DRQ (data request) to be set
/// Returns true if successful, false on timeout or error
fn wait_drq(channel: Channel, timeout: u32) -> Result<(), u8> {
    for _ in 0..timeout {
        let status = read_alt_status(channel);

        // Check for error
        if (status & status::ERR) != 0 {
            return Err(read_error(channel));
        }
        if (status & status::DF) != 0 {
            return Err(0xFF);  // Drive fault
        }

        // Check for data ready
        if (status & status::DRQ) != 0 {
            return Ok(());
        }

        // Still busy, keep waiting
        if (status & status::BSY) != 0 {
            for _ in 0..10 {
                core::hint::spin_loop();
            }
            continue;
        }

        // Not busy, not DRQ, not error - might be done
        for _ in 0..10 {
            core::hint::spin_loop();
        }
    }
    Err(0xFE)  // Timeout
}

/// Wait for drive ready
fn wait_ready(channel: Channel, timeout: u32) -> bool {
    for _ in 0..timeout {
        let status = read_alt_status(channel);
        if (status & status::BSY) == 0 && (status & status::RDY) != 0 {
            return true;
        }
        for _ in 0..10 {
            core::hint::spin_loop();
        }
    }
    false
}

// =============================================================================
// Drive Selection
// =============================================================================

/// Select a drive on a channel
fn select_drive(channel: Channel, drive: Drive) -> bool {
    let base = channel.base_port();

    // Write drive select
    unsafe {
        outb(base + 6, drive.select_byte());
    }

    // Wait 400ns for drive to respond
    io_delay(channel);

    // Wait for not busy - short timeout
    wait_not_busy(channel, 10_000)
}

// =============================================================================
// Software Reset
// =============================================================================

/// Software reset a channel (resets both drives on the channel)
pub fn reset_channel(channel: Channel) {
    let control = channel.control_port();

    unsafe {
        // Set SRST bit (bit 2) and nIEN (bit 1) to disable interrupts
        outb(control, 0x06);

        // Wait at least 5 microseconds
        for _ in 0..2000 {
            core::hint::spin_loop();
        }

        // Clear SRST, keep nIEN set (we're using polling)
        outb(control, 0x02);
    }

    // Brief wait for reset to complete (don't hang on empty channels)
    wait_not_busy(channel, 100_000);

    io_delay(channel);
}

// =============================================================================
// Device Detection
// =============================================================================

/// Check if a device is present on channel/drive
fn detect_device_type(channel: Channel, drive: Drive) -> DeviceType {
    // Select drive
    if !select_drive(channel, drive) {
        return DeviceType::None;
    }

    let base = channel.base_port();

    // Check signature in sector count and LBA registers
    // After reset: ATA drives have 0x01 in sector count and 0x01 in LBA low
    //              ATAPI drives have 0x01 in sector count, 0x14 in LBA mid, 0xEB in LBA hi
    let sc = unsafe { inb(base + 2) };
    let lba_mid = unsafe { inb(base + 4) };
    let lba_hi = unsafe { inb(base + 5) };

    // Check for ATAPI signature
    if lba_mid == 0x14 && lba_hi == 0xEB {
        return DeviceType::Atapi;
    }

    // Check for SATA signature (SATA devices in AHCI mode report differently)
    if lba_mid == 0x3C && lba_hi == 0xC3 {
        return DeviceType::Ata;  // SATA as ATA
    }

    // Try IDENTIFY command to confirm ATA
    if try_identify_ata(channel) {
        return DeviceType::Ata;
    }

    // Try IDENTIFY PACKET for ATAPI
    if try_identify_atapi(channel) {
        return DeviceType::Atapi;
    }

    // Check if anything responded at all
    let status = read_status(channel);
    if status == 0x00 || status == 0xFF {
        DeviceType::None
    } else {
        DeviceType::Unknown
    }
}

/// Try to execute IDENTIFY command (returns true if device responded)
fn try_identify_ata(channel: Channel) -> bool {
    let base = channel.base_port();

    // Clear registers
    unsafe {
        outb(base + 2, 0);  // Sector count
        outb(base + 3, 0);  // LBA lo
        outb(base + 4, 0);  // LBA mid
        outb(base + 5, 0);  // LBA hi
    }

    // Send IDENTIFY command
    unsafe {
        outb(base + 7, cmd::IDENTIFY);
    }

    io_delay(channel);

    // Check immediate response
    let status = read_alt_status(channel);
    if status == 0 || status == 0xFF {
        return false;  // No device
    }

    // Wait for not busy (short timeout)
    if !wait_not_busy(channel, 50_000) {
        return false;
    }

    // Check for ATAPI signature change
    let lba_mid = unsafe { inb(base + 4) };
    let lba_hi = unsafe { inb(base + 5) };

    // If ATAPI signature appeared, this is CD-ROM
    if lba_mid == 0x14 && lba_hi == 0xEB {
        return false;  // ATAPI device, not ATA
    }

    // Check for DRQ or ready status
    let status = read_status(channel);

    // If error, might still be ATA - some drives set error on IDENTIFY
    // Check if DRQ is set (data ready)
    if (status & status::DRQ) != 0 {
        // Drain the data (256 words = 512 bytes)
        for _ in 0..256 {
            let _ = unsafe { inw(base) };
        }
        return true;
    }

    // Some drives need more time - wait for DRQ
    for _ in 0..50_000 {
        let status = read_status(channel);
        if (status & status::DRQ) != 0 {
            // Drain the data
            for _ in 0..256 {
                let _ = unsafe { inw(base) };
            }
            return true;
        }
        if (status & status::ERR) != 0 {
            break;  // Error, give up
        }
        core::hint::spin_loop();
    }

    false
}

/// Try to execute IDENTIFY PACKET command (returns true if device responded)
fn try_identify_atapi(channel: Channel) -> bool {
    let base = channel.base_port();

    // Send IDENTIFY PACKET command
    unsafe {
        outb(base + 7, cmd::IDENTIFY_PACKET);
    }

    io_delay(channel);

    // Check immediate response
    let status = read_alt_status(channel);
    if status == 0 {
        return false;
    }

    // Wait for not busy
    if !wait_not_busy(channel, TIMEOUT_IDENTIFY) {
        return false;
    }

    // Check for DRQ
    let status = read_status(channel);
    if (status & status::ERR) != 0 {
        return false;
    }
    if (status & status::DRQ) != 0 {
        // Drain the data
        for _ in 0..256 {
            let _ = unsafe { inw(base) };
        }
        return true;
    }

    false
}

// =============================================================================
// IDENTIFY Command
// =============================================================================

/// Execute IDENTIFY command and fill in device info
fn identify_device(device: &mut AtaDevice) -> bool {
    let base = device.channel.base_port();

    if !select_drive(device.channel, device.drive) {
        return false;
    }

    // Clear registers
    unsafe {
        outb(base + 2, 0);
        outb(base + 3, 0);
        outb(base + 4, 0);
        outb(base + 5, 0);
    }

    // Send appropriate IDENTIFY command
    let identify_cmd = if device.device_type == DeviceType::Atapi {
        cmd::IDENTIFY_PACKET
    } else {
        cmd::IDENTIFY
    };

    unsafe {
        outb(base + 7, identify_cmd);
    }

    io_delay(device.channel);

    // Wait for data
    if wait_drq(device.channel, TIMEOUT_IDENTIFY).is_err() {
        return false;
    }

    // Read 256 words of identify data
    let mut identify_data = [0u16; 256];
    for i in 0..256 {
        identify_data[i] = unsafe { inw(base) };
    }

    // Parse identify data
    parse_identify_data(device, &identify_data);

    true
}

/// Parse IDENTIFY data into device structure
fn parse_identify_data(device: &mut AtaDevice, data: &[u16; 256]) {
    // Words 27-46: Model number (40 bytes, byte-swapped)
    for i in 0..20 {
        let word = data[27 + i];
        device.model[i * 2] = (word >> 8) as u8;
        device.model[i * 2 + 1] = (word & 0xFF) as u8;
    }

    // Words 10-19: Serial number (20 bytes, byte-swapped)
    for i in 0..10 {
        let word = data[10 + i];
        device.serial[i * 2] = (word >> 8) as u8;
        device.serial[i * 2 + 1] = (word & 0xFF) as u8;
    }

    // Words 23-26: Firmware revision (8 bytes, byte-swapped)
    for i in 0..4 {
        let word = data[23 + i];
        device.firmware[i * 2] = (word >> 8) as u8;
        device.firmware[i * 2 + 1] = (word & 0xFF) as u8;
    }

    // Word 83 bit 10: LBA48 supported
    device.supports_lba48 = (data[83] & (1 << 10)) != 0;

    if device.device_type == DeviceType::Ata {
        if device.supports_lba48 {
            // Words 100-103: LBA48 sector count
            device.sectors = (data[100] as u64)
                | ((data[101] as u64) << 16)
                | ((data[102] as u64) << 32)
                | ((data[103] as u64) << 48);
        } else {
            // Words 60-61: LBA28 sector count
            device.sectors = (data[60] as u64) | ((data[61] as u64) << 16);
        }
    }

    // Word 106: Logical sector size info
    if (data[106] & 0x4000) != 0 && (data[106] & 0x8000) == 0 {
        // Logical sector size is valid
        if (data[106] & 0x1000) != 0 {
            // Words 117-118: Logical sector size in words
            let size_words = (data[117] as u32) | ((data[118] as u32) << 16);
            device.sector_size = size_words * 2;
        }
    }
    if device.sector_size == 0 {
        device.sector_size = 512;
    }
}

// =============================================================================
// Initialization
// =============================================================================

/// Initialize ATA subsystem and detect all devices
pub fn init() -> usize {
    // Debug layout (rows 1-4 of VGA):
    // Row 1: Primary Master status
    // Row 2: Primary Slave status
    // Row 3: Secondary Master status
    // Row 4: Secondary Slave status

    // Mark init started
    draw_debug_pixel(0, 90, 0x0F);  // White = starting

    // FIRST: Quick check if IDE channels exist at all
    // Read status from both channels - 0xFF means floating bus (nothing there)
    let pri_status = unsafe { inb(primary::STATUS) };
    let sec_status = unsafe { inb(secondary::STATUS) };

    draw_debug_byte(0, 25, pri_status);
    draw_debug_byte(0, 27, sec_status);

    // Mark all rows as starting
    for row in 1..=4 {
        draw_debug_pixel(row, 0, 0x0F);
    }

    unsafe {
        ATA_DEVICE_COUNT = 0;
    }

    let mut count = 0;

    // Check each channel
    for (ch_idx, channel) in [Channel::Primary, Channel::Secondary].iter().enumerate() {
        let base = channel.base_port();
        let ch_marker_col = 100 + ch_idx * 20;

        draw_debug_pixel(0, ch_marker_col, 0x0E);  // Yellow = checking channel

        // Quick floating bus check - if 0xFF, channel is empty
        let quick_status = unsafe { inb(base + 7) };
        if quick_status == 0xFF {
            // No controller or nothing connected - skip entire channel
            draw_debug_pixel(0, ch_marker_col, 0x08);  // Gray = empty channel
            let reset_row = ch_idx * 2 + 1;
            draw_debug_cell(reset_row, 0, 0x08);  // Gray
            draw_debug_cell(reset_row + 1, 0, 0x08);  // Gray for slave too
            continue;
        }

        // Channel exists - check if BIOS initialized it
        let reset_row = ch_idx * 2 + 1;
        let needs_reset = quick_status == 0x00 || (quick_status & 0x80) != 0;  // 0x00 or BSY stuck

        if needs_reset {
            draw_debug_cell(reset_row, 0, 0x0E);  // Yellow = resetting
            reset_channel(*channel);
            draw_debug_cell(reset_row, 0, 0x0A);  // Green = reset done
        } else {
            draw_debug_cell(reset_row, 0, 0x0B);  // Cyan = BIOS already init
        }

        draw_debug_pixel(0, ch_marker_col, 0x0A);  // Green = channel OK

        // Check each drive on this channel
        for (drv_idx, drive) in [Drive::Master, Drive::Slave].iter().enumerate() {
            let idx = ch_idx * 2 + drv_idx;
            let row = idx + 1;

            draw_debug_pixel(0, ch_marker_col + 4 + drv_idx * 2, 0x0E);
            draw_debug_cell(row, 1, 0x0E);  // Yellow = selecting

            // Select drive
            unsafe { outb(base + 6, drive.select_byte()); }
            io_delay(*channel);
            io_delay(*channel);

            // Quick check after select
            let status = unsafe { inb(base + 7) };
            draw_debug_byte(row, 10, status);

            // Check for no device patterns
            if status == 0xFF || status == 0x00 || status == 0x7F {
                draw_debug_cell(row, 1, 0x08);  // Gray = no device
                draw_debug_pixel(0, ch_marker_col + 4 + drv_idx * 2, 0x08);
                continue;
            }

            // Wait briefly for BSY to clear (short timeout)
            if (status & status::BSY) != 0 {
                if !wait_not_busy(*channel, 20_000) {
                    draw_debug_cell(row, 1, 0x06);  // Brown = busy timeout
                    draw_debug_pixel(0, ch_marker_col + 4 + drv_idx * 2, 0x06);
                    continue;
                }
            }

            draw_debug_cell(row, 1, 0x0A);  // Green = selected

            // Read signature
            let final_status = read_status(*channel);
            let lba_mid = unsafe { inb(base + 4) };
            let lba_hi = unsafe { inb(base + 5) };

            draw_debug_byte(row, 6, final_status);
            draw_debug_byte(row, 7, lba_mid);
            draw_debug_byte(row, 8, lba_hi);

            // Determine device type - try IDENTIFY command first
            // Some drives don't have proper signature bytes after BIOS init
            let device_type = if lba_mid == 0x14 && lba_hi == 0xEB {
                // ATAPI signature - definitely CD/DVD
                DeviceType::Atapi
            } else if lba_mid == 0x3C && lba_hi == 0xC3 {
                // SATA signature
                DeviceType::Ata
            } else {
                // Try IDENTIFY command - works for most HDDs
                if try_identify_ata(*channel) {
                    DeviceType::Ata
                } else if lba_mid == 0x14 || lba_hi == 0xEB {
                    // Partial ATAPI signature
                    DeviceType::Atapi
                } else if try_identify_atapi(*channel) {
                    DeviceType::Atapi
                } else if final_status != 0 && final_status != 0xFF {
                    // Something is responding but not identifying
                    // Could be a very old drive or unusual hardware
                    DeviceType::Unknown
                } else {
                    DeviceType::None
                }
            };

            let type_color = match device_type {
                DeviceType::None => 0x08,
                DeviceType::Unknown => 0x06,
                DeviceType::Ata => 0x0A,
                DeviceType::Atapi => 0x0B,
            };
            draw_debug_cell(row, 3, type_color);

            if device_type == DeviceType::None {
                draw_debug_cell(row, 2, 0x08);
                draw_debug_pixel(0, ch_marker_col + 4 + drv_idx * 2, 0x08);
                continue;
            }

            draw_debug_cell(row, 2, 0x0A);

            // Create and identify device
            let mut device = AtaDevice::empty();
            device.channel = *channel;
            device.drive = *drive;
            device.device_type = device_type;

            draw_debug_cell(row, 4, 0x0E);
            if identify_device(&mut device) {
                draw_debug_cell(row, 4, 0x0A);
                if device_type == DeviceType::Ata {
                    let gb = (device.capacity_mb() / 1024) as u8;
                    draw_debug_byte(row, 9, gb);
                }
            } else {
                draw_debug_cell(row, 4, 0x06);
            }

            draw_debug_cell(row, 5, type_color);
            draw_debug_pixel(0, ch_marker_col + 4 + drv_idx * 2, type_color);

            unsafe {
                ATA_DEVICES[idx] = device;
                ATA_DEVICE_COUNT = idx + 1;
            }
            count += 1;
        }
    }

    // Summary
    draw_debug_cell(5, 0, if count > 0 { 0x0A } else { 0x04 });
    draw_debug_byte(5, 1, count as u8);
    draw_debug_pixel(0, 180, 0x0F);  // White = complete

    count
}

/// Draw a single debug pixel at row/x position
fn draw_debug_pixel(row: usize, x: usize, color: u8) {
    unsafe {
        let vga = 0xA0000 as *mut u8;
        let offset = row * 320 + x;
        core::ptr::write_volatile(vga.add(offset), color);
    }
}

/// Draw a debug cell (6 pixels wide) at row/column
fn draw_debug_cell(row: usize, col: usize, color: u8) {
    unsafe {
        let vga = 0xA0000 as *mut u8;
        let row_offset = row * 320;
        let col_offset = col * 8;
        for i in 0..6 {
            core::ptr::write_volatile(vga.add(row_offset + col_offset + i), color);
        }
    }
}

/// Draw a byte value as two colored cells (hex nibbles)
fn draw_debug_byte(row: usize, col: usize, value: u8) {
    let hi = (value >> 4) & 0x0F;
    let lo = value & 0x0F;

    // Color code: 0=black, 1-9=blues/greens, A-F=reds/magentas
    let hi_color = if hi == 0 { 0x08 } else { 0x10 + hi };
    let lo_color = if lo == 0 { 0x08 } else { 0x10 + lo };

    unsafe {
        let vga = 0xA0000 as *mut u8;
        let row_offset = row * 320;
        let col_offset = col * 8;

        // High nibble (3 pixels)
        for i in 0..3 {
            core::ptr::write_volatile(vga.add(row_offset + col_offset + i), hi_color);
        }
        // Low nibble (3 pixels)
        for i in 3..6 {
            core::ptr::write_volatile(vga.add(row_offset + col_offset + i), lo_color);
        }
    }
}

/// Get number of detected devices
pub fn device_count() -> usize {
    unsafe { ATA_DEVICE_COUNT }
}

/// Get device by index
pub fn get_device(index: usize) -> Option<&'static AtaDevice> {
    unsafe {
        if index < MAX_DEVICES {
            let dev = &ATA_DEVICES[index];
            if dev.device_type != DeviceType::None {
                return Some(dev);
            }
        }
        None
    }
}

/// Find first ATA device (hard disk)
pub fn find_ata_disk() -> Option<&'static AtaDevice> {
    unsafe {
        for i in 0..MAX_DEVICES {
            if ATA_DEVICES[i].device_type == DeviceType::Ata {
                return Some(&ATA_DEVICES[i]);
            }
        }
        None
    }
}

/// Find first ATAPI device (CD-ROM)
pub fn find_atapi_device() -> Option<&'static AtaDevice> {
    unsafe {
        for i in 0..MAX_DEVICES {
            if ATA_DEVICES[i].device_type == DeviceType::Atapi {
                return Some(&ATA_DEVICES[i]);
            }
        }
        None
    }
}

// =============================================================================
// Sector Reading
// =============================================================================

/// Read sectors from an ATA device
///
/// - `device`: Device to read from
/// - `lba`: Starting logical block address
/// - `count`: Number of sectors to read (1-256, 0 means 256)
/// - `buffer`: Buffer to read into (must be count * sector_size bytes)
///
/// Returns number of sectors read, or error message
pub fn read_sectors(
    device: &AtaDevice,
    lba: u64,
    count: u8,
    buffer: &mut [u8]
) -> Result<usize, &'static str> {
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

    // Select drive and set up LBA
    if !select_drive(device.channel, device.drive) {
        return Err("Drive select timeout");
    }

    // Wait for drive ready
    if !wait_ready(device.channel, TIMEOUT_BSY) {
        return Err("Drive not ready");
    }

    unsafe {
        if device.supports_lba48 && lba > 0x0FFFFFFF {
            // LBA48 mode
            outb(base + 2, 0);                          // Sector count high
            outb(base + 3, ((lba >> 24) & 0xFF) as u8); // LBA 24-31
            outb(base + 4, ((lba >> 32) & 0xFF) as u8); // LBA 32-39
            outb(base + 5, ((lba >> 40) & 0xFF) as u8); // LBA 40-47
            outb(base + 2, count);                       // Sector count low
            outb(base + 3, (lba & 0xFF) as u8);         // LBA 0-7
            outb(base + 4, ((lba >> 8) & 0xFF) as u8);  // LBA 8-15
            outb(base + 5, ((lba >> 16) & 0xFF) as u8); // LBA 16-23
            outb(base + 7, cmd::READ_SECTORS_EXT);
        } else {
            // LBA28 mode
            let drive_byte = device.drive.select_byte() | ((lba >> 24) & 0x0F) as u8;
            outb(base + 6, drive_byte);
            outb(base + 2, count);
            outb(base + 3, (lba & 0xFF) as u8);
            outb(base + 4, ((lba >> 8) & 0xFF) as u8);
            outb(base + 5, ((lba >> 16) & 0xFF) as u8);
            outb(base + 7, cmd::READ_SECTORS);
        }
    }

    // Read each sector
    let mut offset = 0;
    for _ in 0..actual_count {
        // Wait for data
        match wait_drq(device.channel, TIMEOUT_DRQ) {
            Ok(()) => {}
            Err(e) => {
                if e == 0xFE {
                    return Err("Timeout waiting for data");
                } else if e == 0xFF {
                    return Err("Drive fault");
                } else {
                    return Err("Read error");
                }
            }
        }

        // Read sector data (256 words = 512 bytes)
        let words = sector_size / 2;
        for _ in 0..words {
            let word = unsafe { inw(base) };
            buffer[offset] = (word & 0xFF) as u8;
            buffer[offset + 1] = ((word >> 8) & 0xFF) as u8;
            offset += 2;
        }
    }

    Ok(actual_count)
}

// =============================================================================
// Debug Helper
// =============================================================================

/// Draw a debug bar for ATA progress (legacy - kept for compatibility)
#[allow(dead_code)]
fn draw_debug_bar(stage: usize, color: u8) {
    draw_debug_cell(6, stage, color);
}
