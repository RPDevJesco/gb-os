//! PIIX4M IDE Controller Driver - Armada E500 Optimized
//!
//! IDE/ATA driver optimized for the Intel PIIX4M Southbridge found in
//! the Compaq Armada E500, based on the Technical Reference Guide.
//!
//! Hardware Configuration (from Chapter 2, Table 2-4):
//! - Primary IDE: Base 0x1F0h, Control 0x3F6h, IRQ 14
//! - Secondary IDE: Base 0x170h, Control 0x376h, IRQ 15
//! - Ultra-DMA-33 support (up to 33 MB/s)
//! - PIO transfers up to 14 MB/s
//! - 16 x 32-bit buffers per channel
//!
//! Connected Devices:
//! - Primary: Internal 2.5" HDD (44-pin connector)
//! - Secondary: MultiBay device (CD-ROM, DVD-ROM, LS-120, secondary HDD)

use crate::arch::x86::io::{inb, outb, inw, outw};

// =============================================================================
// Hardware Constants (from Armada E500 Tech Ref)
// =============================================================================

/// Primary IDE channel (internal HDD)
pub const PRIMARY_BASE: u16 = 0x1F0;
pub const PRIMARY_CTRL: u16 = 0x3F6;
pub const PRIMARY_IRQ: u8 = 14;

/// Secondary IDE channel (MultiBay)
pub const SECONDARY_BASE: u16 = 0x170;
pub const SECONDARY_CTRL: u16 = 0x376;
pub const SECONDARY_IRQ: u8 = 15;

/// IDE Register Offsets
pub mod reg {
    pub const DATA: u16 = 0;           // Data Register (R/W)
    pub const ERROR: u16 = 1;          // Error Register (R)
    pub const FEATURES: u16 = 1;       // Features Register (W)
    pub const SECTOR_COUNT: u16 = 2;   // Sector Count
    pub const LBA_LOW: u16 = 3;        // LBA 0-7 / Sector Number
    pub const LBA_MID: u16 = 4;        // LBA 8-15 / Cylinder Low
    pub const LBA_HIGH: u16 = 5;       // LBA 16-23 / Cylinder High
    pub const DEVICE: u16 = 6;         // Device/Head Select
    pub const STATUS: u16 = 7;         // Status (R)
    pub const COMMAND: u16 = 7;        // Command (W)
}

/// Control Register Offsets
pub mod ctrl {
    pub const ALT_STATUS: u16 = 0;     // Alternate Status (R)
    pub const DEVICE_CTRL: u16 = 0;    // Device Control (W)
}

/// Status Register Bits
pub mod status {
    pub const BSY: u8 = 0x80;          // Busy
    pub const DRDY: u8 = 0x40;         // Device Ready
    pub const DF: u8 = 0x20;           // Device Fault
    pub const DSC: u8 = 0x10;          // Seek Complete (deprecated)
    pub const DRQ: u8 = 0x08;          // Data Request
    pub const CORR: u8 = 0x04;         // Corrected Data (deprecated)
    pub const IDX: u8 = 0x02;          // Index (deprecated)
    pub const ERR: u8 = 0x01;          // Error
}

/// Error Register Bits
pub mod error {
    pub const BBK: u8 = 0x80;          // Bad Block (deprecated)
    pub const UNC: u8 = 0x40;          // Uncorrectable Data
    pub const MC: u8 = 0x20;           // Media Changed
    pub const IDNF: u8 = 0x10;         // ID Not Found
    pub const MCR: u8 = 0x08;          // Media Change Request
    pub const ABRT: u8 = 0x04;         // Command Aborted
    pub const TK0NF: u8 = 0x02;        // Track 0 Not Found
    pub const AMNF: u8 = 0x01;         // Address Mark Not Found
}

/// Device Control Register Bits
pub mod device_ctrl {
    pub const NIEN: u8 = 0x02;         // Disable Interrupts
    pub const SRST: u8 = 0x04;         // Software Reset
    pub const HOB: u8 = 0x80;          // High Order Byte (LBA48)
}

/// ATA Commands
pub mod cmd {
    pub const NOP: u8 = 0x00;
    pub const READ_SECTORS: u8 = 0x20;
    pub const READ_SECTORS_EXT: u8 = 0x24;     // LBA48
    pub const WRITE_SECTORS: u8 = 0x30;
    pub const WRITE_SECTORS_EXT: u8 = 0x34;    // LBA48
    pub const READ_VERIFY: u8 = 0x40;
    pub const FORMAT_TRACK: u8 = 0x50;
    pub const SEEK: u8 = 0x70;
    pub const EXECUTE_DIAG: u8 = 0x90;
    pub const INIT_PARAMS: u8 = 0x91;
    pub const PACKET: u8 = 0xA0;               // ATAPI
    pub const IDENTIFY_PACKET: u8 = 0xA1;      // ATAPI Identify
    pub const READ_DMA: u8 = 0xC8;
    pub const WRITE_DMA: u8 = 0xCA;
    pub const STANDBY_IMM: u8 = 0xE0;
    pub const IDLE_IMM: u8 = 0xE1;
    pub const STANDBY: u8 = 0xE2;
    pub const IDLE: u8 = 0xE3;
    pub const CHECK_POWER: u8 = 0xE5;
    pub const SLEEP: u8 = 0xE6;
    pub const FLUSH_CACHE: u8 = 0xE7;
    pub const IDENTIFY: u8 = 0xEC;
    pub const SET_FEATURES: u8 = 0xEF;
}

/// ATAPI Commands (for CD/DVD)
pub mod atapi {
    pub const TEST_UNIT_READY: u8 = 0x00;
    pub const REQUEST_SENSE: u8 = 0x03;
    pub const INQUIRY: u8 = 0x12;
    pub const START_STOP: u8 = 0x1B;
    pub const READ_CAPACITY: u8 = 0x25;
    pub const READ_10: u8 = 0x28;
    pub const READ_12: u8 = 0xA8;
    pub const READ_TOC: u8 = 0x43;
}

// =============================================================================
// IDE Channel
// =============================================================================

/// IDE channel identifier
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Channel {
    Primary,
    Secondary,
}

impl Channel {
    pub fn base_port(&self) -> u16 {
        match self {
            Channel::Primary => PRIMARY_BASE,
            Channel::Secondary => SECONDARY_BASE,
        }
    }
    
    pub fn ctrl_port(&self) -> u16 {
        match self {
            Channel::Primary => PRIMARY_CTRL,
            Channel::Secondary => SECONDARY_CTRL,
        }
    }
    
    pub fn irq(&self) -> u8 {
        match self {
            Channel::Primary => PRIMARY_IRQ,
            Channel::Secondary => SECONDARY_IRQ,
        }
    }
}

/// Device on channel (master/slave)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Device {
    Master = 0,
    Slave = 1,
}

// =============================================================================
// Device Information
// =============================================================================

/// Device type identified during probe
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceType {
    None,
    Ata,      // ATA hard drive
    Atapi,    // ATAPI device (CD/DVD/LS-120)
    Unknown,
}

/// Information from IDENTIFY command
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_type: DeviceType,
    pub model: [u8; 40],
    pub serial: [u8; 20],
    pub firmware: [u8; 8],
    pub lba_sectors: u32,
    pub lba48_sectors: u64,
    pub supports_lba: bool,
    pub supports_lba48: bool,
    pub supports_dma: bool,
    pub supports_udma: bool,
    pub sector_size: u16,
    pub max_udma_mode: u8,
}

impl DeviceInfo {
    pub const fn new() -> Self {
        Self {
            device_type: DeviceType::None,
            model: [0; 40],
            serial: [0; 20],
            firmware: [0; 8],
            lba_sectors: 0,
            lba48_sectors: 0,
            supports_lba: false,
            supports_lba48: false,
            supports_dma: false,
            supports_udma: false,
            sector_size: 512,
            max_udma_mode: 0,
        }
    }
    
    /// Get model string
    pub fn model_string(&self) -> &str {
        // Find end of string (trim spaces)
        let mut end = self.model.len();
        while end > 0 && (self.model[end - 1] == 0 || self.model[end - 1] == b' ') {
            end -= 1;
        }
        core::str::from_utf8(&self.model[..end]).unwrap_or("Unknown")
    }
    
    /// Get total capacity in bytes
    pub fn capacity_bytes(&self) -> u64 {
        if self.supports_lba48 {
            self.lba48_sectors * self.sector_size as u64
        } else {
            self.lba_sectors as u64 * self.sector_size as u64
        }
    }
}

// =============================================================================
// IDE Controller
// =============================================================================

/// PIIX4M IDE Controller Driver
pub struct IdeController {
    /// Devices on primary channel
    pub primary_master: DeviceInfo,
    pub primary_slave: DeviceInfo,
    /// Devices on secondary channel
    pub secondary_master: DeviceInfo,
    pub secondary_slave: DeviceInfo,
    /// Initialization state
    initialized: bool,
    /// IRQ enabled
    irq_enabled: bool,
}

impl IdeController {
    pub const fn new() -> Self {
        Self {
            primary_master: DeviceInfo::new(),
            primary_slave: DeviceInfo::new(),
            secondary_master: DeviceInfo::new(),
            secondary_slave: DeviceInfo::new(),
            initialized: false,
            irq_enabled: false,
        }
    }
    
    /// Initialize the IDE controller and probe for devices
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Reset both channels
        self.reset_channel(Channel::Primary)?;
        self.reset_channel(Channel::Secondary)?;
        
        // Probe devices on both channels
        self.primary_master = self.probe_device(Channel::Primary, Device::Master);
        self.primary_slave = self.probe_device(Channel::Primary, Device::Slave);
        self.secondary_master = self.probe_device(Channel::Secondary, Device::Master);
        self.secondary_slave = self.probe_device(Channel::Secondary, Device::Slave);
        
        self.initialized = true;
        Ok(())
    }
    
    /// Software reset an IDE channel
    fn reset_channel(&self, channel: Channel) -> Result<(), &'static str> {
        let ctrl = channel.ctrl_port();
        
        // Assert SRST
        unsafe { outb(ctrl, device_ctrl::SRST | device_ctrl::NIEN); }
        
        // Wait at least 5μs
        self.delay_400ns();
        self.delay_400ns();
        self.delay_400ns();
        
        // De-assert SRST
        unsafe { outb(ctrl, device_ctrl::NIEN); }
        
        // Wait for BSY to clear (timeout after ~2 seconds)
        self.wait_not_busy(channel, 2000)?;
        
        Ok(())
    }
    
    /// Probe a device and identify it
    fn probe_device(&self, channel: Channel, device: Device) -> DeviceInfo {
        let base = channel.base_port();
        let mut info = DeviceInfo::new();
        
        // Select device
        let dev_sel = if device == Device::Master { 0xA0 } else { 0xB0 };
        unsafe { outb(base + reg::DEVICE, dev_sel); }
        self.delay_400ns();
        
        // Check if device exists by writing/reading sector count
        unsafe {
            outb(base + reg::SECTOR_COUNT, 0x55);
            outb(base + reg::LBA_LOW, 0xAA);
            
            if inb(base + reg::SECTOR_COUNT) != 0x55 || 
               inb(base + reg::LBA_LOW) != 0xAA {
                // No device present
                return info;
            }
        }
        
        // Check device signature (after reset)
        let sig_mid = unsafe { inb(base + reg::LBA_MID) };
        let sig_high = unsafe { inb(base + reg::LBA_HIGH) };
        
        let device_type = match (sig_mid, sig_high) {
            (0x00, 0x00) => DeviceType::Ata,
            (0x14, 0xEB) => DeviceType::Atapi,
            (0x69, 0x96) => DeviceType::Atapi, // SATA ATAPI
            (0x3C, 0xC3) => DeviceType::Ata,   // SATA ATA
            _ => DeviceType::Unknown,
        };
        
        info.device_type = device_type;
        
        if device_type == DeviceType::None || device_type == DeviceType::Unknown {
            return info;
        }
        
        // Send IDENTIFY command
        let identify_cmd = match device_type {
            DeviceType::Ata => cmd::IDENTIFY,
            DeviceType::Atapi => cmd::IDENTIFY_PACKET,
            _ => return info,
        };
        
        // Wait for device ready
        if self.wait_ready(channel, 1000).is_err() {
            return info;
        }
        
        // Send identify command
        unsafe { outb(base + reg::COMMAND, identify_cmd); }
        
        // Wait for data
        if self.wait_drq(channel, 1000).is_err() {
            info.device_type = DeviceType::None;
            return info;
        }
        
        // Read 256 words of identify data
        let mut identify_data = [0u16; 256];
        for i in 0..256 {
            identify_data[i] = unsafe { inw(base + reg::DATA) };
        }
        
        // Parse identify data
        self.parse_identify(&mut info, &identify_data);
        
        info
    }
    
    /// Parse IDENTIFY data into DeviceInfo
    fn parse_identify(&self, info: &mut DeviceInfo, data: &[u16; 256]) {
        // Serial number (words 10-19), byte-swapped
        for i in 0..10 {
            let word = data[10 + i];
            info.serial[i * 2] = (word >> 8) as u8;
            info.serial[i * 2 + 1] = word as u8;
        }
        
        // Firmware revision (words 23-26), byte-swapped
        for i in 0..4 {
            let word = data[23 + i];
            info.firmware[i * 2] = (word >> 8) as u8;
            info.firmware[i * 2 + 1] = word as u8;
        }
        
        // Model number (words 27-46), byte-swapped
        for i in 0..20 {
            let word = data[27 + i];
            info.model[i * 2] = (word >> 8) as u8;
            info.model[i * 2 + 1] = word as u8;
        }
        
        // Capabilities (word 49)
        let caps = data[49];
        info.supports_lba = (caps & 0x0200) != 0;
        info.supports_dma = (caps & 0x0100) != 0;
        
        // Total addressable sectors in LBA mode (words 60-61)
        if info.supports_lba {
            info.lba_sectors = (data[60] as u32) | ((data[61] as u32) << 16);
        }
        
        // Command set supported (words 82-84)
        let cmd_set_82 = data[82];
        let cmd_set_83 = data[83];
        
        // LBA48 support (bit 10 of word 83)
        if (cmd_set_83 & 0x0400) != 0 {
            info.supports_lba48 = true;
            // LBA48 sector count (words 100-103)
            info.lba48_sectors = (data[100] as u64) |
                                ((data[101] as u64) << 16) |
                                ((data[102] as u64) << 32) |
                                ((data[103] as u64) << 48);
        }
        
        // UDMA modes (word 88)
        let udma_modes = data[88];
        if udma_modes != 0 {
            info.supports_udma = true;
            // PIIX4M supports up to UDMA mode 2 (33 MB/s)
            if (udma_modes & 0x0004) != 0 { info.max_udma_mode = 2; }
            else if (udma_modes & 0x0002) != 0 { info.max_udma_mode = 1; }
            else if (udma_modes & 0x0001) != 0 { info.max_udma_mode = 0; }
        }
        
        // Sector size (word 106 if bit 14 set and bit 15 clear)
        if (data[106] & 0xC000) == 0x4000 {
            // Logical sector size in words (words 117-118)
            let logical_size = (data[117] as u32) | ((data[118] as u32) << 16);
            info.sector_size = (logical_size * 2) as u16;
        } else {
            info.sector_size = 512;
        }
    }
    
    // =========================================================================
    // Read/Write Operations
    // =========================================================================
    
    /// Read sectors using LBA28 addressing
    pub fn read_sectors_lba28(
        &self,
        channel: Channel,
        device: Device,
        lba: u32,
        count: u8,
        buffer: &mut [u8],
    ) -> Result<(), &'static str> {
        if count == 0 {
            return Err("Sector count cannot be 0");
        }
        
        let base = channel.base_port();
        let dev_bits = if device == Device::Master { 0xE0 } else { 0xF0 };
        
        // Wait for controller ready
        self.wait_not_busy(channel, 1000)?;
        
        // Select device and set LBA bits 24-27
        unsafe {
            outb(base + reg::DEVICE, dev_bits | ((lba >> 24) & 0x0F) as u8);
        }
        self.delay_400ns();
        
        // Wait for device ready
        self.wait_ready(channel, 1000)?;
        
        // Set up parameters
        unsafe {
            outb(base + reg::SECTOR_COUNT, count);
            outb(base + reg::LBA_LOW, lba as u8);
            outb(base + reg::LBA_MID, (lba >> 8) as u8);
            outb(base + reg::LBA_HIGH, (lba >> 16) as u8);
            
            // Issue read command
            outb(base + reg::COMMAND, cmd::READ_SECTORS);
        }
        
        // Read sectors
        let sector_size = 512;
        for sector in 0..count as usize {
            // Wait for data ready
            self.wait_drq(channel, 5000)?;
            
            // Read 256 words (512 bytes)
            let offset = sector * sector_size;
            for i in 0..256 {
                let word = unsafe { inw(base + reg::DATA) };
                buffer[offset + i * 2] = word as u8;
                buffer[offset + i * 2 + 1] = (word >> 8) as u8;
            }
        }
        
        // Check for errors
        let status = unsafe { inb(base + reg::STATUS) };
        if status & status::ERR != 0 {
            return Err("Read error");
        }
        
        Ok(())
    }
    
    /// Write sectors using LBA28 addressing
    pub fn write_sectors_lba28(
        &self,
        channel: Channel,
        device: Device,
        lba: u32,
        count: u8,
        buffer: &[u8],
    ) -> Result<(), &'static str> {
        if count == 0 {
            return Err("Sector count cannot be 0");
        }
        
        let base = channel.base_port();
        let dev_bits = if device == Device::Master { 0xE0 } else { 0xF0 };
        
        // Wait for controller ready
        self.wait_not_busy(channel, 1000)?;
        
        // Select device and set LBA bits 24-27
        unsafe {
            outb(base + reg::DEVICE, dev_bits | ((lba >> 24) & 0x0F) as u8);
        }
        self.delay_400ns();
        
        // Wait for device ready
        self.wait_ready(channel, 1000)?;
        
        // Set up parameters
        unsafe {
            outb(base + reg::SECTOR_COUNT, count);
            outb(base + reg::LBA_LOW, lba as u8);
            outb(base + reg::LBA_MID, (lba >> 8) as u8);
            outb(base + reg::LBA_HIGH, (lba >> 16) as u8);
            
            // Issue write command
            outb(base + reg::COMMAND, cmd::WRITE_SECTORS);
        }
        
        // Write sectors
        let sector_size = 512;
        for sector in 0..count as usize {
            // Wait for data request
            self.wait_drq(channel, 5000)?;
            
            // Write 256 words (512 bytes)
            let offset = sector * sector_size;
            for i in 0..256 {
                let word = (buffer[offset + i * 2] as u16) |
                          ((buffer[offset + i * 2 + 1] as u16) << 8);
                unsafe { outw(base + reg::DATA, word); }
            }
        }
        
        // Flush cache
        unsafe { outb(base + reg::COMMAND, cmd::FLUSH_CACHE); }
        self.wait_not_busy(channel, 5000)?;
        
        // Check for errors
        let status = unsafe { inb(base + reg::STATUS) };
        if status & status::ERR != 0 {
            return Err("Write error");
        }
        
        Ok(())
    }
    
    // =========================================================================
    // ATAPI Operations (for MultiBay CD/DVD)
    // =========================================================================
    
    /// Send ATAPI packet command
    pub fn atapi_packet(
        &self,
        channel: Channel,
        device: Device,
        command: &[u8; 12],
        buffer: &mut [u8],
        buffer_size: usize,
    ) -> Result<usize, &'static str> {
        let base = channel.base_port();
        let dev_sel = if device == Device::Master { 0xA0 } else { 0xB0 };
        
        // Wait for controller ready
        self.wait_not_busy(channel, 1000)?;
        
        // Select device
        unsafe { outb(base + reg::DEVICE, dev_sel); }
        self.delay_400ns();
        
        // Set byte count limit
        unsafe {
            outb(base + reg::LBA_MID, (buffer_size & 0xFF) as u8);
            outb(base + reg::LBA_HIGH, ((buffer_size >> 8) & 0xFF) as u8);
            
            // Issue PACKET command
            outb(base + reg::COMMAND, cmd::PACKET);
        }
        
        // Wait for DRQ
        self.wait_drq(channel, 1000)?;
        
        // Send 12-byte command packet
        for i in 0..6 {
            let word = (command[i * 2] as u16) | ((command[i * 2 + 1] as u16) << 8);
            unsafe { outw(base + reg::DATA, word); }
        }
        
        // Wait for data or completion
        let mut bytes_read = 0;
        loop {
            // Wait for BSY to clear
            self.wait_not_busy(channel, 5000)?;
            
            let status = unsafe { inb(base + reg::STATUS) };
            
            if status & status::ERR != 0 {
                return Err("ATAPI command error");
            }
            
            if status & status::DRQ == 0 {
                break; // Command complete
            }
            
            // Read byte count
            let count_low = unsafe { inb(base + reg::LBA_MID) } as usize;
            let count_high = unsafe { inb(base + reg::LBA_HIGH) } as usize;
            let byte_count = count_low | (count_high << 8);
            
            // Read data
            let words = (byte_count + 1) / 2;
            for _ in 0..words {
                if bytes_read + 2 > buffer_size {
                    // Buffer overflow, discard remaining
                    let _ = unsafe { inw(base + reg::DATA) };
                } else {
                    let word = unsafe { inw(base + reg::DATA) };
                    buffer[bytes_read] = word as u8;
                    buffer[bytes_read + 1] = (word >> 8) as u8;
                    bytes_read += 2;
                }
            }
        }
        
        Ok(bytes_read)
    }
    
    // =========================================================================
    // Waiting/Timing Functions
    // =========================================================================
    
    /// Wait for BSY flag to clear
    fn wait_not_busy(&self, channel: Channel, timeout_ms: u32) -> Result<(), &'static str> {
        let base = channel.base_port();
        
        for _ in 0..(timeout_ms * 100) {
            let status = unsafe { inb(base + reg::STATUS) };
            if status & status::BSY == 0 {
                return Ok(());
            }
            // Small delay (~10μs)
            for _ in 0..10 { 
                unsafe { core::arch::asm!("nop"); }
            }
        }
        
        Err("IDE timeout waiting for BSY clear")
    }
    
    /// Wait for DRDY flag
    fn wait_ready(&self, channel: Channel, timeout_ms: u32) -> Result<(), &'static str> {
        let base = channel.base_port();
        
        for _ in 0..(timeout_ms * 100) {
            let status = unsafe { inb(base + reg::STATUS) };
            if status & status::BSY == 0 && status & status::DRDY != 0 {
                return Ok(());
            }
            for _ in 0..10 { 
                unsafe { core::arch::asm!("nop"); }
            }
        }
        
        Err("IDE timeout waiting for DRDY")
    }
    
    /// Wait for DRQ flag (data request)
    fn wait_drq(&self, channel: Channel, timeout_ms: u32) -> Result<(), &'static str> {
        let base = channel.base_port();
        
        for _ in 0..(timeout_ms * 100) {
            let status = unsafe { inb(base + reg::STATUS) };
            
            if status & status::ERR != 0 {
                return Err("IDE error during wait");
            }
            
            if status & status::BSY == 0 && status & status::DRQ != 0 {
                return Ok(());
            }
            
            for _ in 0..10 { 
                unsafe { core::arch::asm!("nop"); }
            }
        }
        
        Err("IDE timeout waiting for DRQ")
    }
    
    /// 400ns delay (read alternate status 4 times)
    fn delay_400ns(&self) {
        // Reading alternate status takes ~100ns, do 4 times for ~400ns
        unsafe {
            let _ = inb(PRIMARY_CTRL);
            let _ = inb(PRIMARY_CTRL);
            let _ = inb(PRIMARY_CTRL);
            let _ = inb(PRIMARY_CTRL);
        }
    }
    
    // =========================================================================
    // Public Accessors
    // =========================================================================
    
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
    
    /// Get device info for a specific channel/device
    pub fn get_device_info(&self, channel: Channel, device: Device) -> &DeviceInfo {
        match (channel, device) {
            (Channel::Primary, Device::Master) => &self.primary_master,
            (Channel::Primary, Device::Slave) => &self.primary_slave,
            (Channel::Secondary, Device::Master) => &self.secondary_master,
            (Channel::Secondary, Device::Slave) => &self.secondary_slave,
        }
    }
    
    /// Check if a device is present
    pub fn device_present(&self, channel: Channel, device: Device) -> bool {
        self.get_device_info(channel, device).device_type != DeviceType::None
    }
}

// =============================================================================
// Global Instance
// =============================================================================

pub static mut IDE_CONTROLLER: IdeController = IdeController::new();

/// Initialize the IDE controller
pub fn init() -> Result<(), &'static str> {
    unsafe { IDE_CONTROLLER.init() }
}

/// Get the IDE controller instance
pub fn controller() -> &'static IdeController {
    unsafe { &IDE_CONTROLLER }
}

/// Get mutable IDE controller instance
pub fn controller_mut() -> &'static mut IdeController {
    unsafe { &mut IDE_CONTROLLER }
}
