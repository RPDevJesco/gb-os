//! DPI (Display Parallel Interface) Driver for RetroFlag GPi Case 2W
//!
//! The GPi Case 2W uses a 3.0" IPS display connected via the DPI interface
//! on GPIO bank 0. This driver configures the GPIO pins and display timing
//! for the integrated LCD panel.
//!
//! # Display Specifications (GPi Case 2W)
//!
//! | Parameter       | Value          |
//! |-----------------|----------------|
//! | Panel Size      | 3.0" IPS       |
//! | Resolution      | 320x240        |
//! | Color Depth     | 18-bit (BGR666)|
//! | Interface       | DPI (Parallel) |
//! | Rotation        | 90° (portrait) |
//!
//! # DPI Pin Assignment (ALT2 Function)
//!
//! ```text
//! GPIO  | Function      | Description
//! ──────┼───────────────┼─────────────────
//!  0    | PCLK          | Pixel clock
//!  1    | DE            | Data enable
//!  2    | VSYNC         | Vertical sync
//!  3    | HSYNC         | Horizontal sync
//!  4-9  | B[2:7]        | Blue (6 bits)
//! 10-15 | G[2:7]        | Green (6 bits)
//! 16-21 | R[2:7]        | Red (6 bits)
//! ```
//!
//! Note: The GPi Case uses 18-bit color (BGR666) with padding,
//! so only GPIOs 0-21 are needed (not full 24-bit).
//!
//! # Usage
//!
//! ```rust,ignore
//! use dpi::{DpiDisplay, GpiCase2WConfig};
//!
//! // Initialize DPI for GPi Case 2W
//! let config = GpiCase2WConfig::default();
//! let mut display = DpiDisplay::new();
//! display.init(&config)?;
//!
//! // Framebuffer is now available via mailbox
//! ```

use crate::mmio;
use crate::memory_map::PERIPHERAL_BASE;

// ============================================================================
// GPIO Registers for DPI Configuration
// ============================================================================

/// GPIO base address
const GPIO_BASE: usize = PERIPHERAL_BASE + 0x0020_0000;

/// GPIO function select registers
mod gpio_regs {
    use super::GPIO_BASE;

    /// Function select register 0 (GPIO 0-9)
    pub const GPFSEL0: usize = GPIO_BASE + 0x00;
    /// Function select register 1 (GPIO 10-19)
    pub const GPFSEL1: usize = GPIO_BASE + 0x04;
    /// Function select register 2 (GPIO 20-29)
    pub const GPFSEL2: usize = GPIO_BASE + 0x08;

    /// Pull-up/down enable
    pub const GPPUD: usize = GPIO_BASE + 0x94;
    /// Pull-up/down clock 0
    pub const GPPUDCLK0: usize = GPIO_BASE + 0x98;
}

/// GPIO function codes
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum GpioFunction {
    Input = 0b000,
    Output = 0b001,
    Alt0 = 0b100,
    Alt1 = 0b101,
    Alt2 = 0b110,  // DPI function
    Alt3 = 0b111,
    Alt4 = 0b011,
    Alt5 = 0b010,
}

// ============================================================================
// DPI Output Format Register
// ============================================================================

/// DPI output format bits (for dpi_output_format / register configuration)
///
/// Format: 0xRRRRRRRR where bits control various output parameters
///
/// Bits [3:0]: Output format
///   1 = RGB565
///   2 = RGB565 (mode 2)
///   3 = RGB565 (mode 3)
///   4 = RGB666
///   5 = RGB666 (mode 5)
///   6 = RGB666 padded (mode 6) - GPi Case uses this
///   7 = RGB888
///
/// Bit 4: Output enable mode
/// Bits [7:5]: Reserved
/// Bits [11:8]: RGB order
/// Bits [15:12]: Output enable polarity
/// Bits [19:16]: Pixel clock edge
/// Bits [23:20]: Reserved
/// Bits [31:24]: Reserved
pub mod dpi_format {
    // Output format modes
    pub const FORMAT_RGB565: u32 = 1;
    pub const FORMAT_RGB666: u32 = 4;
    pub const FORMAT_RGB666_PADDED: u32 = 6;
    pub const FORMAT_RGB888: u32 = 7;

    // RGB order flags (bits 8-11)
    pub const RGB_ORDER_RGB: u32 = 0 << 8;
    pub const RGB_ORDER_BGR: u32 = 1 << 8;

    // Output enable flags (bit 4)
    pub const OUTPUT_ENABLE_MODE: u32 = 1 << 4;

    // DE polarity (bits 12-15)
    pub const DE_ACTIVE_HIGH: u32 = 0 << 12;
    pub const DE_ACTIVE_LOW: u32 = 1 << 12;

    // Pixel clock edge (bits 16-19)
    pub const PCLK_RISING: u32 = 0 << 16;
    pub const PCLK_FALLING: u32 = 1 << 16;

    // HSYNC polarity
    pub const HSYNC_ACTIVE_HIGH: u32 = 0 << 20;
    pub const HSYNC_ACTIVE_LOW: u32 = 1 << 20;

    // VSYNC polarity
    pub const VSYNC_ACTIVE_HIGH: u32 = 0 << 24;
    pub const VSYNC_ACTIVE_LOW: u32 = 1 << 24;

    /// GPi Case 2W format: 0x6016
    /// - Mode 6 (RGB666 padded)
    /// - Output enable mode
    /// - BGR order (bit 12)
    pub const GPI_CASE_FORMAT: u32 = 0x6016;
}

// ============================================================================
// Display Timing Configuration
// ============================================================================

/// DPI display timing parameters
#[derive(Debug, Clone, Copy)]
pub struct DpiTiming {
    /// Horizontal active pixels
    pub h_active: u32,
    /// Horizontal front porch
    pub h_front_porch: u32,
    /// Horizontal sync width
    pub h_sync: u32,
    /// Horizontal back porch
    pub h_back_porch: u32,
    /// Vertical active lines
    pub v_active: u32,
    /// Vertical front porch
    pub v_front_porch: u32,
    /// Vertical sync width
    pub v_sync: u32,
    /// Vertical back porch
    pub v_back_porch: u32,
    /// Pixel clock in Hz
    pub pixel_clock: u32,
    /// HSYNC polarity (true = negative)
    pub h_sync_neg: bool,
    /// VSYNC polarity (true = negative)
    pub v_sync_neg: bool,
}

impl DpiTiming {
    /// Calculate total horizontal pixels per line
    pub fn h_total(&self) -> u32 {
        self.h_active + self.h_front_porch + self.h_sync + self.h_back_porch
    }

    /// Calculate total vertical lines per frame
    pub fn v_total(&self) -> u32 {
        self.v_active + self.v_front_porch + self.v_sync + self.v_back_porch
    }

    /// Calculate frame rate in Hz
    pub fn frame_rate(&self) -> u32 {
        self.pixel_clock / (self.h_total() * self.v_total())
    }
}

/// GPi Case 2W display timing
///
/// From RetroFlag config.txt:
/// `hdmi_timings=240 1 38 10 20 320 1 20 4 4 0 0 0 60 0 6400000 1`
///
/// Format: h_active h_sync_polarity h_fp h_sync h_bp v_active v_sync_polarity v_fp v_sync v_bp ...
pub const GPI_CASE_2W_TIMING: DpiTiming = DpiTiming {
    // Horizontal (note: display is rotated, so 240 is the "width" in portrait)
    h_active: 240,
    h_front_porch: 38,
    h_sync: 10,
    h_back_porch: 20,
    // Vertical
    v_active: 320,
    v_front_porch: 20,
    v_sync: 4,
    v_back_porch: 4,
    // Clock
    pixel_clock: 6_400_000,  // 6.4 MHz
    // Polarity
    h_sync_neg: true,
    v_sync_neg: true,
};

/// Alternative timing for GPi Case (original)
/// Some variations exist between GPi Case versions
pub const GPI_CASE_TIMING_ALT: DpiTiming = DpiTiming {
    h_active: 320,
    h_front_porch: 28,
    h_sync: 18,
    h_back_porch: 28,
    v_active: 480,  // Some GPi variants report different resolution
    v_front_porch: 2,
    v_sync: 2,
    v_back_porch: 4,
    pixel_clock: 32_000_000,
    h_sync_neg: false,
    v_sync_neg: false,
};

// ============================================================================
// DPI Display Configuration
// ============================================================================

/// Complete DPI display configuration
#[derive(Debug, Clone)]
pub struct DpiConfig {
    /// Display timing parameters
    pub timing: DpiTiming,
    /// Output format (dpi_output_format value)
    pub output_format: u32,
    /// Framebuffer width (may differ from h_active if rotated)
    pub fb_width: u32,
    /// Framebuffer height
    pub fb_height: u32,
    /// Color depth in bits
    pub depth: u32,
    /// Display rotation in degrees (0, 90, 180, 270)
    pub rotation: u32,
}

impl DpiConfig {
    /// Default configuration for GPi Case 2W
    pub const fn gpi_case_2w() -> Self {
        Self {
            timing: GPI_CASE_2W_TIMING,
            output_format: dpi_format::GPI_CASE_FORMAT,
            fb_width: 320,   // After rotation
            fb_height: 240,
            depth: 32,       // Use 32bpp for easy manipulation
            rotation: 90,    // Rotated 90 degrees
        }
    }

    /// Configuration for 320x240 without rotation (testing)
    pub const fn test_320x240() -> Self {
        Self {
            timing: DpiTiming {
                h_active: 320,
                h_front_porch: 20,
                h_sync: 10,
                h_back_porch: 20,
                v_active: 240,
                v_front_porch: 10,
                v_sync: 4,
                v_back_porch: 10,
                pixel_clock: 8_000_000,
                h_sync_neg: false,
                v_sync_neg: false,
            },
            output_format: dpi_format::FORMAT_RGB666_PADDED
                | dpi_format::OUTPUT_ENABLE_MODE,
            fb_width: 320,
            fb_height: 240,
            depth: 32,
            rotation: 0,
        }
    }
}

// ============================================================================
// DPI Display Driver
// ============================================================================

/// DPI display driver error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpiError {
    /// GPIO configuration failed
    GpioConfigFailed,
    /// Framebuffer allocation failed
    FramebufferAllocFailed,
    /// Invalid configuration
    InvalidConfig,
    /// Mailbox communication failed
    MailboxFailed,
}

/// DPI display driver
pub struct DpiDisplay {
    /// Current configuration
    config: Option<DpiConfig>,
    /// Framebuffer address
    fb_addr: u32,
    /// Framebuffer size
    fb_size: u32,
    /// Pitch (bytes per row)
    pitch: u32,
    /// Initialized flag
    initialized: bool,
}

impl DpiDisplay {
    /// Create new DPI display driver (uninitialized)
    pub const fn new() -> Self {
        Self {
            config: None,
            fb_addr: 0,
            fb_size: 0,
            pitch: 0,
            initialized: false,
        }
    }

    /// Initialize DPI display with given configuration
    pub fn init(&mut self, config: &DpiConfig) -> Result<(), DpiError> {
        // 1. Configure GPIO pins for DPI (ALT2)
        self.configure_gpio_for_dpi()?;

        // 2. Store configuration
        self.config = Some(config.clone());

        // 3. Framebuffer will be allocated via mailbox in main.rs
        //    This driver just handles GPIO configuration

        self.initialized = true;
        Ok(())
    }

    /// Configure GPIO pins 0-21 for DPI function (ALT2)
    ///
    /// The GPi Case 2W uses 18-bit color (BGR666 padded), which requires:
    /// - GPIO 0: PCLK
    /// - GPIO 1: DE
    /// - GPIO 2: VSYNC
    /// - GPIO 3: HSYNC
    /// - GPIO 4-9: Blue [2:7]
    /// - GPIO 10-15: Green [2:7]
    /// - GPIO 16-21: Red [2:7]
    fn configure_gpio_for_dpi(&self) -> Result<(), DpiError> {
        // Each GPFSEL register controls 10 pins (3 bits each)

        // GPFSEL0: GPIO 0-9 (PCLK, DE, VSYNC, HSYNC, Blue[2:7])
        // All set to ALT2 (0b110)
        let gpfsel0_val: u32 =
            (GpioFunction::Alt2 as u32) << 0  |  // GPIO 0: PCLK
                (GpioFunction::Alt2 as u32) << 3  |  // GPIO 1: DE
                (GpioFunction::Alt2 as u32) << 6  |  // GPIO 2: VSYNC
                (GpioFunction::Alt2 as u32) << 9  |  // GPIO 3: HSYNC
                (GpioFunction::Alt2 as u32) << 12 |  // GPIO 4: B2
                (GpioFunction::Alt2 as u32) << 15 |  // GPIO 5: B3
                (GpioFunction::Alt2 as u32) << 18 |  // GPIO 6: B4
                (GpioFunction::Alt2 as u32) << 21 |  // GPIO 7: B5
                (GpioFunction::Alt2 as u32) << 24 |  // GPIO 8: B6
                (GpioFunction::Alt2 as u32) << 27;   // GPIO 9: B7

        // GPFSEL1: GPIO 10-19 (Green[2:7], Red[2:5])
        let gpfsel1_val: u32 =
            (GpioFunction::Alt2 as u32) << 0  |  // GPIO 10: G2
                (GpioFunction::Alt2 as u32) << 3  |  // GPIO 11: G3
                (GpioFunction::Alt2 as u32) << 6  |  // GPIO 12: G4
                (GpioFunction::Alt2 as u32) << 9  |  // GPIO 13: G5
                (GpioFunction::Alt2 as u32) << 12 |  // GPIO 14: G6
                (GpioFunction::Alt2 as u32) << 15 |  // GPIO 15: G7
                (GpioFunction::Alt2 as u32) << 18 |  // GPIO 16: R2
                (GpioFunction::Alt2 as u32) << 21 |  // GPIO 17: R3
                (GpioFunction::Alt2 as u32) << 24 |  // GPIO 18: R4
                (GpioFunction::Alt2 as u32) << 27;   // GPIO 19: R5

        // GPFSEL2: GPIO 20-21 (Red[6:7]), rest untouched
        // We only modify bits for GPIO 20-21, preserve others
        let gpfsel2_current = mmio::read(gpio_regs::GPFSEL2);
        let gpfsel2_mask: u32 = 0b111111;  // Bits 0-5 for GPIO 20-21
        let gpfsel2_val: u32 =
            (GpioFunction::Alt2 as u32) << 0 |   // GPIO 20: R6
                (GpioFunction::Alt2 as u32) << 3;    // GPIO 21: R7

        // Write to registers
        mmio::write(gpio_regs::GPFSEL0, gpfsel0_val);
        mmio::write(gpio_regs::GPFSEL1, gpfsel1_val);
        mmio::write(gpio_regs::GPFSEL2, (gpfsel2_current & !gpfsel2_mask) | gpfsel2_val);

        // Disable pull-up/down on DPI pins (they should float)
        self.disable_pulls_for_dpi();

        Ok(())
    }

    /// Disable pull-up/down resistors on DPI pins
    fn disable_pulls_for_dpi(&self) {
        // BCM2835/BCM2710 pull-up/down sequence:
        // 1. Write to GPPUD to set control signal (0 = off)
        // 2. Wait 150 cycles
        // 3. Write to GPPUDCLK0 to clock signal into pins
        // 4. Wait 150 cycles
        // 5. Write to GPPUD to remove control signal
        // 6. Write to GPPUDCLK0 to remove clock

        // Set pull to off (0)
        mmio::write(gpio_regs::GPPUD, 0);

        // Wait ~150 cycles
        for _ in 0..150 {
            core::hint::spin_loop();
        }

        // Clock the control signal into GPIO 0-21
        mmio::write(gpio_regs::GPPUDCLK0, 0x003F_FFFF);  // Bits 0-21

        // Wait ~150 cycles
        for _ in 0..150 {
            core::hint::spin_loop();
        }

        // Remove control signal
        mmio::write(gpio_regs::GPPUD, 0);
        mmio::write(gpio_regs::GPPUDCLK0, 0);
    }

    /// Set framebuffer info after mailbox allocation
    pub fn set_framebuffer(&mut self, addr: u32, size: u32, pitch: u32) {
        self.fb_addr = addr;
        self.fb_size = size;
        self.pitch = pitch;
    }

    /// Get framebuffer address
    pub fn framebuffer_addr(&self) -> u32 {
        self.fb_addr
    }

    /// Get framebuffer as mutable slice
    ///
    /// # Safety
    /// Caller must ensure exclusive access to framebuffer memory
    pub unsafe fn framebuffer_slice(&self) -> &'static mut [u8] {
        core::slice::from_raw_parts_mut(
            self.fb_addr as *mut u8,
            self.fb_size as usize,
        )
    }

    /// Check if display is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get current configuration
    pub fn config(&self) -> Option<&DpiConfig> {
        self.config.as_ref()
    }

    /// Get pitch (bytes per row)
    pub fn pitch(&self) -> u32 {
        self.pitch
    }
}

// ============================================================================
// Global Instance
// ============================================================================

/// Global DPI display instance
static mut DPI_DISPLAY: DpiDisplay = DpiDisplay::new();

/// Get DPI display instance
///
/// # Safety
/// Not thread-safe. Only call from single core during init.
pub fn get_display() -> &'static mut DpiDisplay {
    unsafe { &mut *core::ptr::addr_of_mut!(DPI_DISPLAY) }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Initialize DPI display for GPi Case 2W
pub fn init_gpi_case_2w() -> Result<(), DpiError> {
    let config = DpiConfig::gpi_case_2w();
    let display = get_display();
    display.init(&config)
}

/// Quick test: Check if DPI pins are configured correctly
pub fn verify_dpi_gpio_config() -> bool {
    let gpfsel0 = mmio::read(gpio_regs::GPFSEL0);
    let gpfsel1 = mmio::read(gpio_regs::GPFSEL1);

    // Check that all pins are set to ALT2 (0b110)
    // GPFSEL0 should be 0x36DB6DB6 for all ALT2
    // GPFSEL1 should be 0x36DB6DB6 for all ALT2

    let expected_all_alt2: u32 =
        0b110 << 0  | 0b110 << 3  | 0b110 << 6  | 0b110 << 9  |
            0b110 << 12 | 0b110 << 15 | 0b110 << 18 | 0b110 << 21 |
            0b110 << 24 | 0b110 << 27;

    gpfsel0 == expected_all_alt2 && gpfsel1 == expected_all_alt2
}
