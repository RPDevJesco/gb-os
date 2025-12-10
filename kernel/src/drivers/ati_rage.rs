//! ATI Rage Mobility P GPU Driver
//!
//! Native driver for the ATI Rage Mobility P AGP 2x (PCI ID 1002:4C4D)
//! found in the Compaq Armada E500 laptop.
//!
//! This driver provides:
//! - Native mode setting (bypassing VESA BIOS)
//! - 2D acceleration (rectangle fill, blits)
//! - Hardware cursor support
//! - Power management integration
//!
//! Based on ATI's RAGE 128 PRO Register Reference Guide (RRG-G04500-C)
//! and the xf86-video-r128/Linux DRM driver sources.

use crate::arch::x86::io::{inb, outb, inl, outl};

// =============================================================================
// PCI Identification
// =============================================================================

/// ATI Vendor ID
pub const ATI_VENDOR_ID: u16 = 0x1002;

/// Rage Mobility P Device ID
pub const RAGE_MOBILITY_P_ID: u16 = 0x4C4D;

/// Compaq Armada E500 Subsystem ID
pub const ARMADA_E500_SUBSYS: u32 = 0xB1600E11;

// =============================================================================
// Memory Map (from PCI BARs)
// =============================================================================

/// PCI Configuration Space ports
const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// Minimum valid MMIO base address (anything below this is suspicious)
const MIN_MMIO_ADDR: u32 = 0x80000000;

/// Maximum valid MMIO base address
const MAX_MMIO_ADDR: u32 = 0xFFF00000;

/// GPU state
pub struct AtiRage {
    /// MMIO base address (from BAR2)
    mmio_base: u32,
    /// Framebuffer base address (from BAR0)
    fb_base: u32,
    /// Framebuffer size in bytes
    fb_size: u32,
    /// Current display width
    width: u32,
    /// Current display height
    height: u32,
    /// Bits per pixel
    bpp: u32,
    /// Bytes per scanline (pitch)
    pitch: u32,
    /// Is the GPU initialized?
    initialized: bool,
    /// Hardware cursor enabled?
    hw_cursor_enabled: bool,
    /// Is MMIO verified working?
    mmio_verified: bool,
}

// =============================================================================
// MMIO Register Offsets (from RRG-G04500-C and r128_reg.h)
// =============================================================================

mod regs {
    // CRTC Registers
    pub const CRTC_H_TOTAL_DISP: u32 = 0x0200;
    pub const CRTC_H_SYNC_STRT_WID: u32 = 0x0204;
    pub const CRTC_V_TOTAL_DISP: u32 = 0x0208;
    pub const CRTC_V_SYNC_STRT_WID: u32 = 0x020C;
    pub const CRTC_OFFSET: u32 = 0x0224;
    pub const CRTC_OFFSET_CNTL: u32 = 0x0228;
    pub const CRTC_PITCH: u32 = 0x022C;
    pub const CRTC_GEN_CNTL: u32 = 0x0050;
    pub const CRTC_EXT_CNTL: u32 = 0x0054;

    // DAC Registers
    pub const DAC_CNTL: u32 = 0x0058;
    pub const DAC_MASK: u32 = 0x00B0;
    pub const DAC_R_INDEX: u32 = 0x00B4;
    pub const DAC_W_INDEX: u32 = 0x00B8;
    pub const DAC_DATA: u32 = 0x00BC;

    // PLL Registers (indirect access)
    pub const CLOCK_CNTL_INDEX: u32 = 0x0008;
    pub const CLOCK_CNTL_DATA: u32 = 0x000C;
    pub const PPLL_REF_DIV: u32 = 0x0003;
    pub const PPLL_DIV_0: u32 = 0x0004;
    pub const PPLL_DIV_1: u32 = 0x0005;
    pub const PPLL_DIV_2: u32 = 0x0006;
    pub const PPLL_DIV_3: u32 = 0x0007;
    pub const PPLL_CNTL: u32 = 0x0002;
    pub const VCLK_ECP_CNTL: u32 = 0x0008;

    // Memory Controller
    pub const MEM_CNTL: u32 = 0x0140;
    pub const MEM_ADDR_CONFIG: u32 = 0x0148;
    pub const MC_FB_LOCATION: u32 = 0x0148;
    pub const MC_AGP_LOCATION: u32 = 0x014C;

    // Bus Control
    pub const BUS_CNTL: u32 = 0x0030;
    pub const BUS_CNTL1: u32 = 0x0034;
    pub const AGP_BASE: u32 = 0x0170;
    pub const AGP_CNTL: u32 = 0x0174;

    // Config / Identification
    pub const CONFIG_CHIP_ID: u32 = 0x00E0;
    pub const CONFIG_MEMSIZE: u32 = 0x00F8;

    // 2D Engine Registers
    pub const DP_GUI_MASTER_CNTL: u32 = 0x146C;
    pub const DP_BRUSH_BKGD_CLR: u32 = 0x1478;
    pub const DP_BRUSH_FRGD_CLR: u32 = 0x147C;
    pub const DP_SRC_BKGD_CLR: u32 = 0x15DC;
    pub const DP_SRC_FRGD_CLR: u32 = 0x15D8;
    pub const DP_WRITE_MASK: u32 = 0x16CC;
    pub const DP_MIX: u32 = 0x16C8;
    pub const DP_DATATYPE: u32 = 0x16C4;
    pub const DP_CNTL: u32 = 0x16C0;

    pub const DST_OFFSET: u32 = 0x1404;
    pub const DST_PITCH: u32 = 0x1408;
    pub const DST_Y_X: u32 = 0x1438;
    pub const DST_HEIGHT_WIDTH: u32 = 0x143C;
    pub const DST_BRES_ERR: u32 = 0x1440;
    pub const DST_BRES_INC: u32 = 0x1444;
    pub const DST_BRES_DEC: u32 = 0x1448;

    pub const SRC_OFFSET: u32 = 0x15AC;
    pub const SRC_PITCH: u32 = 0x15B0;
    pub const SRC_Y_X: u32 = 0x1434;
    pub const SRC_HEIGHT_WIDTH: u32 = 0x15B4;

    // Hardware Cursor
    pub const CUR_OFFSET: u32 = 0x0260;
    pub const CUR_HORZ_VERT_POSN: u32 = 0x0264;
    pub const CUR_HORZ_VERT_OFF: u32 = 0x0268;
    pub const CUR_CLR0: u32 = 0x026C;
    pub const CUR_CLR1: u32 = 0x0270;

    // Engine Status
    pub const GUI_STAT: u32 = 0x1740;
    pub const FIFO_STAT: u32 = 0x1470;

    // Power Management
    pub const PM4_BUFFER_CNTL: u32 = 0x0704;
    pub const CLK_PIN_CNTL: u32 = 0x0001;
    pub const POWER_MANAGEMENT: u32 = 0x002F;
}

// =============================================================================
// CRTC_GEN_CNTL bits
// =============================================================================

mod crtc_gen_cntl {
    pub const CRTC_DBL_SCAN_EN: u32 = 1 << 0;
    pub const CRTC_INTERLACE_EN: u32 = 1 << 1;
    pub const CRTC_CSYNC_EN: u32 = 1 << 4;
    pub const CRTC_CUR_EN: u32 = 1 << 16;
    pub const CRTC_CUR_MODE_MASK: u32 = 7 << 17;
    pub const CRTC_EXT_DISP_EN: u32 = 1 << 24;
    pub const CRTC_EN: u32 = 1 << 25;
    pub const CRTC_DISP_REQ_EN_B: u32 = 1 << 26;

    // Pixel depth encoding (bits 8-11)
    pub const CRTC_PIX_WIDTH_MASK: u32 = 0x0F << 8;
    pub const CRTC_PIX_WIDTH_8BPP: u32 = 2 << 8;
    pub const CRTC_PIX_WIDTH_15BPP: u32 = 3 << 8;
    pub const CRTC_PIX_WIDTH_16BPP: u32 = 4 << 8;
    pub const CRTC_PIX_WIDTH_24BPP: u32 = 5 << 8;
    pub const CRTC_PIX_WIDTH_32BPP: u32 = 6 << 8;
}

// =============================================================================
// 2D Engine bits
// =============================================================================

mod dp_gui {
    // ROP3 operations
    pub const ROP3_PATCOPY: u32 = 0xF0;
    pub const ROP3_SRCCOPY: u32 = 0xCC;
    pub const ROP3_ZERO: u32 = 0x00;
    pub const ROP3_ONE: u32 = 0xFF;

    // GUI master control
    pub const GMC_DST_PITCH_OFFSET_CNTL: u32 = 1 << 1;
    pub const GMC_SRC_PITCH_OFFSET_CNTL: u32 = 1 << 0;
    pub const GMC_BRUSH_SOLID_COLOR: u32 = 13 << 4;
    pub const GMC_SRC_DATATYPE_COLOR: u32 = 3 << 12;
    pub const GMC_CLR_CMP_CNTL_DIS: u32 = 1 << 28;
    pub const GMC_WR_MSK_DIS: u32 = 1 << 30;
}

// =============================================================================
// Display Mode Timings
// =============================================================================

/// Standard display mode timing parameters
#[derive(Debug, Clone, Copy)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub pixel_clock: u32,  // kHz
    pub h_total: u32,
    pub h_sync_start: u32,
    pub h_sync_end: u32,
    pub v_total: u32,
    pub v_sync_start: u32,
    pub v_sync_end: u32,
    pub h_sync_polarity: bool,  // true = negative
    pub v_sync_polarity: bool,
}

impl DisplayMode {
    /// 800x600 @ 60Hz (VESA standard)
    pub const fn mode_800x600_60() -> Self {
        Self {
            width: 800,
            height: 600,
            refresh: 60,
            pixel_clock: 40000,
            h_total: 1056,
            h_sync_start: 840,
            h_sync_end: 968,
            v_total: 628,
            v_sync_start: 601,
            v_sync_end: 605,
            h_sync_polarity: false,
            v_sync_polarity: false,
        }
    }

    /// 1024x768 @ 60Hz (VESA standard)
    pub const fn mode_1024x768_60() -> Self {
        Self {
            width: 1024,
            height: 768,
            refresh: 60,
            pixel_clock: 65000,
            h_total: 1344,
            h_sync_start: 1048,
            h_sync_end: 1184,
            v_total: 806,
            v_sync_start: 771,
            v_sync_end: 777,
            h_sync_polarity: true,
            v_sync_polarity: true,
        }
    }

    /// 640x480 @ 60Hz (VGA standard)
    pub const fn mode_640x480_60() -> Self {
        Self {
            width: 640,
            height: 480,
            refresh: 60,
            pixel_clock: 25175,
            h_total: 800,
            h_sync_start: 656,
            h_sync_end: 752,
            v_total: 525,
            v_sync_start: 490,
            v_sync_end: 492,
            h_sync_polarity: true,
            v_sync_polarity: true,
        }
    }
}

// =============================================================================
// Implementation
// =============================================================================

impl AtiRage {
    /// Create a new ATI Rage driver instance
    pub const fn new() -> Self {
        Self {
            mmio_base: 0,
            fb_base: 0,
            fb_size: 0,
            width: 0,
            height: 0,
            bpp: 0,
            pitch: 0,
            initialized: false,
            hw_cursor_enabled: false,
            mmio_verified: false,
        }
    }

    /// Probe for ATI Rage Mobility P on PCI bus
    /// Returns (bus, device, function) if found
    pub fn probe() -> Option<(u8, u8, u8)> {
        // First check if PCI is working at all
        let test = unsafe { pci_config_read(0, 0, 0, 0) };
        if test == 0xFFFFFFFF {
            // No PCI bus or it's not responding
            return None;
        }

        // Scan PCI bus 0 and 1 (AGP is typically on bus 1)
        for bus in 0..2u8 {
            for device in 0..32u8 {
                let vendor_device = unsafe { pci_config_read(bus, device, 0, 0) };

                // 0xFFFFFFFF means no device present
                if vendor_device == 0xFFFFFFFF {
                    continue;
                }

                let vendor = (vendor_device & 0xFFFF) as u16;
                let device_id = ((vendor_device >> 16) & 0xFFFF) as u16;

                if vendor == ATI_VENDOR_ID && device_id == RAGE_MOBILITY_P_ID {
                    return Some((bus, device, 0));
                }
            }
        }
        None
    }

    /// Initialize the GPU
    pub fn init(&mut self, bus: u8, device: u8, func: u8) -> Result<(), &'static str> {
        // Read BARs from PCI config space
        let bar0 = unsafe { pci_config_read(bus, device, func, 0x10) };
        let bar2 = unsafe { pci_config_read(bus, device, func, 0x18) };

        // Check BAR type (bit 0: 0=memory, 1=I/O)
        if (bar0 & 0x01) != 0 {
            return Err("BAR0 is I/O space, expected memory");
        }
        if (bar2 & 0x01) != 0 {
            return Err("BAR2 is I/O space, expected memory");
        }

        // BAR0 = Framebuffer (memory mapped)
        self.fb_base = bar0 & 0xFFFFFFF0;

        // BAR2 = MMIO registers
        self.mmio_base = bar2 & 0xFFFFFFF0;

        // Validate addresses
        if self.fb_base == 0 {
            return Err("Framebuffer BAR is zero");
        }
        if self.mmio_base == 0 {
            return Err("MMIO BAR is zero");
        }

        // Check if addresses are in valid ranges
        // MMIO should be in the memory-mapped I/O region (typically above 0x80000000)
        if self.mmio_base < MIN_MMIO_ADDR || self.mmio_base > MAX_MMIO_ADDR {
            return Err("MMIO address out of expected range");
        }

        // Framebuffer can be lower, but should still be non-zero and reasonable
        if self.fb_base < 0x00100000 {  // Below 1MB is suspicious
            return Err("Framebuffer address suspiciously low");
        }

        // Enable bus mastering and memory space access
        let command = unsafe { pci_config_read(bus, device, func, 0x04) };
        unsafe {
            pci_config_write(bus, device, func, 0x04, command | 0x06);
        }

        // Verify MMIO is working by reading a known register
        if !self.verify_mmio() {
            return Err("MMIO verification failed - hardware not responding");
        }
        self.mmio_verified = true;

        // Now safe to do MMIO operations
        // Detect VRAM size
        self.fb_size = self.detect_vram_size();

        // Perform soft reset (only if MMIO verified)
        self.soft_reset();

        // Initialize memory controller
        self.init_memory_controller();

        self.initialized = true;
        Ok(())
    }

    /// Verify that MMIO is working by reading chip ID register
    fn verify_mmio(&self) -> bool {
        // Try to read CONFIG_CHIP_ID register
        // This should return a non-zero, non-0xFFFFFFFF value for a working chip
        let chip_id = self.mmio_read_safe(regs::CONFIG_CHIP_ID);

        match chip_id {
            Some(id) => {
                // Valid chip IDs for Rage Mobility are in specific ranges
                // Check that it's not obviously invalid
                if id == 0 || id == 0xFFFFFFFF {
                    return false;
                }
                // Could add more specific Rage chip ID validation here
                true
            }
            None => false,
        }
    }

    /// Safe MMIO read with basic validation (doesn't panic on failure)
    fn mmio_read_safe(&self, reg: u32) -> Option<u32> {
        if self.mmio_base == 0 {
            return None;
        }

        let addr = self.mmio_base.wrapping_add(reg);

        // Basic sanity check on address
        if addr < MIN_MMIO_ADDR || addr > MAX_MMIO_ADDR {
            return None;
        }

        // Do the read
        let value = unsafe {
            let ptr = addr as *const u32;
            ptr.read_volatile()
        };

        Some(value)
    }

    /// Detect VRAM size by probing
    fn detect_vram_size(&self) -> u32 {
        // Try reading CONFIG_MEMSIZE register if available
        if let Some(memsize) = self.mmio_read_safe(regs::CONFIG_MEMSIZE) {
            // This register reports memory in bytes on some chips
            if memsize > 0 && memsize <= 32 * 1024 * 1024 {
                return memsize;
            }
        }

        // Fall back to known value for Armada E500
        8 * 1024 * 1024
    }

    /// Soft reset the GPU
    fn soft_reset(&self) {
        if !self.mmio_verified {
            return;
        }

        // Save critical registers
        let crtc_gen_cntl = self.mmio_read(regs::CRTC_GEN_CNTL);
        let crtc_ext_cntl = self.mmio_read(regs::CRTC_EXT_CNTL);

        // Disable display
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen_cntl & !crtc_gen_cntl::CRTC_EN);

        // Reset the engine
        let bus_cntl = self.mmio_read(regs::BUS_CNTL);
        self.mmio_write(regs::BUS_CNTL, bus_cntl | (1 << 9));  // BUS_HOST_ERR_ACK

        // Wait for reset
        for _ in 0..1000 {
            unsafe { core::arch::asm!("nop"); }
        }

        // Clear reset bit
        self.mmio_write(regs::BUS_CNTL, bus_cntl);

        // Restore CRTC
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen_cntl);
        self.mmio_write(regs::CRTC_EXT_CNTL, crtc_ext_cntl);
    }

    /// Initialize memory controller
    fn init_memory_controller(&self) {
        if !self.mmio_verified {
            return;
        }

        // Set framebuffer location (at start of VRAM)
        let fb_location = (self.fb_base >> 16) | ((self.fb_base + self.fb_size - 1) & 0xFFFF0000);
        self.mmio_write(regs::MC_FB_LOCATION, fb_location);
    }

    /// Set display mode
    pub fn set_mode(&mut self, mode: &DisplayMode, bpp: u32) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("GPU not initialized");
        }

        if !self.mmio_verified {
            return Err("MMIO not verified");
        }

        // Disable CRTC during mode change
        let crtc_gen_cntl = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen_cntl & !crtc_gen_cntl::CRTC_EN);

        // Set pixel clock via PLL
        self.set_pixel_clock(mode.pixel_clock)?;

        // Program CRTC timing (values in character units = pixels/8 for horizontal)
        let h_total = (mode.h_total / 8) - 1;
        let h_disp = (mode.width / 8) - 1;
        let h_sync_start = mode.h_sync_start / 8;
        let h_sync_width = (mode.h_sync_end - mode.h_sync_start) / 8;

        let v_total = mode.v_total - 1;
        let v_disp = mode.height - 1;
        let v_sync_start = mode.v_sync_start;
        let v_sync_width = mode.v_sync_end - mode.v_sync_start;

        // H_TOTAL_DISP: bits 0-8 = h_total, bits 16-24 = h_disp
        self.mmio_write(regs::CRTC_H_TOTAL_DISP,
                        (h_total & 0x1FF) | ((h_disp & 0x1FF) << 16));

        // H_SYNC_STRT_WID: bits 0-10 = start (in pixels), bits 16-20 = width, bit 23 = polarity
        let h_sync_pol = if mode.h_sync_polarity { 1 << 23 } else { 0 };
        self.mmio_write(regs::CRTC_H_SYNC_STRT_WID,
                        (h_sync_start & 0x7FF) | ((h_sync_width & 0x1F) << 16) | h_sync_pol);

        // V_TOTAL_DISP
        self.mmio_write(regs::CRTC_V_TOTAL_DISP,
                        (v_total & 0xFFF) | ((v_disp & 0xFFF) << 16));

        // V_SYNC_STRT_WID
        let v_sync_pol = if mode.v_sync_polarity { 1 << 23 } else { 0 };
        self.mmio_write(regs::CRTC_V_SYNC_STRT_WID,
                        (v_sync_start & 0xFFF) | ((v_sync_width & 0x1F) << 16) | v_sync_pol);

        // Calculate pitch (must be aligned to 64 bytes for 2D engine)
        let bytes_per_pixel = bpp / 8;
        let pitch_bytes = ((mode.width * bytes_per_pixel + 63) / 64) * 64;
        let pitch_pixels = pitch_bytes / bytes_per_pixel;

        // CRTC_PITCH: in units of 8 pixels
        self.mmio_write(regs::CRTC_PITCH, pitch_pixels / 8);

        // Set framebuffer offset to 0
        self.mmio_write(regs::CRTC_OFFSET, 0);
        self.mmio_write(regs::CRTC_OFFSET_CNTL, 0);

        // Set pixel depth in CRTC_GEN_CNTL
        let pix_width = match bpp {
            8 => crtc_gen_cntl::CRTC_PIX_WIDTH_8BPP,
            15 => crtc_gen_cntl::CRTC_PIX_WIDTH_15BPP,
            16 => crtc_gen_cntl::CRTC_PIX_WIDTH_16BPP,
            24 => crtc_gen_cntl::CRTC_PIX_WIDTH_24BPP,
            32 => crtc_gen_cntl::CRTC_PIX_WIDTH_32BPP,
            _ => return Err("Unsupported bit depth"),
        };

        let new_crtc_gen = (crtc_gen_cntl & !crtc_gen_cntl::CRTC_PIX_WIDTH_MASK)
            | pix_width
            | crtc_gen_cntl::CRTC_EN
            | crtc_gen_cntl::CRTC_EXT_DISP_EN;
        self.mmio_write(regs::CRTC_GEN_CNTL, new_crtc_gen);

        // Update state
        self.width = mode.width;
        self.height = mode.height;
        self.bpp = bpp;
        self.pitch = pitch_bytes;

        // Initialize 2D engine for this mode
        self.init_2d_engine();

        Ok(())
    }

    /// Set pixel clock using PLL
    fn set_pixel_clock(&self, freq_khz: u32) -> Result<(), &'static str> {
        // Reference clock is typically 14.318 MHz on Rage chips
        const REF_CLK: u32 = 14318;

        // Calculate PLL dividers
        // VCLK = REF_CLK * feedback_div / (ref_div * post_div)

        // Use a simple approach: find dividers that get close to target
        let ref_div = 12u32;
        let post_div = 2u32;
        let feedback_div = (freq_khz * ref_div * post_div) / REF_CLK;

        // Program PLL (indirect register access)
        // Unlock PLL
        let vclk_ecp = self.pll_read(regs::VCLK_ECP_CNTL);
        self.pll_write(regs::VCLK_ECP_CNTL, vclk_ecp | (1 << 8));  // VCLK_SRC = PLL

        // Set reference divider
        self.pll_write(regs::PPLL_REF_DIV, ref_div);

        // Set feedback and post divider (using PPLL_DIV_0)
        self.pll_write(regs::PPLL_DIV_0, feedback_div | (post_div << 16));

        // Wait for PLL lock with timeout
        for _ in 0..10000 {
            let status = self.pll_read(regs::PPLL_CNTL);
            if status & (1 << 2) != 0 {
                return Ok(());
            }
            // Small delay
            for _ in 0..100 {
                unsafe { core::arch::asm!("nop"); }
            }
        }

        // PLL may still work even without lock indication
        Ok(())
    }

    /// Initialize 2D engine
    fn init_2d_engine(&self) {
        if !self.mmio_verified {
            return;
        }

        // Wait for engine idle
        self.wait_for_idle();

        // Avoid division by zero
        let pitch_pixels = if self.bpp > 0 {
            self.pitch / (self.bpp / 8)
        } else {
            self.pitch
        };

        // Set destination pitch and offset
        self.mmio_write(regs::DST_OFFSET, 0);
        self.mmio_write(regs::DST_PITCH, pitch_pixels);

        // Set source pitch and offset
        self.mmio_write(regs::SRC_OFFSET, 0);
        self.mmio_write(regs::SRC_PITCH, pitch_pixels);

        // Set default colors
        self.mmio_write(regs::DP_BRUSH_FRGD_CLR, 0xFFFFFF);
        self.mmio_write(regs::DP_BRUSH_BKGD_CLR, 0x000000);
        self.mmio_write(regs::DP_SRC_FRGD_CLR, 0xFFFFFF);
        self.mmio_write(regs::DP_SRC_BKGD_CLR, 0x000000);
        self.mmio_write(regs::DP_WRITE_MASK, 0xFFFFFFFF);

        // Set datatype based on bpp
        let datatype = match self.bpp {
            8 => 2,
            15 => 3,
            16 => 4,
            24 => 5,
            32 => 6,
            _ => 6,
        };
        self.mmio_write(regs::DP_DATATYPE, datatype << 0);

        // Enable left-to-right, top-to-bottom drawing
        self.mmio_write(regs::DP_CNTL, 0x03);  // DST_X_LEFT_TO_RIGHT | DST_Y_TOP_TO_BOTTOM
    }

    /// Wait for 2D engine to be idle
    pub fn wait_for_idle(&self) {
        if !self.mmio_verified {
            return;
        }

        // Poll GUI_STAT until engine is idle
        for _ in 0..1000000 {
            if let Some(stat) = self.mmio_read_safe(regs::GUI_STAT) {
                if (stat & 0x01) == 0 {
                    return;
                }
            } else {
                // MMIO failed, give up
                return;
            }
        }
        // Timeout - continue anyway
    }

    /// Wait for FIFO to have space
    fn wait_for_fifo(&self, entries: u32) {
        if !self.mmio_verified {
            return;
        }

        for _ in 0..1000000 {
            if let Some(stat) = self.mmio_read_safe(regs::FIFO_STAT) {
                let free = (stat >> 16) & 0x7F;
                if free >= entries {
                    return;
                }
            } else {
                return;
            }
        }
    }

    // =========================================================================
    // 2D Accelerated Operations
    // =========================================================================

    /// Fill a rectangle with a solid color
    pub fn fill_rect(&self, x: u32, y: u32, width: u32, height: u32, color: u32) {
        if !self.initialized || !self.mmio_verified {
            return;
        }

        self.wait_for_fifo(6);

        // Set up for solid fill
        let gmc = dp_gui::GMC_DST_PITCH_OFFSET_CNTL
            | dp_gui::GMC_BRUSH_SOLID_COLOR
            | dp_gui::GMC_CLR_CMP_CNTL_DIS
            | (dp_gui::ROP3_PATCOPY << 16)
            | (6 << 8);  // 32bpp

        self.mmio_write(regs::DP_GUI_MASTER_CNTL, gmc);
        self.mmio_write(regs::DP_BRUSH_FRGD_CLR, color);
        self.mmio_write(regs::DP_CNTL, 0x03);
        self.mmio_write(regs::DST_Y_X, (x << 16) | y);
        self.mmio_write(regs::DST_HEIGHT_WIDTH, (width << 16) | height);
    }

    /// Copy a rectangle (blit)
    pub fn copy_rect(&self, src_x: u32, src_y: u32, dst_x: u32, dst_y: u32, width: u32, height: u32) {
        if !self.initialized || !self.mmio_verified {
            return;
        }

        self.wait_for_fifo(8);

        // Determine direction based on overlap
        let direction = if dst_y > src_y || (dst_y == src_y && dst_x > src_x) {
            // Copy from bottom-right to top-left
            0x00
        } else {
            // Copy from top-left to bottom-right
            0x03
        };

        let (actual_src_x, actual_src_y, actual_dst_x, actual_dst_y) = if direction == 0x00 {
            (src_x + width - 1, src_y + height - 1, dst_x + width - 1, dst_y + height - 1)
        } else {
            (src_x, src_y, dst_x, dst_y)
        };

        let gmc = dp_gui::GMC_DST_PITCH_OFFSET_CNTL
            | dp_gui::GMC_SRC_PITCH_OFFSET_CNTL
            | dp_gui::GMC_SRC_DATATYPE_COLOR
            | dp_gui::GMC_CLR_CMP_CNTL_DIS
            | (dp_gui::ROP3_SRCCOPY << 16)
            | (6 << 8);  // 32bpp

        self.mmio_write(regs::DP_GUI_MASTER_CNTL, gmc);
        self.mmio_write(regs::DP_CNTL, direction);
        self.mmio_write(regs::SRC_Y_X, (actual_src_x << 16) | actual_src_y);
        self.mmio_write(regs::DST_Y_X, (actual_dst_x << 16) | actual_dst_y);
        self.mmio_write(regs::DST_HEIGHT_WIDTH, (width << 16) | height);
    }

    // =========================================================================
    // Hardware Cursor
    // =========================================================================

    /// Enable hardware cursor
    pub fn enable_hw_cursor(&mut self) {
        if !self.initialized || !self.mmio_verified {
            return;
        }

        let crtc_gen = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen | crtc_gen_cntl::CRTC_CUR_EN);

        // Set cursor colors (black and white)
        self.mmio_write(regs::CUR_CLR0, 0x00000000);  // Black
        self.mmio_write(regs::CUR_CLR1, 0x00FFFFFF);  // White

        self.hw_cursor_enabled = true;
    }

    /// Disable hardware cursor
    pub fn disable_hw_cursor(&mut self) {
        if !self.mmio_verified {
            return;
        }

        let crtc_gen = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen & !crtc_gen_cntl::CRTC_CUR_EN);
        self.hw_cursor_enabled = false;
    }

    /// Set hardware cursor position
    pub fn set_cursor_pos(&self, x: i32, y: i32) {
        if !self.hw_cursor_enabled || !self.mmio_verified {
            return;
        }

        let mut hot_x = 0u32;
        let mut hot_y = 0u32;
        let mut pos_x = x as u32;
        let mut pos_y = y as u32;

        // Handle negative coordinates (cursor partially off-screen)
        if x < 0 {
            hot_x = (-x) as u32;
            pos_x = 0;
        }
        if y < 0 {
            hot_y = (-y) as u32;
            pos_y = 0;
        }

        self.mmio_write(regs::CUR_HORZ_VERT_OFF, (hot_x << 16) | hot_y);
        self.mmio_write(regs::CUR_HORZ_VERT_POSN, (pos_x << 16) | pos_y);
    }

    /// Set hardware cursor image (64x64 2bpp bitmap)
    /// Image format: 2 bits per pixel, 00=transparent, 01=color0, 10=color1, 11=inverted
    pub fn set_cursor_image(&self, offset: u32, image: &[u8]) {
        if !self.initialized || !self.mmio_verified {
            return;
        }

        // Cursor image lives in VRAM
        // Set cursor offset register
        self.mmio_write(regs::CUR_OFFSET, offset >> 10);  // In 1KB units

        // Copy cursor image to VRAM at offset
        let cursor_ptr = (self.fb_base + offset) as *mut u8;
        for (i, &byte) in image.iter().enumerate().take(1024) {  // 64x64x2bpp = 1KB
            unsafe {
                cursor_ptr.add(i).write_volatile(byte);
            }
        }
    }

    // =========================================================================
    // Power Management
    // =========================================================================

    /// Put GPU into low power state
    pub fn enter_low_power(&self) {
        if !self.mmio_verified {
            return;
        }

        // Disable CRTC
        let crtc_gen = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen & !crtc_gen_cntl::CRTC_EN);

        // Gate clocks to unused blocks
        self.pll_write(regs::CLK_PIN_CNTL, 0x00);
    }

    /// Wake GPU from low power state
    pub fn exit_low_power(&self) {
        if !self.mmio_verified {
            return;
        }

        // Restore clock gating
        self.pll_write(regs::CLK_PIN_CNTL, 0x02);

        // Re-enable CRTC
        let crtc_gen = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen | crtc_gen_cntl::CRTC_EN);
    }

    // =========================================================================
    // Low-level Register Access
    // =========================================================================

    /// Read MMIO register
    #[inline]
    fn mmio_read(&self, reg: u32) -> u32 {
        unsafe {
            let ptr = (self.mmio_base + reg) as *const u32;
            ptr.read_volatile()
        }
    }

    /// Write MMIO register
    #[inline]
    fn mmio_write(&self, reg: u32, value: u32) {
        unsafe {
            let ptr = (self.mmio_base + reg) as *mut u32;
            ptr.write_volatile(value);
        }
    }

    /// Read PLL register (indirect access)
    fn pll_read(&self, reg: u32) -> u32 {
        self.mmio_write(regs::CLOCK_CNTL_INDEX, reg & 0x3F);
        self.mmio_read(regs::CLOCK_CNTL_DATA)
    }

    /// Write PLL register (indirect access)
    fn pll_write(&self, reg: u32, value: u32) {
        self.mmio_write(regs::CLOCK_CNTL_INDEX, (reg & 0x3F) | (1 << 7));  // Write enable
        self.mmio_write(regs::CLOCK_CNTL_DATA, value);
    }

    // =========================================================================
    // Getters
    // =========================================================================

    pub fn framebuffer_addr(&self) -> u32 { self.fb_base }
    pub fn framebuffer_size(&self) -> u32 { self.fb_size }
    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
    pub fn bpp(&self) -> u32 { self.bpp }
    pub fn pitch(&self) -> u32 { self.pitch }
    pub fn is_initialized(&self) -> bool { self.initialized }
    pub fn mmio_base(&self) -> u32 { self.mmio_base }
}

// =============================================================================
// PCI Configuration Space Access
// =============================================================================

/// Read from PCI configuration space
unsafe fn pci_config_read(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let address = 0x80000000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    outl(PCI_CONFIG_ADDR, address);
    inl(PCI_CONFIG_DATA)
}

/// Write to PCI configuration space
unsafe fn pci_config_write(bus: u8, device: u8, func: u8, offset: u8, value: u32) {
    let address = 0x80000000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    outl(PCI_CONFIG_ADDR, address);
    outl(PCI_CONFIG_DATA, value);
}

// =============================================================================
// Global Instance
// =============================================================================

/// Global ATI Rage GPU instance
pub static mut ATI_RAGE: AtiRage = AtiRage::new();

/// Initialize ATI Rage GPU driver
pub fn init() -> Result<(), &'static str> {
    // Probe for GPU
    let (bus, device, func) = AtiRage::probe()
        .ok_or("ATI Rage Mobility P not found on PCI bus")?;

    unsafe {
        ATI_RAGE.init(bus, device, func)?;
    }

    Ok(())
}

/// Get the global ATI Rage instance
pub fn get() -> Option<&'static mut AtiRage> {
    unsafe {
        if ATI_RAGE.is_initialized() {
            Some(&mut ATI_RAGE)
        } else {
            None
        }
    }
}
