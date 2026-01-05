//! GPIO Input Driver for RetroFlag GPi Case 2W
//!
//! The GPi Case 2W has the following controls connected via GPIO:
//! - D-Pad: Up, Down, Left, Right
//! - Action buttons: A, B, X, Y
//! - Shoulder buttons: L, R
//! - Menu buttons: Start, Select, Home (Hotkey), Turbo
//!
//! # GPIO Pin Assignments (Active Low)
//!
//! Based on common RetroFlag/Recalbox configurations.
//! The GPi Case 2W uses GPIO pins 22-27 for DPI display, so buttons
//! use the remaining available pins.
//!
//! | Button | GPIO | Physical Pin | Notes |
//! |--------|------|--------------|-------|
//! | Up     | 5    | 29           | D-Pad |
//! | Down   | 6    | 31           | D-Pad |
//! | Left   | 13   | 33           | D-Pad |
//! | Right  | 19   | 35           | D-Pad |
//! | A      | 26   | 37           | Face  |
//! | B      | 12   | 32           | Face  |
//! | X      | 20   | 38           | Face  |
//! | Y      | 16   | 36           | Face  |
//! | L      | 4    | 7            | Shoulder |
//! | R      | 17   | 11           | Shoulder |
//! | Start  | 22   | 15           | Menu (after DPI: 27) |
//! | Select | 23   | 16           | Menu |
//! | Home   | 24   | 18           | Hotkey |
//! | Turbo  | 25   | 22           | Extra |
//!
//! Note: GPIO 0-21 are used for DPI display output.
//! Pin assignments may vary between GPi Case revisions.

#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// Hardware Constants
// ============================================================================

/// Peripheral base address for BCM2710 (Pi Zero 2W)
const PERIPHERAL_BASE: usize = 0x3F00_0000;

/// GPIO register base address
const GPIO_BASE: usize = PERIPHERAL_BASE + 0x0020_0000;

/// GPIO Function Select registers (3 bits per pin, 10 pins per register)
mod gpio_regs {
    use super::GPIO_BASE;
    
    pub const GPFSEL0: usize = GPIO_BASE + 0x00;  // GPIO 0-9
    pub const GPFSEL1: usize = GPIO_BASE + 0x04;  // GPIO 10-19
    pub const GPFSEL2: usize = GPIO_BASE + 0x08;  // GPIO 20-29
    pub const GPFSEL3: usize = GPIO_BASE + 0x0C;  // GPIO 30-39
    pub const GPFSEL4: usize = GPIO_BASE + 0x10;  // GPIO 40-49
    pub const GPFSEL5: usize = GPIO_BASE + 0x14;  // GPIO 50-53
    
    /// Pin Level registers (read current state)
    pub const GPLEV0: usize = GPIO_BASE + 0x34;   // GPIO 0-31
    pub const GPLEV1: usize = GPIO_BASE + 0x38;   // GPIO 32-53
    
    /// Pin Event Detect Status
    pub const GPEDS0: usize = GPIO_BASE + 0x40;
    pub const GPEDS1: usize = GPIO_BASE + 0x44;
    
    /// Rising Edge Detect Enable
    pub const GPREN0: usize = GPIO_BASE + 0x4C;
    pub const GPREN1: usize = GPIO_BASE + 0x50;
    
    /// Falling Edge Detect Enable
    pub const GPFEN0: usize = GPIO_BASE + 0x58;
    pub const GPFEN1: usize = GPIO_BASE + 0x5C;
    
    /// Pull-up/down Enable
    pub const GPPUD: usize = GPIO_BASE + 0x94;
    /// Pull-up/down Clock registers
    pub const GPPUDCLK0: usize = GPIO_BASE + 0x98;
    pub const GPPUDCLK1: usize = GPIO_BASE + 0x9C;
}

/// GPIO function codes (3 bits)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioFunction {
    Input  = 0b000,
    Output = 0b001,
    Alt0   = 0b100,
    Alt1   = 0b101,
    Alt2   = 0b110,
    Alt3   = 0b111,
    Alt4   = 0b011,
    Alt5   = 0b010,
}

/// Pull-up/down configuration
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioPull {
    None = 0b00,
    Down = 0b01,
    Up   = 0b10,
}

// ============================================================================
// Button Definitions
// ============================================================================

/// GPi Case 2W button identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GpiButton {
    Up     = 0,
    Down   = 1,
    Left   = 2,
    Right  = 3,
    A      = 4,
    B      = 5,
    X      = 6,
    Y      = 7,
    L      = 8,
    R      = 9,
    Start  = 10,
    Select = 11,
    Home   = 12,  // Hotkey
    Turbo  = 13,  // Plus/Minus equivalent
}

impl GpiButton {
    /// Total number of buttons
    pub const COUNT: usize = 14;
    
    /// Get all buttons as array for iteration
    pub const ALL: [GpiButton; 14] = [
        GpiButton::Up, GpiButton::Down, GpiButton::Left, GpiButton::Right,
        GpiButton::A, GpiButton::B, GpiButton::X, GpiButton::Y,
        GpiButton::L, GpiButton::R,
        GpiButton::Start, GpiButton::Select, GpiButton::Home, GpiButton::Turbo,
    ];
}

/// GPIO pin assignments for each button
/// These can be configured at runtime if needed
#[derive(Debug, Clone, Copy)]
pub struct GpiPinConfig {
    pub up:     u8,
    pub down:   u8,
    pub left:   u8,
    pub right:  u8,
    pub a:      u8,
    pub b:      u8,
    pub x:      u8,
    pub y:      u8,
    pub l:      u8,
    pub r:      u8,
    pub start:  u8,
    pub select: u8,
    pub home:   u8,
    pub turbo:  u8,
}

impl GpiPinConfig {
    /// Default pin configuration for GPi Case 2W
    /// Note: These are estimates based on common configurations.
    /// Actual pins depend on GPi Case 2W hardware revision.
    pub const fn default_gpi_case_2w() -> Self {
        Self {
            // D-Pad (directly accessible GPIOs, not on DPI)
            up:     5,
            down:   6,
            left:   13,
            right:  19,
            
            // Face buttons
            a:      26,
            b:      12,
            x:      20,
            y:      16,
            
            // Shoulders
            l:      4,
            r:      17,
            
            // Menu buttons (GPIOs 22-27 used by DPI, so these use 22-25 which
            // overlap with some DPI pins - this is a simplified mapping)
            // In practice, GPi Case 2W may use I2C GPIO expander or different pins
            start:  27,  // Or via GPIO expander
            select: 23,
            home:   24,
            turbo:  25,
        }
    }
    
    /// Alternative configuration using mk_arcade_joystick standard pinout
    /// GPIO: 4,17,27,22,10,9,25,24,23,18,15,14 for UP,DN,LT,RT,ST,SE,A,B,TR,Y,X,TL
    pub const fn mk_arcade_standard() -> Self {
        Self {
            up:     4,
            down:   17,
            left:   27,
            right:  22,
            start:  10,
            select: 9,
            a:      25,
            b:      24,
            x:      15,
            y:      18,
            l:      14,  // TL
            r:      23,  // TR
            home:   11,  // Hotkey (if available)
            turbo:  8,   // Extra (if available)
        }
    }
    
    /// Get GPIO pin for a button
    pub fn pin_for(&self, button: GpiButton) -> u8 {
        match button {
            GpiButton::Up     => self.up,
            GpiButton::Down   => self.down,
            GpiButton::Left   => self.left,
            GpiButton::Right  => self.right,
            GpiButton::A      => self.a,
            GpiButton::B      => self.b,
            GpiButton::X      => self.x,
            GpiButton::Y      => self.y,
            GpiButton::L      => self.l,
            GpiButton::R      => self.r,
            GpiButton::Start  => self.start,
            GpiButton::Select => self.select,
            GpiButton::Home   => self.home,
            GpiButton::Turbo  => self.turbo,
        }
    }
}

// ============================================================================
// Input State
// ============================================================================

/// Current state of all buttons (bitfield)
#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonState {
    /// Bitmask of currently pressed buttons
    pressed: u16,
    /// Bitmask of buttons pressed this frame (rising edge)
    just_pressed: u16,
    /// Bitmask of buttons released this frame (falling edge)
    just_released: u16,
    /// Previous frame's pressed state
    previous: u16,
}

impl ButtonState {
    pub const fn new() -> Self {
        Self {
            pressed: 0,
            just_pressed: 0,
            just_released: 0,
            previous: 0,
        }
    }
    
    /// Check if a button is currently pressed
    #[inline]
    pub fn is_pressed(&self, button: GpiButton) -> bool {
        (self.pressed & (1 << button as u8)) != 0
    }
    
    /// Check if a button was just pressed this frame
    #[inline]
    pub fn just_pressed(&self, button: GpiButton) -> bool {
        (self.just_pressed & (1 << button as u8)) != 0
    }
    
    /// Check if a button was just released this frame
    #[inline]
    pub fn just_released(&self, button: GpiButton) -> bool {
        (self.just_released & (1 << button as u8)) != 0
    }
    
    /// Get raw pressed bitmask
    #[inline]
    pub fn raw(&self) -> u16 {
        self.pressed
    }
    
    /// Check if any button is pressed
    #[inline]
    pub fn any_pressed(&self) -> bool {
        self.pressed != 0
    }
    
    /// Check if D-pad is pressed in any direction
    #[inline]
    pub fn dpad_pressed(&self) -> bool {
        const DPAD_MASK: u16 = (1 << GpiButton::Up as u8) 
                             | (1 << GpiButton::Down as u8)
                             | (1 << GpiButton::Left as u8)
                             | (1 << GpiButton::Right as u8);
        (self.pressed & DPAD_MASK) != 0
    }
}

// ============================================================================
// GPIO Input Driver
// ============================================================================

/// GPIO input driver for GPi Case 2W
pub struct GpiInput {
    /// Pin configuration
    config: GpiPinConfig,
    /// Current button state
    state: ButtonState,
    /// Debounce counters for each button (frames held)
    debounce: [u8; GpiButton::COUNT],
    /// Initialized flag
    initialized: bool,
}

impl GpiInput {
    /// Create new input driver with default configuration
    pub const fn new() -> Self {
        Self {
            config: GpiPinConfig::default_gpi_case_2w(),
            state: ButtonState::new(),
            debounce: [0; GpiButton::COUNT],
            initialized: false,
        }
    }
    
    /// Create with custom pin configuration
    pub const fn with_config(config: GpiPinConfig) -> Self {
        Self {
            config,
            state: ButtonState::new(),
            debounce: [0; GpiButton::COUNT],
            initialized: false,
        }
    }
    
    /// Initialize GPIO pins for input with pull-ups
    pub fn init(&mut self) {
        // Configure each button pin as input with pull-up
        for button in GpiButton::ALL {
            let pin = self.config.pin_for(button);
            
            // Skip pins that might conflict with DPI (0-21)
            // In production, would check actual hardware configuration
            if pin < 22 || pin > 27 {
                self.set_gpio_function(pin, GpioFunction::Input);
                self.set_gpio_pull(pin, GpioPull::Up);
            }
        }
        
        self.initialized = true;
    }
    
    /// Poll all button states
    /// Call this once per frame
    pub fn poll(&mut self) {
        if !self.initialized {
            return;
        }
        
        // Save previous state
        self.state.previous = self.state.pressed;
        
        // Read all GPIO levels at once
        let gpio_level = self.read_gpio_levels();
        
        // Check each button
        let mut new_pressed: u16 = 0;
        
        for button in GpiButton::ALL {
            let pin = self.config.pin_for(button);
            
            // Buttons are active LOW (pressed = 0, released = 1)
            let bit_set = (gpio_level & (1 << pin)) == 0;
            
            if bit_set {
                new_pressed |= 1 << button as u8;
                
                // Update debounce counter
                let idx = button as usize;
                if self.debounce[idx] < 255 {
                    self.debounce[idx] += 1;
                }
            } else {
                self.debounce[button as usize] = 0;
            }
        }
        
        // Apply debounce (require 2 frames of consistent state)
        const DEBOUNCE_FRAMES: u8 = 2;
        let mut debounced: u16 = 0;
        for button in GpiButton::ALL {
            if self.debounce[button as usize] >= DEBOUNCE_FRAMES {
                debounced |= 1 << button as u8;
            }
        }
        
        self.state.pressed = debounced;
        
        // Calculate edge detection
        self.state.just_pressed = debounced & !self.state.previous;
        self.state.just_released = !debounced & self.state.previous;
    }
    
    /// Get current button state
    #[inline]
    pub fn state(&self) -> &ButtonState {
        &self.state
    }
    
    /// Check if a button is pressed (convenience method)
    #[inline]
    pub fn is_pressed(&self, button: GpiButton) -> bool {
        self.state.is_pressed(button)
    }
    
    /// Check if a button was just pressed this frame
    #[inline]
    pub fn just_pressed(&self, button: GpiButton) -> bool {
        self.state.just_pressed(button)
    }
    
    // ========================================================================
    // Low-level GPIO access
    // ========================================================================
    
    /// Read GPIO level register (all 32 lower pins)
    fn read_gpio_levels(&self) -> u32 {
        unsafe { read_volatile(gpio_regs::GPLEV0 as *const u32) }
    }
    
    /// Set GPIO function for a pin
    fn set_gpio_function(&self, pin: u8, func: GpioFunction) {
        if pin >= 54 {
            return;
        }
        
        let reg = match pin / 10 {
            0 => gpio_regs::GPFSEL0,
            1 => gpio_regs::GPFSEL1,
            2 => gpio_regs::GPFSEL2,
            3 => gpio_regs::GPFSEL3,
            4 => gpio_regs::GPFSEL4,
            5 => gpio_regs::GPFSEL5,
            _ => return,
        };
        
        let shift = (pin % 10) * 3;
        let mask = 0b111 << shift;
        let value = (func as u32) << shift;
        
        unsafe {
            let current = read_volatile(reg as *const u32);
            write_volatile(reg as *mut u32, (current & !mask) | value);
        }
    }
    
    /// Set pull-up/down for a pin
    fn set_gpio_pull(&self, pin: u8, pull: GpioPull) {
        if pin >= 54 {
            return;
        }
        
        // BCM2835/BCM2710 pull sequence:
        // 1. Write to GPPUD to set control signal
        // 2. Wait 150 cycles
        // 3. Write to GPPUDCLK0/1 to clock signal into pins
        // 4. Wait 150 cycles
        // 5. Write to GPPUD to remove control signal
        // 6. Write to GPPUDCLK0/1 to remove clock
        
        let clk_reg = if pin < 32 { gpio_regs::GPPUDCLK0 } else { gpio_regs::GPPUDCLK1 };
        let clk_bit = 1u32 << (pin % 32);
        
        unsafe {
            // Set pull direction
            write_volatile(gpio_regs::GPPUD as *mut u32, pull as u32);
            
            // Wait ~150 cycles
            for _ in 0..150 {
                core::hint::spin_loop();
            }
            
            // Clock into specific pin
            write_volatile(clk_reg as *mut u32, clk_bit);
            
            // Wait ~150 cycles
            for _ in 0..150 {
                core::hint::spin_loop();
            }
            
            // Clear control signal
            write_volatile(gpio_regs::GPPUD as *mut u32, 0);
            write_volatile(clk_reg as *mut u32, 0);
        }
    }
}

// ============================================================================
// Global Instance
// ============================================================================

/// Global input driver instance
static mut GPI_INPUT: GpiInput = GpiInput::new();

/// Get global input driver
///
/// # Safety
/// Not thread-safe. Only call from single core during main loop.
pub fn get_input() -> &'static mut GpiInput {
    unsafe { &mut *core::ptr::addr_of_mut!(GPI_INPUT) }
}

/// Initialize input system
pub fn init() {
    get_input().init();
}

/// Poll input (call once per frame)
pub fn poll() {
    get_input().poll();
}

/// Check if button is pressed
pub fn is_pressed(button: GpiButton) -> bool {
    get_input().is_pressed(button)
}

/// Check if button was just pressed this frame
pub fn just_pressed(button: GpiButton) -> bool {
    get_input().just_pressed(button)
}

// ============================================================================
// GameBoy Keypad Mapping
// ============================================================================

/// Map GPi Case buttons to GameBoy keypad
/// 
/// GPi Case 2W → GameBoy mapping:
/// - D-Pad: Direct mapping (Up/Down/Left/Right)
/// - A → A
/// - B → B  
/// - Start → Start
/// - Select → Select
/// - X/Y/L/R → Not used by GameBoy (can be used for hotkeys)
pub mod gameboy_mapping {
    use super::{GpiButton, ButtonState};
    
    /// GameBoy keypad bits (matches gameboy/keypad.rs)
    #[repr(u8)]
    pub enum GbKey {
        Right  = 0,
        Left   = 1,
        Up     = 2,
        Down   = 3,
        A      = 4,
        B      = 5,
        Select = 6,
        Start  = 7,
    }
    
    /// Convert GPi button state to GameBoy keypad state
    pub fn to_gb_keypad(state: &ButtonState) -> u8 {
        let mut gb_keys: u8 = 0;
        
        if state.is_pressed(GpiButton::Right)  { gb_keys |= 1 << GbKey::Right as u8; }
        if state.is_pressed(GpiButton::Left)   { gb_keys |= 1 << GbKey::Left as u8; }
        if state.is_pressed(GpiButton::Up)     { gb_keys |= 1 << GbKey::Up as u8; }
        if state.is_pressed(GpiButton::Down)   { gb_keys |= 1 << GbKey::Down as u8; }
        if state.is_pressed(GpiButton::A)      { gb_keys |= 1 << GbKey::A as u8; }
        if state.is_pressed(GpiButton::B)      { gb_keys |= 1 << GbKey::B as u8; }
        if state.is_pressed(GpiButton::Select) { gb_keys |= 1 << GbKey::Select as u8; }
        if state.is_pressed(GpiButton::Start)  { gb_keys |= 1 << GbKey::Start as u8; }
        
        gb_keys
    }
    
    /// Check for just-pressed GameBoy keys
    pub fn gb_keys_just_pressed(state: &ButtonState) -> u8 {
        let mut gb_keys: u8 = 0;
        
        if state.just_pressed(GpiButton::Right)  { gb_keys |= 1 << GbKey::Right as u8; }
        if state.just_pressed(GpiButton::Left)   { gb_keys |= 1 << GbKey::Left as u8; }
        if state.just_pressed(GpiButton::Up)     { gb_keys |= 1 << GbKey::Up as u8; }
        if state.just_pressed(GpiButton::Down)   { gb_keys |= 1 << GbKey::Down as u8; }
        if state.just_pressed(GpiButton::A)      { gb_keys |= 1 << GbKey::A as u8; }
        if state.just_pressed(GpiButton::B)      { gb_keys |= 1 << GbKey::B as u8; }
        if state.just_pressed(GpiButton::Select) { gb_keys |= 1 << GbKey::Select as u8; }
        if state.just_pressed(GpiButton::Start)  { gb_keys |= 1 << GbKey::Start as u8; }
        
        gb_keys
    }
}

// ============================================================================
// ROM Browser Input Handling
// ============================================================================

/// Input actions for ROM browser menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    None,
    Up,
    Down,
    Left,
    Right,
    Select,    // A or Start to select
    Back,      // B to go back
    PageUp,    // L shoulder
    PageDown,  // R shoulder
    Home,      // Home button - exit/menu
}

/// Get menu action from current input state
pub fn get_menu_action(state: &ButtonState) -> MenuAction {
    // Priority: directional, then action buttons
    if state.just_pressed(GpiButton::Up)     { return MenuAction::Up; }
    if state.just_pressed(GpiButton::Down)   { return MenuAction::Down; }
    if state.just_pressed(GpiButton::Left)   { return MenuAction::Left; }
    if state.just_pressed(GpiButton::Right)  { return MenuAction::Right; }
    if state.just_pressed(GpiButton::A)      { return MenuAction::Select; }
    if state.just_pressed(GpiButton::Start)  { return MenuAction::Select; }
    if state.just_pressed(GpiButton::B)      { return MenuAction::Back; }
    if state.just_pressed(GpiButton::L)      { return MenuAction::PageUp; }
    if state.just_pressed(GpiButton::R)      { return MenuAction::PageDown; }
    if state.just_pressed(GpiButton::Home)   { return MenuAction::Home; }
    
    MenuAction::None
}
