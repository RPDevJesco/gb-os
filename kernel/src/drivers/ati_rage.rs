//! ATI Rage Mobility P GPU Driver - Armada E500/V300 Enhanced
//!
//! Native driver for the ATI Rage Mobility P AGP 2x (PCI ID 1002:4C4D)
//! found in the Compaq Armada E500 and V300 laptops.
//!
//! Hardware Details (from Armada E500 Technical Reference Guide):
//! - Armada E500: 8 MB video SDRAM
//! - Armada V300: 4 MB video SDRAM
//! - AGP 1X/2X with sideband addressing
//! - 64-bit memory interface, 125 MHz SDRAM
//! - LVDS transmitters for LCD (up to 1024x768)
//! - DDC2B for external CRT detection
//! - Hardware cursor: 64x64 pixels, 2 colors
//! - Built-in TV encoder (NTSC/PAL)
//!
//! Display Support:
//! - E500 LCD: 14.1" or 15.1" XGA TFT (1024x768) via LVDS
//! - V300 LCD: 12.1" SVGA STN/TFT (800x600)
//! - CRT: up to 1600x1200 @ 100Hz
//! - SimulScan: LCD + CRT simultaneously with different resolutions

use crate::arch::x86::io::{inb, outb, inl, outl};

// =============================================================================
// PCI Identification
// =============================================================================

/// ATI Vendor ID
pub const ATI_VENDOR_ID: u16 = 0x1002;

/// Rage Mobility P Device ID (LM = 4C4D)
pub const RAGE_MOBILITY_P_ID: u16 = 0x4C4D;

/// Compaq Armada E500 Subsystem ID (0E11 = Compaq vendor)
pub const ARMADA_E500_SUBSYS: u32 = 0xB1600E11;

/// Compaq Armada V300 Subsystem ID
pub const ARMADA_V300_SUBSYS: u32 = 0xB1000E11;

// =============================================================================
// Hardware Configuration from Tech Ref
// =============================================================================

/// VRAM sizes by model
pub mod vram_size {
    /// Armada E500: 8 MB SDRAM
    pub const E500: u32 = 8 * 1024 * 1024;
    /// Armada V300: 4 MB SDRAM
    pub const V300: u32 = 4 * 1024 * 1024;
    /// Minimum for any Mobility-P
    pub const MIN: u32 = 2 * 1024 * 1024;
    /// Maximum supported
    pub const MAX: u32 = 8 * 1024 * 1024;
}

/// LCD panel types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdPanelType {
    /// No LCD detected
    None,
    /// 800x600 STN (V300)
    Svga800x600Stn,
    /// 800x600 TFT (V300/E500)
    Svga800x600Tft,
    /// 1024x768 TFT (E500)
    Xga1024x768Tft,
}

impl LcdPanelType {
    /// Get native resolution for this panel
    pub fn native_resolution(&self) -> (u32, u32) {
        match self {
            Self::None => (0, 0),
            Self::Svga800x600Stn | Self::Svga800x600Tft => (800, 600),
            Self::Xga1024x768Tft => (1024, 768),
        }
    }

    /// Check if this is a TFT panel (better quality)
    pub fn is_tft(&self) -> bool {
        matches!(self, Self::Svga800x600Tft | Self::Xga1024x768Tft)
    }
}

/// Display output targets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayTarget {
    /// Internal LCD only
    Lcd,
    /// External CRT only
    Crt,
    /// TV output (NTSC/PAL)
    Tv,
    /// SimulScan: LCD + CRT simultaneously
    LcdAndCrt,
    /// LCD + TV simultaneously
    LcdAndTv,
}

/// TV output standard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvStandard {
    NtscM,   // Americas, Japan, etc.
    PalB,    // Most of Europe
    PalI,    // UK, Ireland
    PalM,    // Brazil
    PalN,    // Argentina, Paraguay
}

// =============================================================================
// Memory Map (from PCI BARs)
// =============================================================================

/// PCI Configuration Space ports
const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// Minimum valid MMIO base address
const MIN_MMIO_ADDR: u32 = 0x80000000;

/// Maximum valid MMIO base address
const MAX_MMIO_ADDR: u32 = 0xFFF00000;

/// PCI config register offsets
mod pci_reg {
    pub const VENDOR_DEVICE: u8 = 0x00;
    pub const COMMAND_STATUS: u8 = 0x04;
    pub const CLASS_REV: u8 = 0x08;
    pub const BAR0: u8 = 0x10;  // Framebuffer
    pub const BAR1: u8 = 0x14;  // I/O (unused)
    pub const BAR2: u8 = 0x18;  // MMIO registers
    pub const SUBSYSTEM: u8 = 0x2C;
    pub const AGP_CAP: u8 = 0x58;  // AGP capability
}

// =============================================================================
// MMIO Register Definitions
// =============================================================================

pub mod regs {
    // Clock/PLL
    pub const CLOCK_CNTL_INDEX: u32 = 0x0008;
    pub const CLOCK_CNTL_DATA: u32 = 0x000C;
    pub const CLK_PIN_CNTL: u32 = 0x0001;  // PLL register
    pub const MCLK_FB_DIV: u32 = 0x0004;   // PLL register
    pub const VCLK_ECP_CNTL: u32 = 0x0008; // PLL register

    // Memory controller
    pub const MEM_CNTL: u32 = 0x0140;
    pub const MC_FB_LOCATION: u32 = 0x0148;
    pub const CONFIG_MEMSIZE: u32 = 0x00F8;  // Reports VRAM size
    pub const CONFIG_CHIP_ID: u32 = 0x00E0;

    // CRTC
    pub const CRTC_GEN_CNTL: u32 = 0x0050;
    pub const CRTC_EXT_CNTL: u32 = 0x0054;
    pub const CRTC_H_TOTAL_DISP: u32 = 0x0200;
    pub const CRTC_H_SYNC_STRT_WID: u32 = 0x0204;
    pub const CRTC_V_TOTAL_DISP: u32 = 0x0208;
    pub const CRTC_V_SYNC_STRT_WID: u32 = 0x020C;
    pub const CRTC_OFFSET: u32 = 0x0224;
    pub const CRTC_PITCH: u32 = 0x022C;

    // Second CRTC (for SimulScan dual display)
    pub const CRTC2_GEN_CNTL: u32 = 0x03F8;
    pub const CRTC2_H_TOTAL_DISP: u32 = 0x0300;
    pub const CRTC2_V_TOTAL_DISP: u32 = 0x0308;

    // DAC
    pub const DAC_CNTL: u32 = 0x0058;
    pub const DAC_MASK: u32 = 0x00B0;
    pub const DAC_R_INDEX: u32 = 0x00B4;
    pub const DAC_W_INDEX: u32 = 0x00B8;
    pub const DAC_DATA: u32 = 0x00BC;

    // Hardware cursor
    pub const CUR_OFFSET: u32 = 0x0260;
    pub const CUR_HORZ_VERT_POSN: u32 = 0x0264;
    pub const CUR_HORZ_VERT_OFF: u32 = 0x0268;
    pub const CUR_CLR0: u32 = 0x026C;
    pub const CUR_CLR1: u32 = 0x0270;

    // Bus control
    pub const BUS_CNTL: u32 = 0x0030;
    pub const AGP_CNTL: u32 = 0x0174;

    // LCD/LVDS control
    pub const LVDS_GEN_CNTL: u32 = 0x02D0;
    pub const LVDS_PLL_CNTL: u32 = 0x02D4;
    pub const FP_GEN_CNTL: u32 = 0x0284;   // Flat panel general control
    pub const FP_HORZ_STRETCH: u32 = 0x028C;
    pub const FP_VERT_STRETCH: u32 = 0x0290;
    pub const FP_H_SYNC_STRT_WID: u32 = 0x02C4;
    pub const FP_V_SYNC_STRT_WID: u32 = 0x02C8;

    // TV encoder
    pub const TV_DAC_CNTL: u32 = 0x088C;
    pub const TV_MASTER_CNTL: u32 = 0x0800;

    // DDC/I2C for monitor detection
    pub const GPIO_MONID: u32 = 0x0068;
    pub const GPIO_DDC: u32 = 0x0060;

    // 2D engine
    pub const DST_OFFSET: u32 = 0x1404;
    pub const DST_PITCH: u32 = 0x1408;
    pub const DST_WIDTH: u32 = 0x140C;
    pub const DST_HEIGHT: u32 = 0x1410;
    pub const SRC_OFFSET: u32 = 0x1414;
    pub const SRC_PITCH: u32 = 0x1418;
    pub const SRC_X: u32 = 0x141C;
    pub const SRC_Y: u32 = 0x1420;
    pub const DST_X: u32 = 0x1424;
    pub const DST_Y: u32 = 0x1428;
    pub const DP_GUI_MASTER_CNTL: u32 = 0x146C;
    pub const DP_BRUSH_FRGD_CLR: u32 = 0x147C;
    pub const DP_BRUSH_BKGD_CLR: u32 = 0x1478;
    pub const DP_WRITE_MSK: u32 = 0x16CC;
    pub const GUI_STAT: u32 = 0x1740;
}

/// CRTC_GEN_CNTL bit definitions
pub mod crtc_gen_cntl {
    pub const CRTC_DBL_SCAN_EN: u32 = 1 << 0;
    pub const CRTC_INTERLACE_EN: u32 = 1 << 1;
    pub const CRTC_CSYNC_EN: u32 = 1 << 4;
    pub const CRTC_PIX_WIDTH_MASK: u32 = 0x7 << 8;
    pub const CRTC_PIX_WIDTH_8BPP: u32 = 2 << 8;
    pub const CRTC_PIX_WIDTH_15BPP: u32 = 3 << 8;
    pub const CRTC_PIX_WIDTH_16BPP: u32 = 4 << 8;
    pub const CRTC_PIX_WIDTH_24BPP: u32 = 5 << 8;
    pub const CRTC_PIX_WIDTH_32BPP: u32 = 6 << 8;
    pub const CRTC_EN: u32 = 1 << 25;
    pub const CRTC_DISP_REQ_EN_B: u32 = 1 << 26;
}

/// LVDS_GEN_CNTL bit definitions (for LCD panels)
pub mod lvds_gen_cntl {
    pub const LVDS_ON: u32 = 1 << 0;
    pub const LVDS_BLON: u32 = 1 << 19;  // Backlight on
    pub const LVDS_SEL_CRTC2: u32 = 1 << 23;
    pub const LVDS_EN: u32 = 1 << 7;
}

/// GUI_STAT bit definitions
pub mod gui_stat {
    pub const GUI_ACTIVE: u32 = 1 << 0;
}

// =============================================================================
// Display Mode Definitions
// =============================================================================

/// Display timing parameters
#[derive(Clone, Copy)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,  // Hz
    pub pixel_clock: u32,  // kHz
    pub h_total: u32,
    pub h_sync_start: u32,
    pub h_sync_end: u32,
    pub v_total: u32,
    pub v_sync_start: u32,
    pub v_sync_end: u32,
    pub h_sync_polarity: bool,  // true = positive
    pub v_sync_polarity: bool,
}

impl DisplayMode {
    /// 640x480 @ 60Hz (VGA standard)
    pub const fn mode_640x480_60() -> Self {
        Self {
            width: 640, height: 480, refresh: 60,
            pixel_clock: 25175,
            h_total: 800, h_sync_start: 656, h_sync_end: 752,
            v_total: 525, v_sync_start: 490, v_sync_end: 492,
            h_sync_polarity: false, v_sync_polarity: false,
        }
    }

    /// 800x600 @ 60Hz (SVGA - native for V300)
    pub const fn mode_800x600_60() -> Self {
        Self {
            width: 800, height: 600, refresh: 60,
            pixel_clock: 40000,
            h_total: 1056, h_sync_start: 840, h_sync_end: 968,
            v_total: 628, v_sync_start: 601, v_sync_end: 605,
            h_sync_polarity: true, v_sync_polarity: true,
        }
    }

    /// 1024x768 @ 60Hz (XGA - native for E500)
    pub const fn mode_1024x768_60() -> Self {
        Self {
            width: 1024, height: 768, refresh: 60,
            pixel_clock: 65000,
            h_total: 1344, h_sync_start: 1048, h_sync_end: 1184,
            v_total: 806, v_sync_start: 771, v_sync_end: 777,
            h_sync_polarity: false, v_sync_polarity: false,
        }
    }

    /// 1280x1024 @ 60Hz (CRT only - exceeds LCD capability)
    pub const fn mode_1280x1024_60() -> Self {
        Self {
            width: 1280, height: 1024, refresh: 60,
            pixel_clock: 108000,
            h_total: 1688, h_sync_start: 1328, h_sync_end: 1440,
            v_total: 1066, v_sync_start: 1025, v_sync_end: 1028,
            h_sync_polarity: true, v_sync_polarity: true,
        }
    }

    /// Calculate minimum VRAM required for this mode
    pub fn min_vram_required(&self, bpp: u32) -> u32 {
        let bytes_per_pixel = bpp / 8;
        self.width * self.height * bytes_per_pixel
    }
}

// =============================================================================
// GPU State
// =============================================================================

/// GPU state
pub struct AtiRage {
    // PCI location
    pci_bus: u8,
    pci_dev: u8,
    pci_func: u8,

    // Memory regions
    mmio_base: u32,
    fb_base: u32,
    fb_size: u32,

    // Current display state
    width: u32,
    height: u32,
    bpp: u32,
    pitch: u32,

    // Hardware info
    chip_id: u32,
    subsystem_id: u32,
    lcd_panel: LcdPanelType,
    agp_mode: u8,  // 0 = PCI only, 1 = 1X, 2 = 2X

    // Status flags
    initialized: bool,
    mmio_verified: bool,
    hw_cursor_enabled: bool,
    crt_connected: bool,
    lcd_active: bool,
}

impl AtiRage {
    /// Create a new ATI Rage driver instance
    pub const fn new() -> Self {
        Self {
            pci_bus: 0,
            pci_dev: 0,
            pci_func: 0,
            mmio_base: 0,
            fb_base: 0,
            fb_size: 0,
            width: 0,
            height: 0,
            bpp: 0,
            pitch: 0,
            chip_id: 0,
            subsystem_id: 0,
            lcd_panel: LcdPanelType::None,
            agp_mode: 0,
            initialized: false,
            mmio_verified: false,
            hw_cursor_enabled: false,
            crt_connected: false,
            lcd_active: false,
        }
    }

    /// Probe for ATI Rage Mobility P on PCI bus
    pub fn probe() -> Option<(u8, u8, u8)> {
        // Check PCI bus is working
        let test = unsafe { pci_config_read(0, 0, 0, 0) };
        if test == 0xFFFFFFFF {
            return None;
        }

        // Scan PCI bus 0 and 1 (AGP is typically on bus 1)
        for bus in 0..2u8 {
            for device in 0..32u8 {
                let vendor_device = unsafe {
                    pci_config_read(bus, device, 0, pci_reg::VENDOR_DEVICE)
                };

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
        self.pci_bus = bus;
        self.pci_dev = device;
        self.pci_func = func;

        // Read subsystem ID to identify E500 vs V300
        self.subsystem_id = unsafe {
            pci_config_read(bus, device, func, pci_reg::SUBSYSTEM)
        };

        // Read BARs
        let bar0 = unsafe { pci_config_read(bus, device, func, pci_reg::BAR0) };
        let bar2 = unsafe { pci_config_read(bus, device, func, pci_reg::BAR2) };

        // Validate BAR types
        if (bar0 & 0x01) != 0 {
            return Err("BAR0 is I/O space, expected memory");
        }
        if (bar2 & 0x01) != 0 {
            return Err("BAR2 is I/O space, expected memory");
        }

        self.fb_base = bar0 & 0xFFFFFFF0;
        self.mmio_base = bar2 & 0xFFFFFFF0;

        // Validate addresses
        if self.fb_base == 0 {
            return Err("Framebuffer BAR is zero");
        }
        if self.mmio_base == 0 || self.mmio_base < MIN_MMIO_ADDR || self.mmio_base > MAX_MMIO_ADDR {
            return Err("MMIO address invalid");
        }

        // Enable bus mastering and memory space
        let command = unsafe { pci_config_read(bus, device, func, pci_reg::COMMAND_STATUS) };
        unsafe {
            pci_config_write(bus, device, func, pci_reg::COMMAND_STATUS, command | 0x06);
        }

        // Verify MMIO is working
        if !self.verify_mmio() {
            return Err("MMIO verification failed");
        }
        self.mmio_verified = true;

        // Read chip ID
        self.chip_id = self.mmio_read(regs::CONFIG_CHIP_ID);

        // Detect VRAM size (model-aware)
        self.fb_size = self.detect_vram_size();

        // Detect LCD panel type
        self.lcd_panel = self.detect_lcd_panel();

        // Detect AGP capability
        self.agp_mode = self.detect_agp_mode();

        // Check for external CRT via DDC
        self.crt_connected = self.detect_crt_via_ddc();

        // Initialize memory controller
        self.init_memory_controller();

        self.initialized = true;
        Ok(())
    }

    /// Detect VRAM size with model awareness
    fn detect_vram_size(&self) -> u32 {
        // First try CONFIG_MEMSIZE register
        let memsize = self.mmio_read(regs::CONFIG_MEMSIZE);
        if memsize > 0 && memsize <= vram_size::MAX {
            return memsize;
        }

        // Fall back to model-based detection using subsystem ID
        match self.subsystem_id {
            ARMADA_E500_SUBSYS => vram_size::E500,  // 8 MB
            ARMADA_V300_SUBSYS => vram_size::V300,  // 4 MB
            _ => {
                // Unknown model - probe by writing/reading test pattern
                self.probe_vram_size()
            }
        }
    }

    /// Probe VRAM by writing test patterns
    fn probe_vram_size(&self) -> u32 {
        let test_pattern: u32 = 0xDEADBEEF;
        let anti_pattern: u32 = 0x12345678;

        // Test at various size boundaries
        for &size in &[8 * 1024 * 1024u32, 4 * 1024 * 1024, 2 * 1024 * 1024] {
            let test_offset = size - 4;
            let test_addr = self.fb_base + test_offset;

            unsafe {
                // Write test pattern at end of potential VRAM
                let ptr = test_addr as *mut u32;
                ptr.write_volatile(test_pattern);

                // Write different pattern at start to detect aliasing
                let start_ptr = self.fb_base as *mut u32;
                start_ptr.write_volatile(anti_pattern);

                // Read back - if aliased, start write will have overwritten it
                let readback = ptr.read_volatile();
                if readback == test_pattern {
                    return size;
                }
            }
        }

        // Minimum fallback
        vram_size::MIN
    }

    /// Detect LCD panel type from LVDS/FP registers
    fn detect_lcd_panel(&self) -> LcdPanelType {
        let lvds = self.mmio_read(regs::LVDS_GEN_CNTL);
        let fp = self.mmio_read(regs::FP_GEN_CNTL);

        // Check if LVDS is present/enabled
        if (lvds & lvds_gen_cntl::LVDS_EN) == 0 {
            return LcdPanelType::None;
        }

        // Read panel dimensions from FP stretch registers
        let h_stretch = self.mmio_read(regs::FP_HORZ_STRETCH);
        let native_width = ((h_stretch >> 16) & 0x7FF) + 1;

        // Classify based on width
        match native_width {
            1024 => LcdPanelType::Xga1024x768Tft,
            800 => {
                // Could be STN or TFT - check FP control bits
                // TFT panels typically have different timing requirements
                if (fp & 0x100) != 0 {
                    LcdPanelType::Svga800x600Tft
                } else {
                    LcdPanelType::Svga800x600Stn
                }
            }
            _ => LcdPanelType::None,
        }
    }

    /// Detect AGP mode capability
    fn detect_agp_mode(&self) -> u8 {
        let agp_cntl = self.mmio_read(regs::AGP_CNTL);

        // Check if AGP is enabled
        if (agp_cntl & 0x01) == 0 {
            return 0;  // PCI mode
        }

        // Check for 2X mode
        if (agp_cntl & 0x04) != 0 {
            2
        } else {
            1
        }
    }

    /// Detect external CRT via DDC/I2C
    fn detect_crt_via_ddc(&self) -> bool {
        // Read GPIO_MONID to check for monitor presence
        let monid = self.mmio_read(regs::GPIO_MONID);

        // Bit 8 typically indicates monitor connected
        (monid & 0x100) != 0
    }

    /// Verify MMIO is working
    fn verify_mmio(&self) -> bool {
        if let Some(id) = self.mmio_read_safe(regs::CONFIG_CHIP_ID) {
            id != 0 && id != 0xFFFFFFFF
        } else {
            false
        }
    }

    /// Safe MMIO read
    fn mmio_read_safe(&self, reg: u32) -> Option<u32> {
        if self.mmio_base == 0 {
            return None;
        }
        let addr = self.mmio_base.wrapping_add(reg);
        if addr < MIN_MMIO_ADDR || addr > MAX_MMIO_ADDR {
            return None;
        }
        Some(unsafe { (addr as *const u32).read_volatile() })
    }

    /// Initialize memory controller
    fn init_memory_controller(&self) {
        if !self.mmio_verified {
            return;
        }
        let fb_location = (self.fb_base >> 16) |
            ((self.fb_base + self.fb_size - 1) & 0xFFFF0000);
        self.mmio_write(regs::MC_FB_LOCATION, fb_location);
    }

    /// Set display mode
    pub fn set_mode(&mut self, mode: &DisplayMode, bpp: u32) -> Result<(), &'static str> {
        if !self.initialized || !self.mmio_verified {
            return Err("GPU not initialized");
        }

        // Check VRAM is sufficient
        let required = mode.min_vram_required(bpp);
        if required > self.fb_size {
            return Err("Insufficient VRAM for this mode");
        }

        // Disable CRTC during mode change
        let crtc_gen = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen & !crtc_gen_cntl::CRTC_EN);

        // Program CRTC timing
        self.program_crtc_timing(mode, bpp)?;

        // Set pixel format
        self.set_pixel_format(bpp)?;

        // Re-enable CRTC
        let crtc_gen = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen | crtc_gen_cntl::CRTC_EN);

        // Update state
        self.width = mode.width;
        self.height = mode.height;
        self.bpp = bpp;
        self.pitch = mode.width * (bpp / 8);

        // Enable LCD if this mode fits
        if mode.width <= self.lcd_panel.native_resolution().0 &&
            mode.height <= self.lcd_panel.native_resolution().1 {
            self.enable_lcd();
            self.lcd_active = true;
        }

        Ok(())
    }

    /// Program CRTC timing registers
    fn program_crtc_timing(&self, mode: &DisplayMode, _bpp: u32) -> Result<(), &'static str> {
        let h_total = (mode.h_total / 8) - 1;
        let h_disp = (mode.width / 8) - 1;
        let h_sync_start = mode.h_sync_start / 8;
        let h_sync_width = (mode.h_sync_end - mode.h_sync_start) / 8;

        let v_total = mode.v_total - 1;
        let v_disp = mode.height - 1;
        let v_sync_start = mode.v_sync_start;
        let v_sync_width = mode.v_sync_end - mode.v_sync_start;

        // H_TOTAL_DISP
        self.mmio_write(regs::CRTC_H_TOTAL_DISP,
                        (h_total & 0x1FF) | ((h_disp & 0x1FF) << 16));

        // H_SYNC_STRT_WID
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

        // Set pitch
        self.mmio_write(regs::CRTC_PITCH, mode.width / 8);

        Ok(())
    }

    /// Set pixel format
    fn set_pixel_format(&self, bpp: u32) -> Result<(), &'static str> {
        let mut crtc = self.mmio_read(regs::CRTC_GEN_CNTL);
        crtc &= !crtc_gen_cntl::CRTC_PIX_WIDTH_MASK;

        crtc |= match bpp {
            8 => crtc_gen_cntl::CRTC_PIX_WIDTH_8BPP,
            15 => crtc_gen_cntl::CRTC_PIX_WIDTH_15BPP,
            16 => crtc_gen_cntl::CRTC_PIX_WIDTH_16BPP,
            24 => crtc_gen_cntl::CRTC_PIX_WIDTH_24BPP,
            32 => crtc_gen_cntl::CRTC_PIX_WIDTH_32BPP,
            _ => return Err("Unsupported BPP"),
        };

        self.mmio_write(regs::CRTC_GEN_CNTL, crtc);
        Ok(())
    }

    /// Enable LCD panel via LVDS
    fn enable_lcd(&self) {
        let lvds = self.mmio_read(regs::LVDS_GEN_CNTL);
        self.mmio_write(regs::LVDS_GEN_CNTL,
                        lvds | lvds_gen_cntl::LVDS_ON | lvds_gen_cntl::LVDS_BLON);
    }

    /// Disable LCD panel
    pub fn disable_lcd(&self) {
        let lvds = self.mmio_read(regs::LVDS_GEN_CNTL);
        self.mmio_write(regs::LVDS_GEN_CNTL,
                        lvds & !(lvds_gen_cntl::LVDS_ON | lvds_gen_cntl::LVDS_BLON));
    }

    // =========================================================================
    // Hardware Cursor (64x64 pixels, 2 colors as per Tech Ref)
    // =========================================================================

    /// Enable hardware cursor
    pub fn enable_cursor(&mut self) {
        if !self.mmio_verified {
            return;
        }
        let crtc_gen = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen | (1 << 16));
        self.hw_cursor_enabled = true;
    }

    /// Disable hardware cursor
    pub fn disable_cursor(&mut self) {
        if !self.mmio_verified {
            return;
        }
        let crtc_gen = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc_gen & !(1 << 16));
        self.hw_cursor_enabled = false;
    }

    /// Set cursor position
    pub fn set_cursor_pos(&self, x: u16, y: u16) {
        if !self.hw_cursor_enabled {
            return;
        }
        self.mmio_write(regs::CUR_HORZ_VERT_POSN,
                        (x as u32) | ((y as u32) << 16));
    }

    /// Set cursor colors
    pub fn set_cursor_colors(&self, color0: u32, color1: u32) {
        self.mmio_write(regs::CUR_CLR0, color0);
        self.mmio_write(regs::CUR_CLR1, color1);
    }

    // =========================================================================
    // 2D Acceleration
    // =========================================================================

    /// Wait for 2D engine to become idle
    pub fn wait_for_idle(&self) {
        if !self.mmio_verified {
            return;
        }
        for _ in 0..100000 {
            let stat = self.mmio_read(regs::GUI_STAT);
            if (stat & gui_stat::GUI_ACTIVE) == 0 {
                return;
            }
        }
    }

    /// Fill rectangle (hardware accelerated)
    pub fn fill_rect(&self, x: u32, y: u32, width: u32, height: u32, color: u32) {
        if !self.mmio_verified {
            return;
        }

        self.wait_for_idle();

        // Setup destination
        self.mmio_write(regs::DST_OFFSET, 0);
        self.mmio_write(regs::DST_PITCH, self.pitch / (self.bpp / 8));

        // Set color
        self.mmio_write(regs::DP_BRUSH_FRGD_CLR, color);

        // Setup draw operation: solid fill
        self.mmio_write(regs::DP_GUI_MASTER_CNTL, 0x00000003);

        // Execute
        self.mmio_write(regs::DST_X, x);
        self.mmio_write(regs::DST_Y, y);
        self.mmio_write(regs::DST_WIDTH, width);
        self.mmio_write(regs::DST_HEIGHT, height);
    }

    // =========================================================================
    // Power Management
    // =========================================================================

    /// Enter low power state (disable unused blocks per Tech Ref)
    pub fn enter_low_power(&self) {
        if !self.mmio_verified {
            return;
        }

        // Disable CRTC
        let crtc = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc & !crtc_gen_cntl::CRTC_EN);

        // Disable LCD backlight
        let lvds = self.mmio_read(regs::LVDS_GEN_CNTL);
        self.mmio_write(regs::LVDS_GEN_CNTL, lvds & !lvds_gen_cntl::LVDS_BLON);
    }

    /// Exit low power state
    pub fn exit_low_power(&self) {
        if !self.mmio_verified {
            return;
        }

        // Re-enable backlight
        let lvds = self.mmio_read(regs::LVDS_GEN_CNTL);
        self.mmio_write(regs::LVDS_GEN_CNTL, lvds | lvds_gen_cntl::LVDS_BLON);

        // Re-enable CRTC
        let crtc = self.mmio_read(regs::CRTC_GEN_CNTL);
        self.mmio_write(regs::CRTC_GEN_CNTL, crtc | crtc_gen_cntl::CRTC_EN);
    }

    // =========================================================================
    // Low-level Register Access
    // =========================================================================

    #[inline]
    fn mmio_read(&self, reg: u32) -> u32 {
        unsafe {
            let ptr = (self.mmio_base + reg) as *const u32;
            ptr.read_volatile()
        }
    }

    #[inline]
    fn mmio_write(&self, reg: u32, value: u32) {
        unsafe {
            let ptr = (self.mmio_base + reg) as *mut u32;
            ptr.write_volatile(value);
        }
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
    pub fn lcd_panel_type(&self) -> LcdPanelType { self.lcd_panel }
    pub fn agp_mode(&self) -> u8 { self.agp_mode }
    pub fn is_crt_connected(&self) -> bool { self.crt_connected }
    pub fn is_lcd_active(&self) -> bool { self.lcd_active }
    pub fn chip_id(&self) -> u32 { self.chip_id }

    /// Get model name based on subsystem ID
    pub fn model_name(&self) -> &'static str {
        match self.subsystem_id {
            ARMADA_E500_SUBSYS => "Compaq Armada E500",
            ARMADA_V300_SUBSYS => "Compaq Armada V300",
            _ => "Unknown Armada",
        }
    }
}

// =============================================================================
// PCI Configuration Space Access
// =============================================================================

unsafe fn pci_config_read(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let address = 0x80000000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    outl(PCI_CONFIG_ADDR, address);
    inl(PCI_CONFIG_DATA)
}

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
