//! ATA PIO Disk Driver
//!
//! Simple ATA driver using PIO mode for reading sectors.
//! Works with IDE/PATA drives on vintage hardware.
//!
//! This driver supports:
//! - Primary and secondary ATA channels
//! - LBA28 addressing (up to 128GB)
//! - PIO read operations

use core::arch::asm;

/// Primary ATA channel base port
const ATA_PRIMARY: u16 = 0x1F0;
/// Secondary ATA channel base port
const ATA_SECONDARY: u16 = 0x170;

/// ATA register offsets
const ATA_REG_DATA: u16 = 0;
const ATA_REG_ERROR: u16 = 1;
const ATA_REG_SECCOUNT: u16 = 2;
const ATA_REG_LBA_LO: u16 = 3;
const ATA_REG_LBA_MID: u16 = 4;
const ATA_REG_LBA_HI: u16 = 5;
const ATA_REG_DRIVE: u16 = 6;
const ATA_REG_STATUS: u16 = 7;
const ATA_REG_COMMAND: u16 = 7;

/// ATA control register (primary)
const ATA_PRIMARY_CTRL: u16 = 0x3F6;
/// ATA control register (secondary)
const ATA_SECONDARY_CTRL: u16 = 0x376;

/// ATA commands
const ATA_CMD_READ_PIO: u8 = 0x20;
const ATA_CMD_IDENTIFY: u8 = 0xEC;

/// ATA status bits
const ATA_SR_BSY: u8 = 0x80;  // Busy
const ATA_SR_DRDY: u8 = 0x40; // Drive ready
const ATA_SR_DRQ: u8 = 0x08;  // Data request
const ATA_SR_ERR: u8 = 0x01;  // Error

/// Drive selection
const ATA_DRIVE_MASTER: u8 = 0xE0;
const ATA_DRIVE_SLAVE: u8 = 0xF0;

/// ATA drive identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtaDrive {
    PrimaryMaster,
    PrimarySlave,
    SecondaryMaster,
    SecondarySlave,
}

impl AtaDrive {
    /// Get the base I/O port for this drive's channel
    fn base_port(&self) -> u16 {
        match self {
            AtaDrive::PrimaryMaster | AtaDrive::PrimarySlave => ATA_PRIMARY,
            AtaDrive::SecondaryMaster | AtaDrive::SecondarySlave => ATA_SECONDARY,
        }
    }

    /// Get the control port for this drive's channel
    fn ctrl_port(&self) -> u16 {
        match self {
            AtaDrive::PrimaryMaster | AtaDrive::PrimarySlave => ATA_PRIMARY_CTRL,
            AtaDrive::SecondaryMaster | AtaDrive::SecondarySlave => ATA_SECONDARY_CTRL,
        }
    }

    /// Get the drive select byte
    fn drive_select(&self) -> u8 {
        match self {
            AtaDrive::PrimaryMaster | AtaDrive::SecondaryMaster => ATA_DRIVE_MASTER,
            AtaDrive::PrimarySlave | AtaDrive::SecondarySlave => ATA_DRIVE_SLAVE,
        }
    }
}

/// ATA driver instance
pub struct Ata {
    drive: AtaDrive,
    base: u16,
    ctrl: u16,
}

/// Drive identification data
#[derive(Debug)]
pub struct DriveInfo {
    pub present: bool,
    pub model: [u8; 40],
    pub sectors: u32,
}

impl Ata {
    /// Create a new ATA driver for the specified drive
    pub fn new(drive: AtaDrive) -> Self {
        Self {
            drive,
            base: drive.base_port(),
            ctrl: drive.ctrl_port(),
        }
    }

    /// Check if drive is present and identify it
    pub fn identify(&mut self) -> Option<DriveInfo> {
        // Select drive
        self.outb(ATA_REG_DRIVE, self.drive.drive_select());
        self.delay();

        // Clear sector count and LBA registers
        self.outb(ATA_REG_SECCOUNT, 0);
        self.outb(ATA_REG_LBA_LO, 0);
        self.outb(ATA_REG_LBA_MID, 0);
        self.outb(ATA_REG_LBA_HI, 0);

        // Send IDENTIFY command
        self.outb(ATA_REG_COMMAND, ATA_CMD_IDENTIFY);
        self.delay();

        // Check if drive exists
        let status = self.inb(ATA_REG_STATUS);
        if status == 0 {
            return None; // No drive
        }

        // Wait for BSY to clear
        if !self.wait_not_busy() {
            return None;
        }

        // Check for ATAPI (we only support ATA)
        let lba_mid = self.inb(ATA_REG_LBA_MID);
        let lba_hi = self.inb(ATA_REG_LBA_HI);
        if lba_mid != 0 || lba_hi != 0 {
            return None; // ATAPI or SATA, not supported
        }

        // Wait for DRQ or ERR
        loop {
            let status = self.inb(ATA_REG_STATUS);
            if status & ATA_SR_ERR != 0 {
                return None;
            }
            if status & ATA_SR_DRQ != 0 {
                break;
            }
        }

        // Read identification data (256 words = 512 bytes)
        let mut data = [0u16; 256];
        for word in data.iter_mut() {
            *word = self.inw(ATA_REG_DATA);
        }

        // Extract model string (words 27-46)
        let mut model = [0u8; 40];
        for i in 0..20 {
            let word = data[27 + i];
            model[i * 2] = (word >> 8) as u8;
            model[i * 2 + 1] = (word & 0xFF) as u8;
        }

        // Get total sectors (words 60-61 for LBA28)
        let sectors = (data[61] as u32) << 16 | (data[60] as u32);

        Some(DriveInfo {
            present: true,
            model,
            sectors,
        })
    }

    /// Read sectors from the drive using LBA28
    ///
    /// # Arguments
    /// * `lba` - Starting logical block address
    /// * `count` - Number of sectors to read (1-255, 0 means 256)
    /// * `buffer` - Buffer to read into (must be count * 512 bytes)
    ///
    /// # Returns
    /// Number of sectors successfully read, or error
    pub fn read_sectors(&mut self, lba: u32, count: u8, buffer: &mut [u8]) -> Result<u8, &'static str> {
        if buffer.len() < (count as usize) * 512 {
            return Err("Buffer too small");
        }

        if lba > 0x0FFFFFFF {
            return Err("LBA out of range for LBA28");
        }

        let actual_count = if count == 0 { 256u16 } else { count as u16 };

        // Select drive and set LBA mode + high nibble of LBA
        let drive_byte = self.drive.drive_select() | 0x40 | ((lba >> 24) & 0x0F) as u8;
        self.outb(ATA_REG_DRIVE, drive_byte);
        self.delay();

        // Wait for drive ready
        if !self.wait_ready() {
            return Err("Drive not ready");
        }

        // Set sector count
        self.outb(ATA_REG_SECCOUNT, count);

        // Set LBA address
        self.outb(ATA_REG_LBA_LO, (lba & 0xFF) as u8);
        self.outb(ATA_REG_LBA_MID, ((lba >> 8) & 0xFF) as u8);
        self.outb(ATA_REG_LBA_HI, ((lba >> 16) & 0xFF) as u8);

        // Send read command
        self.outb(ATA_REG_COMMAND, ATA_CMD_READ_PIO);

        // Read each sector
        for sector in 0..actual_count {
            // Wait for data ready
            if !self.wait_drq() {
                return Err("Timeout waiting for data");
            }

            // Check for error
            let status = self.inb(ATA_REG_STATUS);
            if status & ATA_SR_ERR != 0 {
                return Err("Read error");
            }

            // Read 256 words (512 bytes)
            let offset = (sector as usize) * 512;
            for i in 0..256 {
                let word = self.inw(ATA_REG_DATA);
                buffer[offset + i * 2] = (word & 0xFF) as u8;
                buffer[offset + i * 2 + 1] = (word >> 8) as u8;
            }
        }

        Ok(if count == 0 { 255 } else { count })
    }

    /// Wait for BSY flag to clear
    fn wait_not_busy(&self) -> bool {
        for _ in 0..100000 {
            let status = self.inb(ATA_REG_STATUS);
            if status & ATA_SR_BSY == 0 {
                return true;
            }
        }
        false
    }

    /// Wait for drive ready (BSY clear, DRDY set)
    fn wait_ready(&self) -> bool {
        for _ in 0..100000 {
            let status = self.inb(ATA_REG_STATUS);
            if status & ATA_SR_BSY == 0 && status & ATA_SR_DRDY != 0 {
                return true;
            }
        }
        false
    }

    /// Wait for DRQ flag
    fn wait_drq(&self) -> bool {
        for _ in 0..100000 {
            let status = self.inb(ATA_REG_STATUS);
            if status & ATA_SR_BSY == 0 {
                if status & ATA_SR_DRQ != 0 {
                    return true;
                }
                if status & ATA_SR_ERR != 0 {
                    return false;
                }
            }
        }
        false
    }

    /// 400ns delay (read alternate status 4 times)
    fn delay(&self) {
        for _ in 0..4 {
            self.inb_ctrl();
        }
    }

    /// Read byte from ATA register
    fn inb(&self, reg: u16) -> u8 {
        let port = self.base + reg;
        let value: u8;
        unsafe {
            asm!(
            "in al, dx",
            out("al") value,
            in("dx") port,
            options(nomem, nostack)
            );
        }
        value
    }

    /// Read word from ATA register
    fn inw(&self, reg: u16) -> u16 {
        let port = self.base + reg;
        let value: u16;
        unsafe {
            asm!(
            "in ax, dx",
            out("ax") value,
            in("dx") port,
            options(nomem, nostack)
            );
        }
        value
    }

    /// Write byte to ATA register
    fn outb(&self, reg: u16, value: u8) {
        let port = self.base + reg;
        unsafe {
            asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack)
            );
        }
    }

    /// Read from control register (for delays)
    fn inb_ctrl(&self) -> u8 {
        let value: u8;
        unsafe {
            asm!(
            "in al, dx",
            out("al") value,
            in("dx") self.ctrl,
            options(nomem, nostack)
            );
        }
        value
    }
}

/// Convert BIOS drive number to ATA drive
///
/// BIOS drive 0x80 = first hard disk = Primary Master
/// BIOS drive 0x81 = second hard disk = Primary Slave or Secondary Master
pub fn bios_drive_to_ata(bios_drive: u8) -> Option<AtaDrive> {
    match bios_drive {
        0x80 => Some(AtaDrive::PrimaryMaster),
        0x81 => Some(AtaDrive::PrimarySlave),
        0x82 => Some(AtaDrive::SecondaryMaster),
        0x83 => Some(AtaDrive::SecondarySlave),
        _ => None,
    }
}

/// Detect all ATA drives
pub fn detect_drives() -> [Option<DriveInfo>; 4] {
    let drives = [
        AtaDrive::PrimaryMaster,
        AtaDrive::PrimarySlave,
        AtaDrive::SecondaryMaster,
        AtaDrive::SecondarySlave,
    ];

    let mut results = [None, None, None, None];

    for (i, &drive) in drives.iter().enumerate() {
        let mut ata = Ata::new(drive);
        results[i] = ata.identify();
    }

    results
}
