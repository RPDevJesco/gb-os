//! DPI Display Driver for GPi Case 2W
//!
//! Configures GPIO pins 0-21 for DPI (Display Parallel Interface) output.
//! The actual framebuffer is allocated via VideoCore mailbox in hal_impl.rs.

use super::gpio::{self, GpioFunction};

// ============================================================================
// DPI Configuration
// ============================================================================

/// DPI display configuration for GPi Case 2W
pub struct DpiConfig {
    pub width: u32,
    pub height: u32,
    pub pixel_clock_hz: u32,
    pub h_sync_polarity: bool,
    pub v_sync_polarity: bool,
}

impl DpiConfig {
    /// Default configuration for GPi Case 2W (640x480 @ 60Hz)
    pub const fn gpi_case_2w() -> Self {
        Self {
            width: 640,
            height: 480,
            pixel_clock_hz: 19_200_000,  // 19.2 MHz
            h_sync_polarity: false,       // Active low
            v_sync_polarity: false,       // Active low
        }
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize GPIO pins for DPI output
/// 
/// This configures GPIO 0-21 as ALT2 (DPI function).
/// The actual display timing is controlled by config.txt settings
/// which are read by the VideoCore firmware at boot.
pub fn init_gpio() {
    // Configure all 22 DPI pins as ALT2
    for pin in 0..=21 {
        gpio::set_function(pin, GpioFunction::Alt2);
    }
}

/// DPI Pin Assignments (GPi Case 2W with 18-bit BGR666 color)
/// 
/// | GPIO | Function | Description     |
/// |------|----------|-----------------|
/// | 0    | CLK      | Pixel clock     |
/// | 1    | DEN      | Data enable     |
/// | 2    | V_SYNC   | Vertical sync   |
/// | 3    | H_SYNC   | Horizontal sync |
/// | 4    | D0       | Blue bit 2      |
/// | 5    | D1       | Blue bit 3      |
/// | 6    | D2       | Blue bit 4      |
/// | 7    | D3       | Blue bit 5      |
/// | 8    | D4       | Blue bit 6      |
/// | 9    | D5       | Blue bit 7      |
/// | 10   | D6       | Green bit 2     |
/// | 11   | D7       | Green bit 3     |
/// | 12   | D8       | Green bit 4     |
/// | 13   | D9       | Green bit 5     |
/// | 14   | D10      | Green bit 6     |
/// | 15   | D11      | Green bit 7     |
/// | 16   | D12      | Red bit 2       |
/// | 17   | D13      | Red bit 3       |
/// | 18   | D14      | Red bit 4       |
/// | 19   | D15      | Red bit 5       |
/// | 20   | D16      | Red bit 6       |
/// | 21   | D17      | Red bit 7       |

// ============================================================================
// Config.txt Settings (for reference)
// ============================================================================

/// Required config.txt settings for GPi Case 2W DPI display:
pub const CONFIG_TXT_TEMPLATE: &str = r#"
# GB-OS GPi Case 2W Configuration
# ================================

# GPU Memory (32MB for VideoCore)
gpu_mem=32

# Disable HDMI (required for DPI)
hdmi_blanking=2

# Enable DPI LCD
enable_dpi_lcd=1
display_default_lcd=1

# DPI output format: BGR666 (18-bit)
dpi_output_format=0x6f016

# DPI timing group and mode
dpi_group=2
dpi_mode=87

# Custom timing: 640x480 @ 60Hz
dpi_timings=640 0 16 64 80 480 0 4 15 13 0 0 0 60 0 25000000 1

# Framebuffer settings
framebuffer_width=640
framebuffer_height=480

# No overscan
disable_overscan=1
overscan_left=0
overscan_right=0
overscan_top=0
overscan_bottom=0

# Boot kernel
kernel=kernel8.img
arm_64bit=1
"#;
