//! USB HID Gamepad Input Driver
//!
//! Handles USB HID input from gamepads, specifically optimized for
//! Xbox 360-compatible controllers (like GPi Case 2W internal gamepad).
//!
//! # Usage
//!
//! ```rust
//! let mut usb = UsbHost::new();
//! usb.init()?;
//! usb.wait_for_connection(3000);
//! usb.reset_port()?;
//! usb.enumerate()?;
//!
//! let mut gamepad = UsbGamepad::new();
//! gamepad.configure(&mut usb)?;
//!
//! loop {
//!     gamepad.poll(&mut usb);
//!     if gamepad.just_pressed(Button::A) {
//!         // Handle button press
//!     }
//! }
//! ```

use crate::usb_host::{UsbHost, EndpointType, TransferResult, SetupPacket};
use crate::mmio;

// ============================================================================
// USB Descriptor Constants
// ============================================================================

const USB_DESC_ENDPOINT: u8 = 0x05;
const USB_DESC_HID: u8 = 0x21;

// ============================================================================
// Xbox 360 HID Report
// ============================================================================

/// Xbox 360 controller input report structure.
///
/// This matches the HID report format used by Xbox 360 controllers
/// and compatible gamepads.
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct Xbox360Report {
    /// Report ID (usually 0x00)
    pub report_id: u8,
    /// Report length (usually 0x14 = 20)
    pub report_length: u8,
    /// Low byte of digital buttons (D-pad, Start, Back, L3, R3)
    pub buttons_low: u8,
    /// High byte of digital buttons (LB, RB, Guide, A, B, X, Y)
    pub buttons_high: u8,
    /// Left trigger (0-255)
    pub left_trigger: u8,
    /// Right trigger (0-255)
    pub right_trigger: u8,
    /// Left stick X axis (-32768 to 32767)
    pub left_stick_x: i16,
    /// Left stick Y axis (-32768 to 32767)
    pub left_stick_y: i16,
    /// Right stick X axis (-32768 to 32767)
    pub right_stick_x: i16,
    /// Right stick Y axis (-32768 to 32767)
    pub right_stick_y: i16,
    /// Reserved bytes
    pub _reserved: [u8; 6],
}

impl Xbox360Report {
    // buttons_low bits
    pub const DPAD_UP: u8 = 1 << 0;
    pub const DPAD_DOWN: u8 = 1 << 1;
    pub const DPAD_LEFT: u8 = 1 << 2;
    pub const DPAD_RIGHT: u8 = 1 << 3;
    pub const START: u8 = 1 << 4;
    pub const BACK: u8 = 1 << 5;
    pub const LEFT_STICK: u8 = 1 << 6;
    pub const RIGHT_STICK: u8 = 1 << 7;

    // buttons_high bits
    pub const LB: u8 = 1 << 0;
    pub const RB: u8 = 1 << 1;
    pub const GUIDE: u8 = 1 << 2;
    // bit 3 unused
    pub const A: u8 = 1 << 4;
    pub const B: u8 = 1 << 5;
    pub const X: u8 = 1 << 6;
    pub const Y: u8 = 1 << 7;
}

// ============================================================================
// Generic HID Report (fallback)
// ============================================================================

/// Generic HID gamepad report.
///
/// Used as a fallback for non-Xbox controllers.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GenericHidReport {
    pub data: [u8; 64],
}

impl Default for GenericHidReport {
    fn default() -> Self {
        Self { data: [0u8; 64] }
    }
}

// ============================================================================
// Button Definitions
// ============================================================================

/// Button identifiers.
///
/// These map to a common set of buttons found on most gamepads.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    DpadUp = 1 << 0,
    DpadDown = 1 << 1,
    DpadLeft = 1 << 2,
    DpadRight = 1 << 3,
    A = 1 << 4,
    B = 1 << 5,
    X = 1 << 6,
    Y = 1 << 7,
    Start = 1 << 8,
    Select = 1 << 9, // Back on Xbox
    L = 1 << 10,     // LB on Xbox
    R = 1 << 11,     // RB on Xbox
    Home = 1 << 12,  // Guide on Xbox
    L3 = 1 << 13,    // Left stick click
    R3 = 1 << 14,    // Right stick click
}

impl Button {
    /// All D-pad buttons.
    pub const DPAD_MASK: u16 = 0x000F;
    /// All face buttons (A, B, X, Y).
    pub const FACE_MASK: u16 = 0x00F0;
    /// Menu buttons (Start, Select, Home).
    pub const MENU_MASK: u16 = 0x1700;
    /// Shoulder buttons (L, R).
    pub const SHOULDER_MASK: u16 = 0x0C00;

    /// Convert button to bit mask.
    #[inline]
    pub fn mask(self) -> u16 {
        self as u16
    }
}

// ============================================================================
// Button State
// ============================================================================

/// Button state with edge detection.
///
/// Tracks current and previous button states to detect
/// button presses and releases.
#[derive(Clone, Copy, Default)]
pub struct ButtonState {
    /// Current button state (bit flags).
    pub current: u16,
    /// Previous frame's button state.
    pub previous: u16,
    /// Left trigger value (0-255).
    pub left_trigger: u8,
    /// Right trigger value (0-255).
    pub right_trigger: u8,
    /// Left stick X (-32768 to 32767).
    pub left_stick_x: i16,
    /// Left stick Y (-32768 to 32767).
    pub left_stick_y: i16,
    /// Right stick X (-32768 to 32767).
    pub right_stick_x: i16,
    /// Right stick Y (-32768 to 32767).
    pub right_stick_y: i16,
}

impl ButtonState {
    /// Create new empty button state.
    pub const fn new() -> Self {
        Self {
            current: 0,
            previous: 0,
            left_trigger: 0,
            right_trigger: 0,
            left_stick_x: 0,
            left_stick_y: 0,
            right_stick_x: 0,
            right_stick_y: 0,
        }
    }

    /// Update state from Xbox 360 report.
    pub fn update_from_xbox(&mut self, report: &Xbox360Report) {
        self.previous = self.current;

        let low = report.buttons_low;
        let high = report.buttons_high;

        // Map Xbox buttons to our common format
        let mut state: u16 = 0;

        // D-pad
        if low & Xbox360Report::DPAD_UP != 0 {
            state |= Button::DpadUp as u16;
        }
        if low & Xbox360Report::DPAD_DOWN != 0 {
            state |= Button::DpadDown as u16;
        }
        if low & Xbox360Report::DPAD_LEFT != 0 {
            state |= Button::DpadLeft as u16;
        }
        if low & Xbox360Report::DPAD_RIGHT != 0 {
            state |= Button::DpadRight as u16;
        }

        // Face buttons
        if high & Xbox360Report::A != 0 {
            state |= Button::A as u16;
        }
        if high & Xbox360Report::B != 0 {
            state |= Button::B as u16;
        }
        if high & Xbox360Report::X != 0 {
            state |= Button::X as u16;
        }
        if high & Xbox360Report::Y != 0 {
            state |= Button::Y as u16;
        }

        // Menu buttons
        if low & Xbox360Report::START != 0 {
            state |= Button::Start as u16;
        }
        if low & Xbox360Report::BACK != 0 {
            state |= Button::Select as u16;
        }
        if high & Xbox360Report::GUIDE != 0 {
            state |= Button::Home as u16;
        }

        // Shoulder buttons
        if high & Xbox360Report::LB != 0 {
            state |= Button::L as u16;
        }
        if high & Xbox360Report::RB != 0 {
            state |= Button::R as u16;
        }

        // Stick clicks
        if low & Xbox360Report::LEFT_STICK != 0 {
            state |= Button::L3 as u16;
        }
        if low & Xbox360Report::RIGHT_STICK != 0 {
            state |= Button::R3 as u16;
        }

        self.current = state;

        // Analog values
        self.left_trigger = report.left_trigger;
        self.right_trigger = report.right_trigger;
        self.left_stick_x = report.left_stick_x;
        self.left_stick_y = report.left_stick_y;
        self.right_stick_x = report.right_stick_x;
        self.right_stick_y = report.right_stick_y;
    }

    /// Check if a button is currently pressed.
    #[inline]
    pub fn is_pressed(&self, button: Button) -> bool {
        (self.current & button as u16) != 0
    }

    /// Check if a button was just pressed this frame.
    #[inline]
    pub fn just_pressed(&self, button: Button) -> bool {
        let mask = button as u16;
        (self.current & mask) != 0 && (self.previous & mask) == 0
    }

    /// Check if a button was just released this frame.
    #[inline]
    pub fn just_released(&self, button: Button) -> bool {
        let mask = button as u16;
        (self.current & mask) == 0 && (self.previous & mask) != 0
    }

    /// Get all buttons that were just pressed.
    #[inline]
    pub fn newly_pressed(&self) -> u16 {
        self.current & !self.previous
    }

    /// Get all buttons that were just released.
    #[inline]
    pub fn newly_released(&self) -> u16 {
        !self.current & self.previous
    }

    /// Check if any button is pressed.
    #[inline]
    pub fn any_pressed(&self) -> bool {
        self.current != 0
    }

    /// Check if left trigger is pressed (threshold: 128).
    #[inline]
    pub fn left_trigger_pressed(&self) -> bool {
        self.left_trigger >= 128
    }

    /// Check if right trigger is pressed (threshold: 128).
    #[inline]
    pub fn right_trigger_pressed(&self) -> bool {
        self.right_trigger >= 128
    }
}

// ============================================================================
// USB Gamepad Driver
// ============================================================================

/// HID transfer PID values.
const HCTSIZ_PID_DATA0: u32 = 0 << 29;
const HCTSIZ_PID_DATA1: u32 = 2 << 29;

/// USB HID gamepad driver.
///
/// Manages communication with a USB HID gamepad and provides
/// a clean button state interface.
pub struct UsbGamepad {
    /// HID interrupt endpoint number.
    endpoint: u8,
    /// HID endpoint max packet size.
    max_packet: u16,
    /// Current data toggle for endpoint.
    data_toggle: bool,
    /// Whether the gamepad is configured.
    configured: bool,
    /// Current button state.
    state: ButtonState,
}

impl UsbGamepad {
    /// Create a new USB gamepad driver.
    pub const fn new() -> Self {
        Self {
            endpoint: 0,
            max_packet: 0,
            data_toggle: false,
            configured: false,
            state: ButtonState::new(),
        }
    }

    /// Configure the gamepad by finding the HID interrupt endpoint.
    ///
    /// This should be called after the USB device is enumerated.
    pub fn configure(&mut self, usb: &mut UsbHost) -> Result<(), &'static str> {
        if !usb.is_enumerated() {
            return Err("USB device not enumerated");
        }

        // Get configuration descriptor
        let mut config_buf = [0u8; 256];
        let setup = SetupPacket::get_descriptor(2, 0, 256);
        let len = usb.control_transfer(&setup, Some(&mut config_buf))?;

        // Parse descriptors to find HID interrupt IN endpoint
        let mut pos = 0;
        while pos + 2 <= len {
            let desc_len = config_buf[pos] as usize;
            let desc_type = config_buf[pos + 1];

            if desc_len == 0 || pos + desc_len > len {
                break;
            }

            if desc_type == USB_DESC_ENDPOINT && desc_len >= 7 {
                let ep_addr = config_buf[pos + 2];
                let ep_attr = config_buf[pos + 3];
                let ep_max_pkt =
                    u16::from_le_bytes([config_buf[pos + 4], config_buf[pos + 5]]);

                let is_in = (ep_addr & 0x80) != 0;
                let ep_type = ep_attr & 0x03;

                // Looking for Interrupt IN endpoint
                if is_in && ep_type == 3 {
                    self.endpoint = ep_addr & 0x0F;
                    self.max_packet = ep_max_pkt;
                    self.data_toggle = false;
                    self.configured = true;
                    return Ok(());
                }
            }

            pos += desc_len;
        }

        Err("No HID interrupt endpoint found")
    }

    /// Poll the gamepad for new input.
    ///
    /// This should be called once per frame. Returns `true` if new
    /// data was received.
    pub fn poll(&mut self, usb: &UsbHost) -> bool {
        if !self.configured || self.endpoint == 0 {
            return false;
        }

        const CH: usize = 1; // Use channel 1 for HID

        let pid = if self.data_toggle {
            HCTSIZ_PID_DATA1
        } else {
            HCTSIZ_PID_DATA0
        };

        let len = core::mem::size_of::<Xbox360Report>().min(self.max_packet as usize);
        let mut report = Xbox360Report::default();
        let report_bytes =
            unsafe { core::slice::from_raw_parts_mut(&mut report as *mut _ as *mut u8, len) };

        match usb.do_transfer(
            CH,
            self.endpoint,
            true,
            EndpointType::Interrupt,
            pid,
            report_bytes,
            len,
        ) {
            TransferResult::Success(_) => {
                self.data_toggle = !self.data_toggle;
                self.state.update_from_xbox(&report);
                true
            }
            TransferResult::Nak | TransferResult::Timeout => false,
            _ => false,
        }
    }

    /// Get the current button state.
    #[inline]
    pub fn state(&self) -> &ButtonState {
        &self.state
    }

    /// Check if a button is currently pressed.
    #[inline]
    pub fn is_pressed(&self, button: Button) -> bool {
        self.state.is_pressed(button)
    }

    /// Check if a button was just pressed this frame.
    #[inline]
    pub fn just_pressed(&self, button: Button) -> bool {
        self.state.just_pressed(button)
    }

    /// Check if a button was just released this frame.
    #[inline]
    pub fn just_released(&self, button: Button) -> bool {
        self.state.just_released(button)
    }

    /// Get all newly pressed buttons as a bitmask.
    #[inline]
    pub fn newly_pressed(&self) -> u16 {
        self.state.newly_pressed()
    }

    /// Get all newly released buttons as a bitmask.
    #[inline]
    pub fn newly_released(&self) -> u16 {
        self.state.newly_released()
    }

    /// Get current button state as raw bits.
    #[inline]
    pub fn raw_buttons(&self) -> u16 {
        self.state.current
    }

    /// Check if gamepad is configured.
    #[inline]
    pub fn is_configured(&self) -> bool {
        self.configured
    }

    /// Get the left stick position.
    #[inline]
    pub fn left_stick(&self) -> (i16, i16) {
        (self.state.left_stick_x, self.state.left_stick_y)
    }

    /// Get the right stick position.
    #[inline]
    pub fn right_stick(&self) -> (i16, i16) {
        (self.state.right_stick_x, self.state.right_stick_y)
    }

    /// Get trigger values (left, right).
    #[inline]
    pub fn triggers(&self) -> (u8, u8) {
        (self.state.left_trigger, self.state.right_trigger)
    }
}

impl Default for UsbGamepad {
    fn default() -> Self {
        Self::new()
    }
}
