//! SD Card driver via SDHOST controller
//!
//! This module provides low-level SD card access using the BCM283x SDHOST
//! controller (not EMMC). The SDHOST controller is simpler and more reliable
//! for basic SD card operations.
//!
//! Supports:
//! - SD/SDHC/SDXC cards
//! - Single block reads (512 bytes)
//!
//! Note: This driver uses GPIO 48-53 which must be configured for ALT0.

use crate::platform_core::mmio::{mmio_read, mmio_write, delay_ms, PERIPHERAL_BASE};
use crate::hal::gpio::configure_for_sd;
use crate::hal::mailbox::{set_power_state, device};

// ============================================================================
// SDHOST Register Addresses
// ============================================================================

const SDHOST_BASE: usize = PERIPHERAL_BASE + 0x0020_2000;

const SDHOST_CMD: usize = SDHOST_BASE + 0x00;
const SDHOST_ARG: usize = SDHOST_BASE + 0x04;
const SDHOST_TOUT: usize = SDHOST_BASE + 0x08;
const SDHOST_CDIV: usize = SDHOST_BASE + 0x0C;
const SDHOST_RSP0: usize = SDHOST_BASE + 0x10;
const SDHOST_HSTS: usize = SDHOST_BASE + 0x20;
const SDHOST_VDD: usize = SDHOST_BASE + 0x30;
const SDHOST_HCFG: usize = SDHOST_BASE + 0x38;
const SDHOST_HBCT: usize = SDHOST_BASE + 0x3C;
const SDHOST_DATA: usize = SDHOST_BASE + 0x40;
const SDHOST_HBLC: usize = SDHOST_BASE + 0x50;

// ============================================================================
// SDHOST Command Flags
// ============================================================================

const SDHOST_CMD_NEW: u32 = 0x8000;
const SDHOST_CMD_FAIL: u32 = 0x4000;
const SDHOST_CMD_BUSY: u32 = 0x0800;
const SDHOST_CMD_NO_RSP: u32 = 0x0400;
const SDHOST_CMD_LONG_RSP: u32 = 0x0200;
const SDHOST_CMD_READ: u32 = 0x0040;

// ============================================================================
// SDHOST Status Flags
// ============================================================================

const SDHOST_HSTS_DATA_FLAG: u32 = 0x0001;
const SDHOST_HSTS_ERROR_MASK: u32 = 0x7F8;

// ============================================================================
// SDHOST Configuration
// ============================================================================

const SDHOST_HCFG_SLOW_CARD: u32 = 0x0002;
const SDHOST_HCFG_INTBUS: u32 = 0x0001;

// ============================================================================
// SD Commands
// ============================================================================

/// SD command indices
pub mod cmd {
    pub const GO_IDLE_STATE: u32 = 0;
    pub const SEND_IF_COND: u32 = 8;
    pub const SEND_CSD: u32 = 9;
    pub const SEND_CID: u32 = 10;
    pub const STOP_TRANSMISSION: u32 = 12;
    pub const SET_BLOCKLEN: u32 = 16;
    pub const READ_SINGLE_BLOCK: u32 = 17;
    pub const READ_MULTIPLE_BLOCK: u32 = 18;
    pub const APP_CMD: u32 = 55;
    pub const SD_SEND_OP_COND: u32 = 41; // ACMD41
    pub const ALL_SEND_CID: u32 = 2;
    pub const SEND_RELATIVE_ADDR: u32 = 3;
    pub const SELECT_CARD: u32 = 7;
}

// ============================================================================
// Sector Size
// ============================================================================

/// Standard sector size
pub const SECTOR_SIZE: usize = 512;

// ============================================================================
// SD Card Driver
// ============================================================================

/// SD Card driver state
pub struct SdCard {
    /// Card has been successfully initialized
    initialized: bool,
    /// Card is SDHC/SDXC (block addressing) vs SD (byte addressing)
    is_sdhc: bool,
    /// Relative Card Address (RCA) - upper 16 bits
    rca: u32,
}

impl SdCard {
    /// Create a new uninitialized SD card driver
    pub const fn new() -> Self {
        Self {
            initialized: false,
            is_sdhc: true,
            rca: 0,
        }
    }

    /// Check if card is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Check if card is SDHC/SDXC
    pub fn is_sdhc(&self) -> bool {
        self.is_sdhc
    }

    /// Clear status register errors
    fn clear_status(&self) {
        mmio_write(SDHOST_HSTS, SDHOST_HSTS_ERROR_MASK);
    }

    /// Reset the SDHOST controller
    fn reset(&self) {
        mmio_write(SDHOST_CMD, 0);
        mmio_write(SDHOST_ARG, 0);
        mmio_write(SDHOST_TOUT, 0xF0_0000);
        mmio_write(SDHOST_CDIV, 0);
        mmio_write(SDHOST_HSTS, SDHOST_HSTS_ERROR_MASK);
        mmio_write(SDHOST_HCFG, 0);
        mmio_write(SDHOST_HBCT, 0);
        mmio_write(SDHOST_HBLC, 0);

        // Power on VDD
        mmio_write(SDHOST_VDD, 1);
        delay_ms(10);

        // Configure for slow card initially
        mmio_write(SDHOST_HCFG, SDHOST_HCFG_SLOW_CARD | SDHOST_HCFG_INTBUS);

        // Clock divider for ~400kHz identification mode
        mmio_write(SDHOST_CDIV, 0x148);
        delay_ms(10);
    }

    /// Wait for command to complete
    fn wait_cmd(&self) -> Result<(), &'static str> {
        for _ in 0..50_000 {
            let cmd = mmio_read(SDHOST_CMD);

            if (cmd & SDHOST_CMD_NEW) == 0 {
                let hsts = mmio_read(SDHOST_HSTS);
                if (hsts & 0x40) != 0 {
                    self.clear_status();
                    return Err("Timeout");
                }
                if (hsts & 0x10) != 0 {
                    self.clear_status();
                    return Err("CRC error");
                }
                return Ok(());
            }

            if (cmd & SDHOST_CMD_FAIL) != 0 {
                self.clear_status();
                return Err("Command failed");
            }
        }
        Err("Wait timeout")
    }

    /// Send a command and get response
    fn send_cmd(&mut self, cmd_idx: u32, arg: u32, flags: u32) -> Result<u32, &'static str> {
        self.clear_status();
        mmio_write(SDHOST_ARG, arg);
        mmio_write(SDHOST_CMD, (cmd_idx & 0x3F) | flags | SDHOST_CMD_NEW);
        self.wait_cmd()?;
        Ok(mmio_read(SDHOST_RSP0))
    }

    /// Initialize the SD card
    ///
    /// This performs the full initialization sequence:
    /// 1. Configure GPIO pins
    /// 2. Power on via mailbox
    /// 3. Reset controller
    /// 4. Send CMD0 (GO_IDLE_STATE)
    /// 5. Send CMD8 (SEND_IF_COND) to detect SD v2
    /// 6. Send ACMD41 loop to wait for card ready
    /// 7. Get CID and RCA
    /// 8. Select card and switch to high speed
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Configure GPIO for SDHOST
        configure_for_sd();

        // Power on SD card via mailbox
        set_power_state(device::SD_CARD, true);

        // Reset controller
        self.reset();

        // CMD0 - GO_IDLE_STATE (no response)
        mmio_write(SDHOST_ARG, 0);
        mmio_write(SDHOST_CMD, cmd::GO_IDLE_STATE | SDHOST_CMD_NO_RSP | SDHOST_CMD_NEW);
        delay_ms(50);
        self.clear_status();

        // CMD8 - SEND_IF_COND (check for SD v2)
        match self.send_cmd(cmd::SEND_IF_COND, 0x1AA, 0) {
            Ok(resp) => {
                self.is_sdhc = (resp & 0xFF) == 0xAA;
            }
            Err(_) => {
                // SD v1 card - no SDHC support
                self.is_sdhc = false;
                self.clear_status();
            }
        }

        // ACMD41 loop - wait for card to be ready
        for _ in 0..50 {
            // CMD55 - APP_CMD prefix
            let _ = self.send_cmd(cmd::APP_CMD, 0, 0);

            // ACMD41 - SD_SEND_OP_COND
            let hcs = if self.is_sdhc { 0x4000_0000 } else { 0 };
            if let Ok(ocr) = self.send_cmd(cmd::SD_SEND_OP_COND, 0x00FF_8000 | hcs, 0) {
                if (ocr & 0x8000_0000) != 0 {
                    // Card is ready
                    self.is_sdhc = (ocr & 0x4000_0000) != 0;
                    break;
                }
            }
            delay_ms(10);
        }

        // CMD2 - ALL_SEND_CID (get card identification)
        self.send_cmd(cmd::ALL_SEND_CID, 0, SDHOST_CMD_LONG_RSP)?;

        // CMD3 - SEND_RELATIVE_ADDR (get RCA)
        let resp = self.send_cmd(cmd::SEND_RELATIVE_ADDR, 0, 0)?;
        self.rca = resp & 0xFFFF_0000;

        // CMD7 - SELECT_CARD (select this card)
        self.send_cmd(cmd::SELECT_CARD, self.rca, SDHOST_CMD_BUSY)?;

        // Switch to high-speed clock
        mmio_write(SDHOST_CDIV, 4);

        // Set block size
        mmio_write(SDHOST_HBCT, SECTOR_SIZE as u32);

        self.initialized = true;
        Ok(())
    }

    /// Read a single 512-byte sector
    ///
    /// # Arguments
    /// * `lba` - Logical Block Address (sector number)
    /// * `buffer` - Buffer to read into (must be exactly 512 bytes)
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(&str)` on failure
    pub fn read_sector(&mut self, lba: u32, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Not initialized");
        }

        // Set block count
        mmio_write(SDHOST_HBCT, SECTOR_SIZE as u32);
        mmio_write(SDHOST_HBLC, 1);

        // Calculate address (byte address for SD, block address for SDHC)
        let addr = if self.is_sdhc { lba } else { lba * SECTOR_SIZE as u32 };

        // Send CMD17 - READ_SINGLE_BLOCK
        self.clear_status();
        mmio_write(SDHOST_ARG, addr);
        mmio_write(SDHOST_CMD, cmd::READ_SINGLE_BLOCK | SDHOST_CMD_READ | SDHOST_CMD_NEW);
        self.wait_cmd()?;

        // Read data from FIFO
        let mut idx = 0;
        for _ in 0..500_000 {
            if idx >= SECTOR_SIZE {
                break;
            }

            let hsts = mmio_read(SDHOST_HSTS);
            if (hsts & SDHOST_HSTS_DATA_FLAG) != 0 {
                let word = mmio_read(SDHOST_DATA);
                buffer[idx] = (word >> 0) as u8;
                buffer[idx + 1] = (word >> 8) as u8;
                buffer[idx + 2] = (word >> 16) as u8;
                buffer[idx + 3] = (word >> 24) as u8;
                idx += 4;
            }
        }

        self.clear_status();

        if idx < SECTOR_SIZE {
            return Err("Data timeout");
        }

        Ok(())
    }

    /// Read multiple sectors into a buffer
    ///
    /// This is a convenience wrapper that calls read_sector multiple times.
    /// For better performance, a multi-block read command could be implemented.
    pub fn read_sectors(&mut self, start_lba: u32, buffer: &mut [u8]) -> Result<usize, &'static str> {
        let num_sectors = buffer.len() / SECTOR_SIZE;
        let mut sector_buf = [0u8; SECTOR_SIZE];

        for i in 0..num_sectors {
            self.read_sector(start_lba + i as u32, &mut sector_buf)?;
            let offset = i * SECTOR_SIZE;
            buffer[offset..offset + SECTOR_SIZE].copy_from_slice(&sector_buf);
        }

        Ok(num_sectors * SECTOR_SIZE)
    }
}
