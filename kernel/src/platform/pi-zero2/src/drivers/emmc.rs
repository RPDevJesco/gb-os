//! EMMC/SD Card Driver for BCM2835/BCM2837
//!
//! Implements SD card access via the EMMC controller on Pi Zero 2W.
//! Based on the SD Host Controller Simplified Specification.

use crate::hal::storage::BlockDevice;

// ============================================================================
// BCM2835 EMMC Registers (Memory-mapped at peripheral base + 0x300000)
// ============================================================================

const PERIPHERAL_BASE: usize = 0x3F00_0000;  // BCM2837 on Pi Zero 2W
const EMMC_BASE: usize = PERIPHERAL_BASE + 0x30_0000;

// EMMC register offsets
const EMMC_ARG2: usize = 0x00;
const EMMC_BLKSIZECNT: usize = 0x04;
const EMMC_ARG1: usize = 0x08;
const EMMC_CMDTM: usize = 0x0C;
const EMMC_RESP0: usize = 0x10;
const EMMC_RESP1: usize = 0x14;
const EMMC_RESP2: usize = 0x18;
const EMMC_RESP3: usize = 0x1C;
const EMMC_DATA: usize = 0x20;
const EMMC_STATUS: usize = 0x24;
const EMMC_CONTROL0: usize = 0x28;
const EMMC_CONTROL1: usize = 0x2C;
const EMMC_INTERRUPT: usize = 0x30;
const EMMC_IRPT_MASK: usize = 0x34;
const EMMC_IRPT_EN: usize = 0x38;
const EMMC_CONTROL2: usize = 0x3C;
const EMMC_SLOTISR_VER: usize = 0xFC;

// Status register bits
const SR_DAT_INHIBIT: u32 = 1 << 1;
const SR_CMD_INHIBIT: u32 = 1 << 0;
const SR_READ_AVAILABLE: u32 = 1 << 11;
const SR_WRITE_AVAILABLE: u32 = 1 << 10;

// Interrupt flags
const INT_CMD_DONE: u32 = 1 << 0;
const INT_DATA_DONE: u32 = 1 << 1;
const INT_READ_RDY: u32 = 1 << 5;
const INT_WRITE_RDY: u32 = 1 << 4;
const INT_ERROR_MASK: u32 = 0xFFFF_0000;

// Commands
const CMD_GO_IDLE: u32 = 0;
const CMD_ALL_SEND_CID: u32 = 2;
const CMD_SEND_REL_ADDR: u32 = 3;
const CMD_SELECT_CARD: u32 = 7;
const CMD_SEND_IF_COND: u32 = 8;
const CMD_SEND_CSD: u32 = 9;
const CMD_STOP_TRANS: u32 = 12;
const CMD_SET_BLOCKLEN: u32 = 16;
const CMD_READ_SINGLE: u32 = 17;
const CMD_READ_MULTI: u32 = 18;
const CMD_WRITE_SINGLE: u32 = 24;
const CMD_WRITE_MULTI: u32 = 25;
const CMD_APP_CMD: u32 = 55;
const ACMD_SD_SEND_OP_COND: u32 = 41;
const ACMD_SET_BUS_WIDTH: u32 = 6;

// Command flags
const CMD_NEED_APP: u32 = 0x8000_0000;
const CMD_RSPNS_48: u32 = 0x0002_0000;
const CMD_RSPNS_136: u32 = 0x0001_0000;
const CMD_RSPNS_48B: u32 = 0x0003_0000;
const CMD_DATA_READ: u32 = 0x0000_0010;
const CMD_DATA_WRITE: u32 = 0x0000_0000;
const CMD_ISDATA: u32 = 0x0000_0020;
const CMD_MULTI_BLOCK: u32 = 0x0000_0022;

// ============================================================================
// EMMC Driver State
// ============================================================================

pub struct EmmcDriver {
    initialized: bool,
    card_rca: u32,      // Relative Card Address
    is_sdhc: bool,      // SDHC/SDXC (block addressing) vs SD (byte addressing)
    sector_count: u64,
}

impl EmmcDriver {
    pub const fn new() -> Self {
        Self {
            initialized: false,
            card_rca: 0,
            is_sdhc: false,
            sector_count: 0,
        }
    }
    
    /// Initialize the EMMC controller and SD card
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Reset controller
        self.write_reg(EMMC_CONTROL0, 0);
        self.write_reg(EMMC_CONTROL1, 0);
        
        // Wait for reset
        self.delay(10000);
        
        // Set clock to identification mode (400 KHz)
        self.set_clock(400_000)?;
        
        // Enable interrupts
        self.write_reg(EMMC_IRPT_EN, 0xFFFF_FFFF);
        self.write_reg(EMMC_IRPT_MASK, 0xFFFF_FFFF);
        
        // Send GO_IDLE_STATE (CMD0)
        self.send_command(CMD_GO_IDLE, 0)?;
        
        // Send SEND_IF_COND (CMD8) to check for SDHC
        let cmd8_arg = 0x1AA;  // Check pattern + voltage
        match self.send_command(CMD_SEND_IF_COND | CMD_RSPNS_48, cmd8_arg) {
            Ok(_) => {
                let resp = self.read_reg(EMMC_RESP0);
                if (resp & 0xFFF) != 0x1AA {
                    return Err("CMD8 pattern mismatch");
                }
                self.is_sdhc = true;
            }
            Err(_) => {
                // Older SD 1.x card
                self.is_sdhc = false;
            }
        }
        
        // Send ACMD41 to initialize card
        let mut ocr = 0u32;
        for _ in 0..100 {
            // Send APP_CMD first
            self.send_command(CMD_APP_CMD | CMD_RSPNS_48, 0)?;
            
            // ACMD41 with HCS bit for SDHC
            let acmd41_arg = 0x40FF_8000 | if self.is_sdhc { 0x4000_0000 } else { 0 };
            self.send_command(ACMD_SD_SEND_OP_COND | CMD_RSPNS_48, acmd41_arg)?;
            
            ocr = self.read_reg(EMMC_RESP0);
            if (ocr & 0x8000_0000) != 0 {
                // Card is ready
                break;
            }
            self.delay(10000);
        }
        
        if (ocr & 0x8000_0000) == 0 {
            return Err("Card init timeout");
        }
        
        // Check CCS bit for SDHC
        self.is_sdhc = (ocr & 0x4000_0000) != 0;
        
        // Get CID (CMD2)
        self.send_command(CMD_ALL_SEND_CID | CMD_RSPNS_136, 0)?;
        
        // Get RCA (CMD3)
        self.send_command(CMD_SEND_REL_ADDR | CMD_RSPNS_48, 0)?;
        self.card_rca = self.read_reg(EMMC_RESP0) & 0xFFFF_0000;
        
        // Select card (CMD7)
        self.send_command(CMD_SELECT_CARD | CMD_RSPNS_48B, self.card_rca)?;
        
        // Set block size to 512 bytes
        self.send_command(CMD_SET_BLOCKLEN | CMD_RSPNS_48, 512)?;
        self.write_reg(EMMC_BLKSIZECNT, 512);
        
        // Set bus width to 4 bits (ACMD6)
        self.send_command(CMD_APP_CMD | CMD_RSPNS_48, self.card_rca)?;
        self.send_command(ACMD_SET_BUS_WIDTH | CMD_RSPNS_48, 2)?;  // 2 = 4-bit
        
        // Increase clock speed to 25 MHz
        self.set_clock(25_000_000)?;
        
        // Get card capacity from CSD
        self.sector_count = self.get_capacity()?;
        
        self.initialized = true;
        Ok(())
    }
    
    /// Set EMMC clock frequency
    fn set_clock(&self, freq: u32) -> Result<(), &'static str> {
        // Calculate divisor (base clock is typically 41.6 MHz or 50 MHz)
        let base_clock = 50_000_000u32;
        let divisor = (base_clock / freq).max(2);
        
        // Disable clock
        let mut ctrl1 = self.read_reg(EMMC_CONTROL1);
        ctrl1 &= !(0xFF << 8);  // Clear divider
        ctrl1 &= !(1 << 2);     // Disable clock
        self.write_reg(EMMC_CONTROL1, ctrl1);
        
        // Set divider
        ctrl1 |= ((divisor & 0xFF) << 8);
        ctrl1 |= (1 << 0);  // Internal clock enable
        self.write_reg(EMMC_CONTROL1, ctrl1);
        
        // Wait for clock stable
        for _ in 0..10000 {
            if (self.read_reg(EMMC_CONTROL1) & (1 << 1)) != 0 {
                break;
            }
            self.delay(10);
        }
        
        // Enable clock to card
        ctrl1 |= (1 << 2);
        self.write_reg(EMMC_CONTROL1, ctrl1);
        
        self.delay(10000);
        Ok(())
    }
    
    /// Send a command to the card
    fn send_command(&self, cmd: u32, arg: u32) -> Result<(), &'static str> {
        // Wait for command line free
        self.wait_status(SR_CMD_INHIBIT, false, 100000)?;
        
        // Clear interrupts
        self.write_reg(EMMC_INTERRUPT, 0xFFFF_FFFF);
        
        // Set argument
        self.write_reg(EMMC_ARG1, arg);
        
        // Send command (command index in bits 29:24)
        let cmd_idx = cmd & 0x3F;
        let cmd_flags = cmd & 0xFFFF_FFC0;
        self.write_reg(EMMC_CMDTM, (cmd_idx << 24) | cmd_flags);
        
        // Wait for command complete
        self.wait_interrupt(INT_CMD_DONE, 100000)?;
        
        Ok(())
    }
    
    /// Wait for status register condition
    fn wait_status(&self, mask: u32, set: bool, timeout: u32) -> Result<(), &'static str> {
        for _ in 0..timeout {
            let status = self.read_reg(EMMC_STATUS);
            if set {
                if (status & mask) != 0 { return Ok(()); }
            } else {
                if (status & mask) == 0 { return Ok(()); }
            }
            self.delay(1);
        }
        Err("Status timeout")
    }
    
    /// Wait for interrupt
    fn wait_interrupt(&self, mask: u32, timeout: u32) -> Result<(), &'static str> {
        for _ in 0..timeout {
            let irq = self.read_reg(EMMC_INTERRUPT);
            if (irq & INT_ERROR_MASK) != 0 {
                self.write_reg(EMMC_INTERRUPT, irq);
                return Err("EMMC error");
            }
            if (irq & mask) != 0 {
                self.write_reg(EMMC_INTERRUPT, mask);
                return Ok(());
            }
            self.delay(1);
        }
        Err("Interrupt timeout")
    }
    
    /// Get card capacity in sectors
    fn get_capacity(&self) -> Result<u64, &'static str> {
        // Send CMD9 to get CSD
        self.send_command(CMD_SEND_CSD | CMD_RSPNS_136, self.card_rca)?;
        
        let csd0 = self.read_reg(EMMC_RESP0);
        let csd1 = self.read_reg(EMMC_RESP1);
        let csd2 = self.read_reg(EMMC_RESP2);
        let _csd3 = self.read_reg(EMMC_RESP3);
        
        // CSD version is in bits 127:126 (in csd3 upper bits)
        // For SDHC (CSD v2), capacity is in C_SIZE field
        if self.is_sdhc {
            // C_SIZE is in bits 69:48, spans csd1 and csd2
            let c_size = ((csd2 & 0x3F) << 16) | (csd1 >> 16);
            // Capacity = (C_SIZE + 1) * 512KB
            Ok(((c_size as u64) + 1) * 1024)  // Returns sectors (512 bytes each)
        } else {
            // CSD v1 - more complex calculation
            let c_size = ((csd2 & 0x3FF) << 2) | (csd1 >> 30);
            let c_size_mult = (csd1 >> 15) & 0x07;
            let read_bl_len = (csd2 >> 16) & 0x0F;
            
            let mult = 1u64 << (c_size_mult + 2);
            let block_len = 1u64 << read_bl_len;
            let capacity_bytes = ((c_size as u64) + 1) * mult * block_len;
            Ok(capacity_bytes / 512)
        }
    }
    
    /// Read register
    #[inline]
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe {
            core::ptr::read_volatile((EMMC_BASE + offset) as *const u32)
        }
    }
    
    /// Write register
    #[inline]
    fn write_reg(&self, offset: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile((EMMC_BASE + offset) as *mut u32, value);
        }
    }
    
    /// Simple delay loop
    fn delay(&self, count: u32) {
        for _ in 0..count {
            unsafe { core::arch::asm!("nop"); }
        }
    }
}

impl BlockDevice for EmmcDriver {
    fn read_sectors(&self, lba: u64, count: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("EMMC not initialized");
        }
        
        if buffer.len() < (count as usize * 512) {
            return Err("Buffer too small");
        }
        
        // Set block count
        self.write_reg(EMMC_BLKSIZECNT, (count << 16) | 512);
        
        // Calculate address (byte address for SD, block address for SDHC)
        let addr = if self.is_sdhc { lba as u32 } else { (lba * 512) as u32 };
        
        // Send read command
        let cmd = if count == 1 {
            CMD_READ_SINGLE | CMD_RSPNS_48 | CMD_ISDATA | CMD_DATA_READ
        } else {
            CMD_READ_MULTI | CMD_RSPNS_48 | CMD_ISDATA | CMD_DATA_READ | CMD_MULTI_BLOCK
        };
        
        self.send_command(cmd, addr)?;
        
        // Read data
        let mut offset = 0;
        for _ in 0..count {
            // Wait for data ready
            self.wait_interrupt(INT_READ_RDY, 100000)?;
            
            // Read 512 bytes (128 x 4 bytes)
            for _ in 0..128 {
                let word = self.read_reg(EMMC_DATA);
                buffer[offset..offset+4].copy_from_slice(&word.to_le_bytes());
                offset += 4;
            }
        }
        
        // Wait for transfer complete
        self.wait_interrupt(INT_DATA_DONE, 100000)?;
        
        // Stop transmission for multi-block
        if count > 1 {
            self.send_command(CMD_STOP_TRANS | CMD_RSPNS_48B, 0)?;
        }
        
        Ok(())
    }
    
    fn write_sectors(&self, lba: u64, count: u32, buffer: &[u8]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("EMMC not initialized");
        }
        
        if buffer.len() < (count as usize * 512) {
            return Err("Buffer too small");
        }
        
        // Set block count
        self.write_reg(EMMC_BLKSIZECNT, (count << 16) | 512);
        
        // Calculate address
        let addr = if self.is_sdhc { lba as u32 } else { (lba * 512) as u32 };
        
        // Send write command
        let cmd = if count == 1 {
            CMD_WRITE_SINGLE | CMD_RSPNS_48 | CMD_ISDATA | CMD_DATA_WRITE
        } else {
            CMD_WRITE_MULTI | CMD_RSPNS_48 | CMD_ISDATA | CMD_DATA_WRITE | CMD_MULTI_BLOCK
        };
        
        self.send_command(cmd, addr)?;
        
        // Write data
        let mut offset = 0;
        for _ in 0..count {
            // Wait for write ready
            self.wait_interrupt(INT_WRITE_RDY, 100000)?;
            
            // Write 512 bytes
            for _ in 0..128 {
                let word = u32::from_le_bytes([
                    buffer[offset], buffer[offset+1], 
                    buffer[offset+2], buffer[offset+3]
                ]);
                self.write_reg(EMMC_DATA, word);
                offset += 4;
            }
        }
        
        // Wait for transfer complete
        self.wait_interrupt(INT_DATA_DONE, 100000)?;
        
        // Stop transmission for multi-block
        if count > 1 {
            self.send_command(CMD_STOP_TRANS | CMD_RSPNS_48B, 0)?;
        }
        
        Ok(())
    }
    
    fn sector_count(&self) -> u64 {
        self.sector_count
    }
    
    fn is_ready(&self) -> bool {
        self.initialized
    }
}

// Global EMMC instance
static mut EMMC: EmmcDriver = EmmcDriver::new();

/// Get global EMMC driver
pub fn get_emmc() -> &'static mut EmmcDriver {
    unsafe { &mut EMMC }
}

/// Initialize EMMC
pub fn init() -> Result<(), &'static str> {
    unsafe { EMMC.init() }
}
