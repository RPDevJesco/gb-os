//! Unified Input System
//!
//! Provides a common interface for button input from multiple sources:
//! - Direct GPIO buttons (original GPi Case)
//! - USB HID gamepad (GPi Case 2W)
//!
//! # Features
//!
//! - Type-safe `Button` enum
//! - Edge detection (just pressed/released)
//! - Game Boy key mapping
//! - Menu navigation actions
//! - Batch change detection for performance
//!
//! # Usage
//!
//! ```rust
//! // Initialize (choose one backend)
//! input::init_gpio();
//! // or
//! input::init_usb(&usb_host, &usb_gamepad);
//!
//! // Main loop
//! loop {
//!     input::update();
//!
//!     if input::just_pressed(Button::A) {
//!         // Handle A button press
//!     }
//!
//!     let gb_keys = input::gb_keys();
//!     emulator.set_keys(gb_keys);
//! }
//! ```

use crate::gpio::{self, Function, Pull};

// ============================================================================
// GPIO Pin Assignments (GPi Case 2W direct GPIO)
// ============================================================================

/// D-Pad GPIO pins
mod dpad_pins {
    pub const UP: u8 = 5;
    pub const DOWN: u8 = 6;
    pub const LEFT: u8 = 13;
    pub const RIGHT: u8 = 19;
}

/// Face button GPIO pins
mod face_pins {
    pub const A: u8 = 26;
    pub const B: u8 = 21;
    pub const X: u8 = 4;
    pub const Y: u8 = 12;
}

/// Menu button GPIO pins
mod menu_pins {
    pub const START: u8 = 22;
    pub const SELECT: u8 = 17;
    pub const HOME: u8 = 27;
}

/// Shoulder button GPIO pins
mod shoulder_pins {
    pub const L: u8 = 16;
    pub const R: u8 = 20;
}

/// All button GPIO pins for initialization
const ALL_GPIO_BUTTONS: [u8; 13] = [
    dpad_pins::UP,
    dpad_pins::DOWN,
    dpad_pins::LEFT,
    dpad_pins::RIGHT,
    face_pins::A,
    face_pins::B,
    face_pins::X,
    face_pins::Y,
    menu_pins::START,
    menu_pins::SELECT,
    menu_pins::HOME,
    shoulder_pins::L,
    shoulder_pins::R,
];

// ============================================================================
// Button Definitions
// ============================================================================

/// Button identifiers with bit flag values.
///
/// These can be used directly as bit masks or with the helper methods.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Right = 1 << 0,
    Left = 1 << 1,
    Up = 1 << 2,
    Down = 1 << 3,
    A = 1 << 4,
    B = 1 << 5,
    Select = 1 << 6,
    Start = 1 << 7,
    X = 1 << 8,
    Y = 1 << 9,
    L = 1 << 10,
    R = 1 << 11,
    Home = 1 << 12,
}

impl Button {
    /// All D-Pad buttons combined.
    pub const DPAD_MASK: u16 = 0x000F;
    /// All face buttons combined (A, B, X, Y).
    pub const FACE_MASK: u16 = 0x03F0;
    /// Start and Select buttons.
    pub const MENU_MASK: u16 = 0x00C0;
    /// Shoulder buttons (L, R).
    pub const SHOULDER_MASK: u16 = 0x0C00;
    /// Home button.
    pub const HOME_MASK: u16 = 0x1000;

    /// Convert to bit mask.
    #[inline]
    pub const fn mask(self) -> u16 {
        self as u16
    }
}

// ============================================================================
// Game Boy Key Mapping
// ============================================================================

/// Game Boy key indices (matches hardware register layout).
///
/// The Game Boy has 8 keys that map to bits in the joypad register.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GbKey {
    Right = 0,
    Left = 1,
    Up = 2,
    Down = 3,
    A = 4,
    B = 5,
    Select = 6,
    Start = 7,
}

impl GbKey {
    /// Convert to bit mask.
    #[inline]
    pub const fn mask(self) -> u8 {
        1 << (self as u8)
    }
}

// ============================================================================
// Menu Actions
// ============================================================================

/// High-level input actions for menus and UI navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// No action.
    None,
    /// Navigate up.
    Up,
    /// Navigate down.
    Down,
    /// Navigate left.
    Left,
    /// Navigate right.
    Right,
    /// Confirm/select (A or Start).
    Select,
    /// Cancel/back (B).
    Back,
    /// Page up (L shoulder).
    PageUp,
    /// Page down (R shoulder).
    PageDown,
    /// Home/menu button.
    Home,
}

// ============================================================================
// Input State
// ============================================================================

/// Current and previous button state for edge detection.
#[derive(Debug, Clone, Copy, Default)]
pub struct InputState {
    /// Current button state (bit flags).
    pub current: u16,
    /// Previous frame's button state.
    pub previous: u16,
}

impl InputState {
    /// Create a new input state.
    pub const fn new() -> Self {
        Self {
            current: 0,
            previous: 0,
        }
    }

    /// Update state with new button readings.
    #[inline]
    pub fn update(&mut self, buttons: u16) {
        self.previous = self.current;
        self.current = buttons;
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

    /// Get all buttons that were just pressed this frame.
    #[inline]
    pub fn newly_pressed(&self) -> u16 {
        self.current & !self.previous
    }

    /// Get all buttons that were just released this frame.
    #[inline]
    pub fn newly_released(&self) -> u16 {
        !self.current & self.previous
    }

    /// Check if any button is pressed.
    #[inline]
    pub fn any_pressed(&self) -> bool {
        self.current != 0
    }

    /// Convert current state to Game Boy key state (8-bit).
    ///
    /// Only the 8 Game Boy keys are mapped; X, Y, L, R, Home are ignored.
    pub fn to_gb_keys(&self) -> u8 {
        let mut gb = 0u8;

        if self.is_pressed(Button::Right) {
            gb |= GbKey::Right.mask();
        }
        if self.is_pressed(Button::Left) {
            gb |= GbKey::Left.mask();
        }
        if self.is_pressed(Button::Up) {
            gb |= GbKey::Up.mask();
        }
        if self.is_pressed(Button::Down) {
            gb |= GbKey::Down.mask();
        }
        if self.is_pressed(Button::A) {
            gb |= GbKey::A.mask();
        }
        if self.is_pressed(Button::B) {
            gb |= GbKey::B.mask();
        }
        if self.is_pressed(Button::Select) {
            gb |= GbKey::Select.mask();
        }
        if self.is_pressed(Button::Start) {
            gb |= GbKey::Start.mask();
        }

        gb
    }

    /// Get Game Boy keys that were just pressed.
    pub fn gb_keys_pressed(&self) -> u8 {
        let mut gb = 0u8;

        if self.just_pressed(Button::Right) {
            gb |= GbKey::Right.mask();
        }
        if self.just_pressed(Button::Left) {
            gb |= GbKey::Left.mask();
        }
        if self.just_pressed(Button::Up) {
            gb |= GbKey::Up.mask();
        }
        if self.just_pressed(Button::Down) {
            gb |= GbKey::Down.mask();
        }
        if self.just_pressed(Button::A) {
            gb |= GbKey::A.mask();
        }
        if self.just_pressed(Button::B) {
            gb |= GbKey::B.mask();
        }
        if self.just_pressed(Button::Select) {
            gb |= GbKey::Select.mask();
        }
        if self.just_pressed(Button::Start) {
            gb |= GbKey::Start.mask();
        }

        gb
    }

    /// Get Game Boy keys that were just released.
    pub fn gb_keys_released(&self) -> u8 {
        let mut gb = 0u8;

        if self.just_released(Button::Right) {
            gb |= GbKey::Right.mask();
        }
        if self.just_released(Button::Left) {
            gb |= GbKey::Left.mask();
        }
        if self.just_released(Button::Up) {
            gb |= GbKey::Up.mask();
        }
        if self.just_released(Button::Down) {
            gb |= GbKey::Down.mask();
        }
        if self.just_released(Button::A) {
            gb |= GbKey::A.mask();
        }
        if self.just_released(Button::B) {
            gb |= GbKey::B.mask();
        }
        if self.just_released(Button::Select) {
            gb |= GbKey::Select.mask();
        }
        if self.just_released(Button::Start) {
            gb |= GbKey::Start.mask();
        }

        gb
    }

    /// Get menu action from current input state.
    ///
    /// Returns the highest priority action based on just-pressed buttons.
    pub fn menu_action(&self) -> MenuAction {
        // Priority order: D-Pad, then action buttons
        if self.just_pressed(Button::Up) {
            return MenuAction::Up;
        }
        if self.just_pressed(Button::Down) {
            return MenuAction::Down;
        }
        if self.just_pressed(Button::Left) {
            return MenuAction::Left;
        }
        if self.just_pressed(Button::Right) {
            return MenuAction::Right;
        }
        if self.just_pressed(Button::A) || self.just_pressed(Button::Start) {
            return MenuAction::Select;
        }
        if self.just_pressed(Button::B) {
            return MenuAction::Back;
        }
        if self.just_pressed(Button::L) {
            return MenuAction::PageUp;
        }
        if self.just_pressed(Button::R) {
            return MenuAction::PageDown;
        }
        if self.just_pressed(Button::Home) {
            return MenuAction::Home;
        }

        MenuAction::None
    }
}

// ============================================================================
// Input Backend
// ============================================================================

/// Input backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputBackend {
    /// No backend configured.
    None,
    /// Direct GPIO buttons.
    Gpio,
    /// USB HID gamepad (state managed externally).
    Usb,
}

// ============================================================================
// Global Input State
// ============================================================================

/// Global input manager state.
struct InputManager {
    backend: InputBackend,
    state: InputState,
    gpio_initialized: bool,
}

impl InputManager {
    const fn new() -> Self {
        Self {
            backend: InputBackend::None,
            state: InputState::new(),
            gpio_initialized: false,
        }
    }
}

static mut INPUT_MANAGER: InputManager = InputManager::new();

// ============================================================================
// GPIO Input Functions
// ============================================================================

/// Read current button state from GPIO pins.
///
/// All buttons are active-low (pressed = pin reads low).
fn read_gpio_buttons() -> u16 {
    let mut state = 0u16;

    // D-Pad (active low)
    if !gpio::read(dpad_pins::UP) {
        state |= Button::Up as u16;
    }
    if !gpio::read(dpad_pins::DOWN) {
        state |= Button::Down as u16;
    }
    if !gpio::read(dpad_pins::LEFT) {
        state |= Button::Left as u16;
    }
    if !gpio::read(dpad_pins::RIGHT) {
        state |= Button::Right as u16;
    }

    // Face buttons
    if !gpio::read(face_pins::A) {
        state |= Button::A as u16;
    }
    if !gpio::read(face_pins::B) {
        state |= Button::B as u16;
    }
    if !gpio::read(face_pins::X) {
        state |= Button::X as u16;
    }
    if !gpio::read(face_pins::Y) {
        state |= Button::Y as u16;
    }

    // Menu buttons
    if !gpio::read(menu_pins::START) {
        state |= Button::Start as u16;
    }
    if !gpio::read(menu_pins::SELECT) {
        state |= Button::Select as u16;
    }
    if !gpio::read(menu_pins::HOME) {
        state |= Button::Home as u16;
    }

    // Shoulder buttons
    if !gpio::read(shoulder_pins::L) {
        state |= Button::L as u16;
    }
    if !gpio::read(shoulder_pins::R) {
        state |= Button::R as u16;
    }

    state
}

// ============================================================================
// Public API - Initialization
// ============================================================================

/// Initialize GPIO-based input.
///
/// Configures all button GPIO pins as inputs with pull-ups.
pub fn init_gpio() {
    unsafe {
        // Configure GPIO pins
        for &pin in &ALL_GPIO_BUTTONS {
            gpio::set_function(pin, Function::Input);
        }

        // Enable pull-ups on all button pins at once
        gpio::set_pull_multi(&ALL_GPIO_BUTTONS, Pull::Up);

        INPUT_MANAGER.backend = InputBackend::Gpio;
        INPUT_MANAGER.gpio_initialized = true;
    }
}

/// Initialize USB-based input.
///
/// Call this when using a USB gamepad instead of GPIO buttons.
/// The USB gamepad must be polled separately and state passed to `update_usb()`.
pub fn init_usb() {
    unsafe {
        INPUT_MANAGER.backend = InputBackend::Usb;
    }
}

/// Check if input system is initialized.
pub fn is_initialized() -> bool {
    unsafe { INPUT_MANAGER.backend != InputBackend::None }
}

// ============================================================================
// Public API - Update
// ============================================================================

/// Update input state (call once per frame).
///
/// For GPIO backend, this reads the current pin states.
/// For USB backend, use `update_usb()` instead.
pub fn update() {
    unsafe {
        match INPUT_MANAGER.backend {
            InputBackend::Gpio => {
                let buttons = read_gpio_buttons();
                INPUT_MANAGER.state.update(buttons);
            }
            InputBackend::Usb => {
                // USB state is updated via update_usb()
            }
            InputBackend::None => {}
        }
    }
}

/// Update input state from USB gamepad.
///
/// Call this with the current button state from the USB gamepad driver.
pub fn update_usb(buttons: u16) {
    unsafe {
        if INPUT_MANAGER.backend == InputBackend::Usb {
            INPUT_MANAGER.state.update(buttons);
        }
    }
}

// ============================================================================
// Public API - Button State Queries
// ============================================================================

/// Get the current input state.
pub fn state() -> InputState {
    unsafe { INPUT_MANAGER.state }
}

/// Get current button state as raw bits.
#[inline]
pub fn raw() -> u16 {
    unsafe { INPUT_MANAGER.state.current }
}

/// Check if a button is currently pressed.
#[inline]
pub fn is_pressed(button: Button) -> bool {
    unsafe { INPUT_MANAGER.state.is_pressed(button) }
}

/// Check if a button was just pressed this frame.
#[inline]
pub fn just_pressed(button: Button) -> bool {
    unsafe { INPUT_MANAGER.state.just_pressed(button) }
}

/// Check if a button was just released this frame.
#[inline]
pub fn just_released(button: Button) -> bool {
    unsafe { INPUT_MANAGER.state.just_released(button) }
}

/// Get all buttons that were just pressed this frame.
#[inline]
pub fn newly_pressed() -> u16 {
    unsafe { INPUT_MANAGER.state.newly_pressed() }
}

/// Get all buttons that were just released this frame.
#[inline]
pub fn newly_released() -> u16 {
    unsafe { INPUT_MANAGER.state.newly_released() }
}

/// Get both newly pressed and released buttons.
///
/// Returns `(pressed, released)` for efficient batch processing.
#[inline]
pub fn changes() -> (u16, u16) {
    unsafe {
        (
            INPUT_MANAGER.state.newly_pressed(),
            INPUT_MANAGER.state.newly_released(),
        )
    }
}

/// Check if any button is pressed.
#[inline]
pub fn any_pressed() -> bool {
    unsafe { INPUT_MANAGER.state.any_pressed() }
}

// ============================================================================
// Public API - Game Boy Keys
// ============================================================================

/// Get current Game Boy key state (8-bit).
#[inline]
pub fn gb_keys() -> u8 {
    unsafe { INPUT_MANAGER.state.to_gb_keys() }
}

/// Get Game Boy keys that were just pressed.
#[inline]
pub fn gb_keys_pressed() -> u8 {
    unsafe { INPUT_MANAGER.state.gb_keys_pressed() }
}

/// Get Game Boy keys that were just released.
#[inline]
pub fn gb_keys_released() -> u8 {
    unsafe { INPUT_MANAGER.state.gb_keys_released() }
}

// ============================================================================
// Public API - Menu Navigation
// ============================================================================

/// Get the current menu action.
///
/// Returns the highest priority action based on just-pressed buttons.
#[inline]
pub fn menu_action() -> MenuAction {
    unsafe { INPUT_MANAGER.state.menu_action() }
}

// ============================================================================
// Wait Functions
// ============================================================================

/// Wait until any button is pressed.
pub fn wait_for_press() {
    loop {
        update();
        if newly_pressed() != 0 {
            return;
        }
        core::hint::spin_loop();
    }
}

/// Wait until a specific button is pressed.
pub fn wait_for_button(button: Button) {
    loop {
        update();
        if just_pressed(button) {
            return;
        }
        core::hint::spin_loop();
    }
}

/// Wait until all buttons are released.
pub fn wait_for_release() {
    loop {
        update();
        if !any_pressed() {
            return;
        }
        core::hint::spin_loop();
    }
}
