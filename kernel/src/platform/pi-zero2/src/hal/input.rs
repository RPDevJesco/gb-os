//! Input Hardware Abstraction
//!
//! Abstracts the differences between:
//! - x86: PS/2 keyboard with scan codes
//! - ARM: GPIO buttons with direct pin reading

/// Game Boy buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GameBoyButton {
    Right = 0,
    Left = 1,
    Up = 2,
    Down = 3,
    A = 4,
    B = 5,
    Select = 6,
    Start = 7,
}

/// Extended buttons (for UI navigation)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendedButton {
    // Game Boy buttons
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
    // Extra buttons (GPi Case 2W has these)
    X,
    Y,
    L,
    R,
    Home,
    Turbo,
}

/// Button state flags (bitfield)
#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonState {
    /// Currently pressed buttons (bitfield)
    pub pressed: u16,
    /// Buttons that were just pressed this frame
    pub just_pressed: u16,
    /// Buttons that were just released this frame
    pub just_released: u16,
}

impl ButtonState {
    pub const RIGHT: u16 = 1 << 0;
    pub const LEFT: u16 = 1 << 1;
    pub const UP: u16 = 1 << 2;
    pub const DOWN: u16 = 1 << 3;
    pub const A: u16 = 1 << 4;
    pub const B: u16 = 1 << 5;
    pub const SELECT: u16 = 1 << 6;
    pub const START: u16 = 1 << 7;
    pub const X: u16 = 1 << 8;
    pub const Y: u16 = 1 << 9;
    pub const L: u16 = 1 << 10;
    pub const R: u16 = 1 << 11;
    pub const HOME: u16 = 1 << 12;
    pub const TURBO: u16 = 1 << 13;
    
    /// Check if a button is currently pressed
    #[inline]
    pub fn is_pressed(&self, button: u16) -> bool {
        (self.pressed & button) != 0
    }
    
    /// Check if a button was just pressed this frame
    #[inline]
    pub fn was_just_pressed(&self, button: u16) -> bool {
        (self.just_pressed & button) != 0
    }
    
    /// Check if a button was just released this frame
    #[inline]
    pub fn was_just_released(&self, button: u16) -> bool {
        (self.just_released & button) != 0
    }
    
    /// Get Game Boy keypad state (lower 8 bits)
    #[inline]
    pub fn gb_keypad(&self) -> u8 {
        (self.pressed & 0xFF) as u8
    }
    
    /// Convert to Game Boy keypad format for emulator
    /// Returns the keypad register value (active low, directly usable by keypad.rs)
    pub fn to_gb_keys(&self) -> u8 {
        let mut keys = 0u8;
        if self.is_pressed(Self::RIGHT)  { keys |= 0x01; }
        if self.is_pressed(Self::LEFT)   { keys |= 0x02; }
        if self.is_pressed(Self::UP)     { keys |= 0x04; }
        if self.is_pressed(Self::DOWN)   { keys |= 0x08; }
        if self.is_pressed(Self::A)      { keys |= 0x10; }
        if self.is_pressed(Self::B)      { keys |= 0x20; }
        if self.is_pressed(Self::SELECT) { keys |= 0x40; }
        if self.is_pressed(Self::START)  { keys |= 0x80; }
        keys
    }
}

/// Input device trait
pub trait InputDevice {
    /// Poll for new input (call once per frame)
    fn poll(&mut self) -> ButtonState;
    
    /// Get current button state without polling
    fn state(&self) -> ButtonState;
    
    /// Check if Home/Menu button was pressed (for returning to ROM browser)
    fn menu_requested(&self) -> bool;
}

/// Menu action for ROM browser navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    None,
    Up,
    Down,
    Left,
    Right,
    Select,    // A or Start
    Back,      // B
    PageUp,    // L
    PageDown,  // R
    Home,      // Home button
}

impl ButtonState {
    /// Convert button state to menu action (for ROM browser)
    pub fn to_menu_action(&self) -> MenuAction {
        // Priority order matters - check most specific first
        if self.was_just_pressed(Self::HOME) { return MenuAction::Home; }
        if self.was_just_pressed(Self::A) || self.was_just_pressed(Self::START) { 
            return MenuAction::Select; 
        }
        if self.was_just_pressed(Self::B) { return MenuAction::Back; }
        if self.was_just_pressed(Self::L) { return MenuAction::PageUp; }
        if self.was_just_pressed(Self::R) { return MenuAction::PageDown; }
        if self.was_just_pressed(Self::UP) { return MenuAction::Up; }
        if self.was_just_pressed(Self::DOWN) { return MenuAction::Down; }
        if self.was_just_pressed(Self::LEFT) { return MenuAction::Left; }
        if self.was_just_pressed(Self::RIGHT) { return MenuAction::Right; }
        MenuAction::None
    }
}
