//! Input handling for GPi Case 2W
//!
//! This module provides:
//! - Button state tracking with press/release detection
//! - Xbox 360 controller report parsing
//! - GameBoy button mapping
//!
//! The GPi Case 2W presents itself as an Xbox 360 controller over USB.
//! This module translates Xbox controller inputs to GameBoy buttons.

use crate::drivers::usb::{UsbHost, Xbox360InputReport};
use crate::platform_core::mmio::delay_ms;

// ============================================================================
// Button Bit Positions
// ============================================================================

/// Button bit flags for `GpiButtonState`
pub mod button {
    pub const UP: u16 = 1 << 0;
    pub const DOWN: u16 = 1 << 1;
    pub const LEFT: u16 = 1 << 2;
    pub const RIGHT: u16 = 1 << 3;
    pub const A: u16 = 1 << 4;
    pub const B: u16 = 1 << 5;
    pub const X: u16 = 1 << 6;
    pub const Y: u16 = 1 << 7;
    pub const START: u16 = 1 << 8;
    pub const SELECT: u16 = 1 << 9;
    pub const L: u16 = 1 << 10;
    pub const R: u16 = 1 << 11;
    pub const HOME: u16 = 1 << 12;

    // Aliases for GameBoy mapping
    pub const GB_UP: u16 = UP;
    pub const GB_DOWN: u16 = DOWN;
    pub const GB_LEFT: u16 = LEFT;
    pub const GB_RIGHT: u16 = RIGHT;
    pub const GB_A: u16 = A;
    pub const GB_B: u16 = B;
    pub const GB_START: u16 = START;
    pub const GB_SELECT: u16 = SELECT;

    /// D-pad mask (UP | DOWN | LEFT | RIGHT)
    pub const DPAD_MASK: u16 = UP | DOWN | LEFT | RIGHT;

    /// Face buttons mask (A | B | X | Y)
    pub const FACE_MASK: u16 = A | B | X | Y;

    /// Shoulder buttons mask (L | R)
    pub const SHOULDER_MASK: u16 = L | R;

    /// All buttons mask
    pub const ALL_MASK: u16 = 0x1FFF;
}

// ============================================================================
// Button State
// ============================================================================

/// GPi Case 2W button state with edge detection
///
/// Tracks both current and previous button states to detect
/// button press and release events.
#[derive(Clone, Copy, Default)]
pub struct GpiButtonState {
    /// Current button state (bit flags)
    pub current: u16,
    /// Previous button state (for edge detection)
    pub previous: u16,
}

impl GpiButtonState {
    /// Create a new button state (all buttons released)
    pub const fn new() -> Self {
        Self {
            current: 0,
            previous: 0,
        }
    }

    /// Update button state from Xbox 360 controller report
    ///
    /// This translates the Xbox controller buttons to our unified
    /// button format, preserving the previous state for edge detection.
    pub fn update_from_xbox(&mut self, report: &Xbox360InputReport) {
        self.previous = self.current;
        self.current = 0;

        // D-pad (from buttons_low)
        if report.buttons_low & Xbox360InputReport::DPAD_UP != 0 {
            self.current |= button::UP;
        }
        if report.buttons_low & Xbox360InputReport::DPAD_DOWN != 0 {
            self.current |= button::DOWN;
        }
        if report.buttons_low & Xbox360InputReport::DPAD_LEFT != 0 {
            self.current |= button::LEFT;
        }
        if report.buttons_low & Xbox360InputReport::DPAD_RIGHT != 0 {
            self.current |= button::RIGHT;
        }

        // Start/Back (from buttons_low)
        if report.buttons_low & Xbox360InputReport::START != 0 {
            self.current |= button::START;
        }
        if report.buttons_low & Xbox360InputReport::BACK != 0 {
            self.current |= button::SELECT;
        }

        // Face buttons (from buttons_high)
        if report.buttons_high & Xbox360InputReport::A != 0 {
            self.current |= button::A;
        }
        if report.buttons_high & Xbox360InputReport::B != 0 {
            self.current |= button::B;
        }
        if report.buttons_high & Xbox360InputReport::X != 0 {
            self.current |= button::X;
        }
        if report.buttons_high & Xbox360InputReport::Y != 0 {
            self.current |= button::Y;
        }

        // Shoulder buttons (from buttons_high)
        if report.buttons_high & Xbox360InputReport::LB != 0 {
            self.current |= button::L;
        }
        if report.buttons_high & Xbox360InputReport::RB != 0 {
            self.current |= button::R;
        }

        // Guide/Home button (from buttons_high)
        if report.buttons_high & Xbox360InputReport::GUIDE != 0 {
            self.current |= button::HOME;
        }
    }

    /// Check if a button is currently pressed
    #[inline]
    pub fn is_pressed(&self, btn: u16) -> bool {
        (self.current & btn) != 0
    }

    /// Check if a button was just pressed this frame
    ///
    /// Returns true only on the frame the button transitions from released to pressed.
    #[inline]
    pub fn just_pressed(&self, btn: u16) -> bool {
        (self.current & btn) != 0 && (self.previous & btn) == 0
    }

    /// Check if a button was just released this frame
    ///
    /// Returns true only on the frame the button transitions from pressed to released.
    #[inline]
    pub fn just_released(&self, btn: u16) -> bool {
        (self.current & btn) == 0 && (self.previous & btn) != 0
    }

    /// Check if any of the specified buttons are pressed
    #[inline]
    pub fn any_pressed(&self, mask: u16) -> bool {
        (self.current & mask) != 0
    }

    /// Check if all of the specified buttons are pressed
    #[inline]
    pub fn all_pressed(&self, mask: u16) -> bool {
        (self.current & mask) == mask
    }

    /// Check if any button was just pressed
    #[inline]
    pub fn any_just_pressed(&self, mask: u16) -> bool {
        let pressed_now = self.current & mask;
        let pressed_before = self.previous & mask;
        (pressed_now & !pressed_before) != 0
    }

    /// Get the raw current button state
    #[inline]
    pub fn raw(&self) -> u16 {
        self.current
    }

    /// Get buttons that changed since last update
    #[inline]
    pub fn changed(&self) -> u16 {
        self.current ^ self.previous
    }

    /// Clear all button state
    pub fn clear(&mut self) {
        self.current = 0;
        self.previous = 0;
    }

    /// Copy current to previous (for manual state management)
    pub fn latch(&mut self) {
        self.previous = self.current;
    }
}

// ============================================================================
// GameBoy Joypad State
// ============================================================================

/// GameBoy joypad register format
///
/// The GameBoy joypad register (0xFF00) uses active-low logic
/// and is split into direction and button groups.
#[derive(Clone, Copy, Default)]
pub struct GbJoypad {
    /// Direction buttons (active low): bit3=Down, bit2=Up, bit1=Left, bit0=Right
    pub directions: u8,
    /// Action buttons (active low): bit3=Start, bit2=Select, bit1=B, bit0=A
    pub buttons: u8,
}

impl GbJoypad {
    /// Create joypad state from GpiButtonState
    ///
    /// Converts our button format to the GameBoy's active-low format
    pub fn from_gpi(state: &GpiButtonState) -> Self {
        let mut directions = 0x0F; // All released (active low)
        let mut buttons = 0x0F;

        // Directions (active low)
        if state.is_pressed(button::RIGHT) {
            directions &= !0x01;
        }
        if state.is_pressed(button::LEFT) {
            directions &= !0x02;
        }
        if state.is_pressed(button::UP) {
            directions &= !0x04;
        }
        if state.is_pressed(button::DOWN) {
            directions &= !0x08;
        }

        // Buttons (active low)
        if state.is_pressed(button::A) {
            buttons &= !0x01;
        }
        if state.is_pressed(button::B) {
            buttons &= !0x02;
        }
        if state.is_pressed(button::SELECT) {
            buttons &= !0x04;
        }
        if state.is_pressed(button::START) {
            buttons &= !0x08;
        }

        Self { directions, buttons }
    }

    /// Get joypad value based on selection bits
    ///
    /// P14 (bit 4) selects direction buttons when low
    /// P15 (bit 5) selects action buttons when low
    pub fn read(&self, selection: u8) -> u8 {
        let mut result = 0x0F;

        // P14 low = select directions
        if (selection & 0x10) == 0 {
            result &= self.directions;
        }

        // P15 low = select buttons
        if (selection & 0x20) == 0 {
            result &= self.buttons;
        }

        result | (selection & 0x30) | 0xC0
    }
}

// ============================================================================
// Input Repeat Helper
// ============================================================================

/// Helper for implementing key repeat (for menu navigation)
pub struct InputRepeat {
    /// Button being repeated
    button: u16,
    /// Time of last repeat
    last_repeat: u32,
    /// Initial delay before repeat starts (microseconds)
    initial_delay: u32,
    /// Delay between repeats (microseconds)
    repeat_delay: u32,
    /// Whether initial delay has passed
    repeating: bool,
}

impl InputRepeat {
    /// Create a new input repeat helper
    ///
    /// # Arguments
    /// * `initial_delay_ms` - Delay before repeat starts (milliseconds)
    /// * `repeat_delay_ms` - Delay between repeats (milliseconds)
    pub const fn new(initial_delay_ms: u32, repeat_delay_ms: u32) -> Self {
        Self {
            button: 0,
            last_repeat: 0,
            initial_delay: initial_delay_ms * 1000,
            repeat_delay: repeat_delay_ms * 1000,
            repeating: false,
        }
    }

    /// Check if a button should trigger (including repeat)
    ///
    /// Returns true on initial press and on repeat intervals while held
    pub fn check(&mut self, state: &GpiButtonState, btn: u16, now_us: u32) -> bool {
        if state.just_pressed(btn) {
            self.button = btn;
            self.last_repeat = now_us;
            self.repeating = false;
            return true;
        }

        if state.is_pressed(btn) && self.button == btn {
            let elapsed = now_us.wrapping_sub(self.last_repeat);
            let delay = if self.repeating {
                self.repeat_delay
            } else {
                self.initial_delay
            };

            if elapsed >= delay {
                self.last_repeat = now_us;
                self.repeating = true;
                return true;
            }
        } else if !state.is_pressed(btn) && self.button == btn {
            self.button = 0;
            self.repeating = false;
        }

        false
    }

    /// Reset the repeat state
    pub fn reset(&mut self) {
        self.button = 0;
        self.repeating = false;
    }
}

// ============================================================================
// Default Repeat Timing
// ============================================================================

/// Default initial delay for key repeat (500ms)
pub const DEFAULT_REPEAT_INITIAL_MS: u32 = 500;

/// Default repeat interval (100ms = 10 repeats per second)
pub const DEFAULT_REPEAT_INTERVAL_MS: u32 = 100;

// ============================================================================
// ROM Selector Input Adapter
// ============================================================================

/// Input adapter for ROM selector
pub struct RomSelectorInput<'a> {
    usb: &'a mut UsbHost,
    report: Xbox360InputReport,
    state: GpiButtonState,
    debounce_ms: u32,
}

impl<'a> RomSelectorInput<'a> {
    pub fn new(usb: &'a mut UsbHost) -> Self {
        Self {
            usb,
            report: Xbox360InputReport::default(),
            state: GpiButtonState::new(),
            debounce_ms: 150,
        }
    }

    pub fn set_debounce(&mut self, ms: u32) {
        self.debounce_ms = ms;
    }
}

impl<'a> crate::subsystems::rom_selector::Input for RomSelectorInput<'a> {
    fn poll(&mut self) -> crate::subsystems::rom_selector::ButtonEvent {
        use crate::subsystems::rom_selector::ButtonEvent;

        if !self.usb.is_enumerated() {
            return ButtonEvent::None;
        }

        // Drain FIFO until we find an event or run out of data
        loop {
            match self.usb.read_input(&mut self.report) {
                Ok(true) => {
                    self.state.update_from_xbox(&self.report);

                    let event = if self.state.just_pressed(button::UP) {
                        ButtonEvent::Up
                    } else if self.state.just_pressed(button::DOWN) {
                        ButtonEvent::Down
                    } else if self.state.just_pressed(button::LEFT) || self.state.just_pressed(button::L) {
                        ButtonEvent::Left
                    } else if self.state.just_pressed(button::RIGHT) || self.state.just_pressed(button::R) {
                        ButtonEvent::Right
                    } else if self.state.just_pressed(button::A) {
                        ButtonEvent::Select
                    } else if self.state.just_pressed(button::B) {
                        ButtonEvent::Back
                    } else {
                        ButtonEvent::None
                    };

                    if event != ButtonEvent::None {
                        return event;
                    }
                    // If no event, catch up with real-time by continuing to read
                }
                _ => return ButtonEvent::None,
            }
        }
    }
}
