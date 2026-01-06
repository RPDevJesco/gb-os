//! GPi Case 2W Input Handler
//!
//! Handles button input from the GPi Case 2W GPIO pins.
//! All buttons are directly connected to GPIO and active-low.

use crate::gpio::{self, Function, Pull};

// ============================================================================
// GPIO Pin Assignments (GPi Case 2W)
// ============================================================================

/// D-Pad GPIO pins
mod dpad {
    pub const UP: u8 = 5;
    pub const DOWN: u8 = 6;
    pub const LEFT: u8 = 13;
    pub const RIGHT: u8 = 19;
}

/// Face button GPIO pins
mod face {
    pub const A: u8 = 26;
    pub const B: u8 = 21;
    pub const X: u8 = 4;
    pub const Y: u8 = 12;
}

/// Menu button GPIO pins
mod menu {
    pub const START: u8 = 22;
    pub const SELECT: u8 = 17;
    pub const HOME: u8 = 27;
}

/// Shoulder button GPIO pins
mod shoulder {
    pub const L: u8 = 16;
    pub const R: u8 = 20;
}

// ============================================================================
// Button Bit Flags
// ============================================================================

/// Button state bit flags for 16-bit packed state.
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
    /// All face buttons combined.
    pub const FACE_MASK: u16 = 0x00F0;
    /// All menu buttons combined.
    pub const MENU_MASK: u16 = 0x1F00;
    /// All shoulder buttons combined.
    pub const SHOULDER_MASK: u16 = 0x0C00;
}

// ============================================================================
// Game Boy Key Mapping
// ============================================================================

/// Game Boy key indices (matches hardware register layout).
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
    pub fn mask(self) -> u8 {
        1 << (self as u8)
    }
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

    /// Get buttons that were just pressed.
    #[inline]
    pub fn pressed_this_frame(&self) -> u16 {
        self.current & !self.previous
    }

    /// Get buttons that were just released.
    #[inline]
    pub fn released_this_frame(&self) -> u16 {
        !self.current & self.previous
    }

    /// Convert to Game Boy key state (8-bit).
    pub fn to_gb_keys(&self) -> u8 {
        let mut gb = 0u8;

        if self.is_pressed(Button::Right) { gb |= GbKey::Right.mask(); }
        if self.is_pressed(Button::Left) { gb |= GbKey::Left.mask(); }
        if self.is_pressed(Button::Up) { gb |= GbKey::Up.mask(); }
        if self.is_pressed(Button::Down) { gb |= GbKey::Down.mask(); }
        if self.is_pressed(Button::A) { gb |= GbKey::A.mask(); }
        if self.is_pressed(Button::B) { gb |= GbKey::B.mask(); }
        if self.is_pressed(Button::Select) { gb |= GbKey::Select.mask(); }
        if self.is_pressed(Button::Start) { gb |= GbKey::Start.mask(); }

        gb
    }

    /// Get Game Boy keys that were just pressed.
    pub fn gb_keys_just_pressed(&self) -> u8 {
        let mut gb = 0u8;

        if self.just_pressed(Button::Right) { gb |= GbKey::Right.mask(); }
        if self.just_pressed(Button::Left) { gb |= GbKey::Left.mask(); }
        if self.just_pressed(Button::Up) { gb |= GbKey::Up.mask(); }
        if self.just_pressed(Button::Down) { gb |= GbKey::Down.mask(); }
        if self.just_pressed(Button::A) { gb |= GbKey::A.mask(); }
        if self.just_pressed(Button::B) { gb |= GbKey::B.mask(); }
        if self.just_pressed(Button::Select) { gb |= GbKey::Select.mask(); }
        if self.just_pressed(Button::Start) { gb |= GbKey::Start.mask(); }

        gb
    }

    /// Get Game Boy keys that were just released.
    pub fn gb_keys_just_released(&self) -> u8 {
        let mut gb = 0u8;

        if self.just_released(Button::Right) { gb |= GbKey::Right.mask(); }
        if self.just_released(Button::Left) { gb |= GbKey::Left.mask(); }
        if self.just_released(Button::Up) { gb |= GbKey::Up.mask(); }
        if self.just_released(Button::Down) { gb |= GbKey::Down.mask(); }
        if self.just_released(Button::A) { gb |= GbKey::A.mask(); }
        if self.just_released(Button::B) { gb |= GbKey::B.mask(); }
        if self.just_released(Button::Select) { gb |= GbKey::Select.mask(); }
        if self.just_released(Button::Start) { gb |= GbKey::Start.mask(); }

        gb
    }
}

// ============================================================================
// Input Driver
// ============================================================================

/// GPi Case 2W input driver.
pub struct Input {
    state: InputState,
    initialized: bool,
}

impl Input {
    /// Create a new input driver (not yet initialized).
    pub const fn new() -> Self {
        Self {
            state: InputState::new(),
            initialized: false,
        }
    }

    /// Initialize GPIO pins for button input.
    pub fn init(&mut self) {
        let buttons = [
            // D-Pad
            dpad::UP, dpad::DOWN, dpad::LEFT, dpad::RIGHT,
            // Face buttons
            face::A, face::B, face::X, face::Y,
            // Menu buttons
            menu::START, menu::SELECT, menu::HOME,
            // Shoulder buttons
            shoulder::L, shoulder::R,
        ];

        for &pin in &buttons {
            gpio::set_function(pin, Function::Input);
            gpio::set_pull(pin, Pull::Up);
        }

        self.initialized = true;
    }

    /// Read current button state from GPIO.
    pub fn read(&self) -> u16 {
        if !self.initialized {
            return 0;
        }

        let mut state = 0u16;

        // D-Pad (active low)
        if !gpio::read(dpad::UP) { state |= Button::Up as u16; }
        if !gpio::read(dpad::DOWN) { state |= Button::Down as u16; }
        if !gpio::read(dpad::LEFT) { state |= Button::Left as u16; }
        if !gpio::read(dpad::RIGHT) { state |= Button::Right as u16; }

        // Face buttons
        if !gpio::read(face::A) { state |= Button::A as u16; }
        if !gpio::read(face::B) { state |= Button::B as u16; }
        if !gpio::read(face::X) { state |= Button::X as u16; }
        if !gpio::read(face::Y) { state |= Button::Y as u16; }

        // Menu buttons
        if !gpio::read(menu::START) { state |= Button::Start as u16; }
        if !gpio::read(menu::SELECT) { state |= Button::Select as u16; }
        if !gpio::read(menu::HOME) { state |= Button::Home as u16; }

        // Shoulder buttons
        if !gpio::read(shoulder::L) { state |= Button::L as u16; }
        if !gpio::read(shoulder::R) { state |= Button::R as u16; }

        state
    }

    /// Update input state (call once per frame).
    pub fn update(&mut self) {
        let buttons = self.read();
        self.state.update(buttons);
    }

    /// Get current input state.
    pub fn state(&self) -> &InputState {
        &self.state
    }

    /// Check if a button is currently pressed.
    pub fn is_pressed(&self, button: Button) -> bool {
        self.state.is_pressed(button)
    }

    /// Check if a button was just pressed this frame.
    pub fn just_pressed(&self, button: Button) -> bool {
        self.state.just_pressed(button)
    }

    /// Check if a button was just released this frame.
    pub fn just_released(&self, button: Button) -> bool {
        self.state.just_released(button)
    }

    /// Get Game Boy key state.
    pub fn gb_keys(&self) -> u8 {
        self.state.to_gb_keys()
    }
}

// ============================================================================
// Menu Actions
// ============================================================================

/// Input actions for ROM browser and menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    None,
    Up,
    Down,
    Left,
    Right,
    Select,  // A or Start
    Back,    // B
    PageUp,  // L
    PageDown, // R
    Home,    // Home button
}

impl Input {
    /// Get menu action from current input state.
    pub fn get_menu_action(&self) -> MenuAction {
        // Priority order: D-Pad, then action buttons
        if self.just_pressed(Button::Up) { return MenuAction::Up; }
        if self.just_pressed(Button::Down) { return MenuAction::Down; }
        if self.just_pressed(Button::Left) { return MenuAction::Left; }
        if self.just_pressed(Button::Right) { return MenuAction::Right; }
        if self.just_pressed(Button::A) { return MenuAction::Select; }
        if self.just_pressed(Button::Start) { return MenuAction::Select; }
        if self.just_pressed(Button::B) { return MenuAction::Back; }
        if self.just_pressed(Button::L) { return MenuAction::PageUp; }
        if self.just_pressed(Button::R) { return MenuAction::PageDown; }
        if self.just_pressed(Button::Home) { return MenuAction::Home; }

        MenuAction::None
    }
}

// ============================================================================
// Global Instance
// ============================================================================

static mut INPUT: Input = Input::new();

/// Initialize the global input driver.
pub fn init() {
    unsafe { INPUT.init(); }
}

/// Update global input state (call once per frame).
pub fn update() {
    unsafe { INPUT.update(); }
}

/// Get the global input driver.
pub fn get() -> &'static Input {
    unsafe { &INPUT }
}

/// Get mutable reference to global input driver.
pub fn get_mut() -> &'static mut Input {
    unsafe { &mut INPUT }
}
