//! PS/2 Keyboard Driver - Armada E500/V300 Enhanced
//!
//! Handles PS/2 keyboard input through the MSIO Super I/O Controller.
//!
//! Hardware Details (from Armada E500 Technical Reference Guide):
//! - SMC FDC37N97X "Tikki" MSIO handles all keyboard functions
//! - Internal 8051 microcontroller processes keyboard matrix (16x8 = 128 keys)
//! - Host access via standard ports 60h (data) and 64h (status/command)
//! - KEYBOARD LEDs ARE CONTROLLED VIA GPIO, NOT STANDARD PS/2 COMMANDS!
//!   - GPIO16 = CAPS_LED#_3 (Caps Lock)
//!   - GPIO20 = NUM_LED#_3 (Num Lock)
//!   - GPIO21 = SCROLL_LED#_3 (Scroll Lock)
//! - GPIO access requires MSIO mailbox register communication
//!
//! Hotkey Support:
//! - Fn+key combinations generate SMI interrupts
//! - Handled by MSIO 8051 firmware, not visible to host
//! - Fn+F4 = LCD/CRT switch (handled by MSIO → ATI Rage)
//!
//! Note: Standard PS/2 LED commands (0xED) may not work on this hardware!
//! Use set_led_via_msio() for reliable LED control.

use crate::arch::x86::io::{inb, outb};

// =============================================================================
// Hardware Constants (from Tech Ref)
// =============================================================================

/// PS/2 keyboard data port (8042 compatible via MSIO)
const PS2_DATA_PORT: u16 = 0x60;

/// PS/2 keyboard status/command port
const PS2_STATUS_PORT: u16 = 0x64;
const PS2_COMMAND_PORT: u16 = 0x64;

/// Status register bits
mod status {
    pub const OUTPUT_BUFFER_FULL: u8 = 0x01;  // Data available in port 60h
    pub const INPUT_BUFFER_FULL: u8 = 0x02;   // Controller busy
    pub const SYSTEM_FLAG: u8 = 0x04;
    pub const COMMAND_DATA: u8 = 0x08;        // 0=data, 1=command
    pub const TIMEOUT_ERROR: u8 = 0x40;
    pub const PARITY_ERROR: u8 = 0x80;
}

/// PS/2 commands
mod ps2_cmd {
    pub const SET_LEDS: u8 = 0xED;            // May not work on MSIO!
    pub const ECHO: u8 = 0xEE;
    pub const SCAN_CODE_SET: u8 = 0xF0;
    pub const IDENTIFY: u8 = 0xF2;
    pub const SET_TYPEMATIC: u8 = 0xF3;
    pub const ENABLE_SCANNING: u8 = 0xF4;
    pub const DISABLE_SCANNING: u8 = 0xF5;
    pub const SET_DEFAULTS: u8 = 0xF6;
    pub const RESEND: u8 = 0xFE;
    pub const RESET: u8 = 0xFF;
}

/// PS/2 response codes
mod ps2_resp {
    pub const ACK: u8 = 0xFA;
    pub const RESEND: u8 = 0xFE;
    pub const ECHO: u8 = 0xEE;
    pub const BAT_OK: u8 = 0xAA;
    pub const BAT_FAIL: u8 = 0xFC;
}

/// LED bit flags (for both standard PS/2 and MSIO GPIO)
pub mod led {
    pub const SCROLL_LOCK: u8 = 0x01;
    pub const NUM_LOCK: u8 = 0x02;
    pub const CAPS_LOCK: u8 = 0x04;
}

// =============================================================================
// MSIO Mailbox Registers (for GPIO LED control)
// =============================================================================

/// MSIO uses port 66h/68h for ACPI Embedded Controller interface
/// which provides indirect access to GPIOs
mod msio {
    /// EC command port
    pub const EC_CMD_PORT: u16 = 0x66;
    /// EC data port
    pub const EC_DATA_PORT: u16 = 0x62;

    /// EC status bits
    pub const EC_OBF: u8 = 0x01;  // Output buffer full
    pub const EC_IBF: u8 = 0x02;  // Input buffer full

    /// EC commands
    pub const EC_READ_CMD: u8 = 0x80;
    pub const EC_WRITE_CMD: u8 = 0x81;

    /// GPIO register offsets in EC space (approximate - may need adjustment)
    pub const GPIO_OUT_REG: u8 = 0x40;  // Output register base

    /// LED GPIO bit positions
    pub const GPIO16_CAPS: u8 = 16;
    pub const GPIO20_NUM: u8 = 20;
    pub const GPIO21_SCROLL: u8 = 21;
}

// =============================================================================
// Key Codes
// =============================================================================

/// Key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyCode {
    Escape = 0x01,
    Key1 = 0x02, Key2 = 0x03, Key3 = 0x04, Key4 = 0x05, Key5 = 0x06,
    Key6 = 0x07, Key7 = 0x08, Key8 = 0x09, Key9 = 0x0A, Key0 = 0x0B,
    Minus = 0x0C, Equals = 0x0D,
    Backspace = 0x0E,
    Tab = 0x0F,
    Q = 0x10, W = 0x11, E = 0x12, R = 0x13, T = 0x14,
    Y = 0x15, U = 0x16, I = 0x17, O = 0x18, P = 0x19,
    LeftBracket = 0x1A, RightBracket = 0x1B,
    Enter = 0x1C,
    LeftCtrl = 0x1D,
    A = 0x1E, S = 0x1F, D = 0x20, F = 0x21, G = 0x22,
    H = 0x23, J = 0x24, K = 0x25, L = 0x26,
    Semicolon = 0x27, Quote = 0x28, Backtick = 0x29,
    LeftShift = 0x2A, Backslash = 0x2B,
    Z = 0x2C, X = 0x2D, C = 0x2E, V = 0x2F, B = 0x30,
    N = 0x31, M = 0x32,
    Comma = 0x33, Period = 0x34, Slash = 0x35,
    RightShift = 0x36,
    KeypadAsterisk = 0x37,
    LeftAlt = 0x38,
    Space = 0x39,
    CapsLock = 0x3A,
    F1 = 0x3B, F2 = 0x3C, F3 = 0x3D, F4 = 0x3E, F5 = 0x3F,
    F6 = 0x40, F7 = 0x41, F8 = 0x42, F9 = 0x43, F10 = 0x44,
    NumLock = 0x45,
    ScrollLock = 0x46,
    Keypad7 = 0x47, Keypad8 = 0x48, Keypad9 = 0x49,
    KeypadMinus = 0x4A,
    Keypad4 = 0x4B, Keypad5 = 0x4C, Keypad6 = 0x4D,
    KeypadPlus = 0x4E,
    Keypad1 = 0x4F, Keypad2 = 0x50, Keypad3 = 0x51,
    Keypad0 = 0x52, KeypadDot = 0x53,
    F11 = 0x57, F12 = 0x58,
    // Extended keys (after E0 prefix)
    Up = 0x80,        // E0 48
    Left = 0x81,      // E0 4B
    Right = 0x82,     // E0 4D
    Down = 0x83,      // E0 50
    Insert = 0x84,    // E0 52
    Delete = 0x85,    // E0 53
    Home = 0x86,      // E0 47
    End = 0x87,       // E0 4F
    PageUp = 0x88,    // E0 49
    PageDown = 0x89,  // E0 51
    RightCtrl = 0x8A, // E0 1D
    RightAlt = 0x8B,  // E0 38
    // Fn key (MSIO-specific, generates SMI)
    Fn = 0x8C,
    Unknown = 0xFF,
}

impl KeyCode {
    /// Convert scancode to KeyCode (handles extended scancodes)
    pub fn from_scancode(scancode: u8, extended: bool) -> Self {
        let code = scancode & 0x7F;

        if extended {
            match code {
                0x48 => Self::Up,
                0x4B => Self::Left,
                0x4D => Self::Right,
                0x50 => Self::Down,
                0x52 => Self::Insert,
                0x53 => Self::Delete,
                0x47 => Self::Home,
                0x4F => Self::End,
                0x49 => Self::PageUp,
                0x51 => Self::PageDown,
                0x1D => Self::RightCtrl,
                0x38 => Self::RightAlt,
                _ => Self::Unknown,
            }
        } else {
            match code {
                0x01 => Self::Escape,
                0x02 => Self::Key1, 0x03 => Self::Key2, 0x04 => Self::Key3,
                0x05 => Self::Key4, 0x06 => Self::Key5, 0x07 => Self::Key6,
                0x08 => Self::Key7, 0x09 => Self::Key8, 0x0A => Self::Key9,
                0x0B => Self::Key0, 0x0C => Self::Minus, 0x0D => Self::Equals,
                0x0E => Self::Backspace, 0x0F => Self::Tab,
                0x10 => Self::Q, 0x11 => Self::W, 0x12 => Self::E,
                0x13 => Self::R, 0x14 => Self::T, 0x15 => Self::Y,
                0x16 => Self::U, 0x17 => Self::I, 0x18 => Self::O,
                0x19 => Self::P, 0x1A => Self::LeftBracket, 0x1B => Self::RightBracket,
                0x1C => Self::Enter, 0x1D => Self::LeftCtrl,
                0x1E => Self::A, 0x1F => Self::S, 0x20 => Self::D,
                0x21 => Self::F, 0x22 => Self::G, 0x23 => Self::H,
                0x24 => Self::J, 0x25 => Self::K, 0x26 => Self::L,
                0x27 => Self::Semicolon, 0x28 => Self::Quote, 0x29 => Self::Backtick,
                0x2A => Self::LeftShift, 0x2B => Self::Backslash,
                0x2C => Self::Z, 0x2D => Self::X, 0x2E => Self::C,
                0x2F => Self::V, 0x30 => Self::B, 0x31 => Self::N,
                0x32 => Self::M, 0x33 => Self::Comma, 0x34 => Self::Period,
                0x35 => Self::Slash, 0x36 => Self::RightShift,
                0x37 => Self::KeypadAsterisk, 0x38 => Self::LeftAlt,
                0x39 => Self::Space, 0x3A => Self::CapsLock,
                0x3B => Self::F1, 0x3C => Self::F2, 0x3D => Self::F3,
                0x3E => Self::F4, 0x3F => Self::F5, 0x40 => Self::F6,
                0x41 => Self::F7, 0x42 => Self::F8, 0x43 => Self::F9,
                0x44 => Self::F10, 0x45 => Self::NumLock, 0x46 => Self::ScrollLock,
                0x47 => Self::Keypad7, 0x48 => Self::Keypad8, 0x49 => Self::Keypad9,
                0x4A => Self::KeypadMinus, 0x4B => Self::Keypad4, 0x4C => Self::Keypad5,
                0x4D => Self::Keypad6, 0x4E => Self::KeypadPlus,
                0x4F => Self::Keypad1, 0x50 => Self::Keypad2, 0x51 => Self::Keypad3,
                0x52 => Self::Keypad0, 0x53 => Self::KeypadDot,
                0x57 => Self::F11, 0x58 => Self::F12,
                _ => Self::Unknown,
            }
        }
    }

    /// Convert to ASCII character (if printable)
    pub fn to_ascii(&self, shift: bool) -> Option<char> {
        let c = match self {
            Self::Key1 => if shift { '!' } else { '1' },
            Self::Key2 => if shift { '@' } else { '2' },
            Self::Key3 => if shift { '#' } else { '3' },
            Self::Key4 => if shift { '$' } else { '4' },
            Self::Key5 => if shift { '%' } else { '5' },
            Self::Key6 => if shift { '^' } else { '6' },
            Self::Key7 => if shift { '&' } else { '7' },
            Self::Key8 => if shift { '*' } else { '8' },
            Self::Key9 => if shift { '(' } else { '9' },
            Self::Key0 => if shift { ')' } else { '0' },
            Self::Minus => if shift { '_' } else { '-' },
            Self::Equals => if shift { '+' } else { '=' },
            Self::Q => if shift { 'Q' } else { 'q' },
            Self::W => if shift { 'W' } else { 'w' },
            Self::E => if shift { 'E' } else { 'e' },
            Self::R => if shift { 'R' } else { 'r' },
            Self::T => if shift { 'T' } else { 't' },
            Self::Y => if shift { 'Y' } else { 'y' },
            Self::U => if shift { 'U' } else { 'u' },
            Self::I => if shift { 'I' } else { 'i' },
            Self::O => if shift { 'O' } else { 'o' },
            Self::P => if shift { 'P' } else { 'p' },
            Self::LeftBracket => if shift { '{' } else { '[' },
            Self::RightBracket => if shift { '}' } else { ']' },
            Self::A => if shift { 'A' } else { 'a' },
            Self::S => if shift { 'S' } else { 's' },
            Self::D => if shift { 'D' } else { 'd' },
            Self::F => if shift { 'F' } else { 'f' },
            Self::G => if shift { 'G' } else { 'g' },
            Self::H => if shift { 'H' } else { 'h' },
            Self::J => if shift { 'J' } else { 'j' },
            Self::K => if shift { 'K' } else { 'k' },
            Self::L => if shift { 'L' } else { 'l' },
            Self::Semicolon => if shift { ':' } else { ';' },
            Self::Quote => if shift { '"' } else { '\'' },
            Self::Backtick => if shift { '~' } else { '`' },
            Self::Backslash => if shift { '|' } else { '\\' },
            Self::Z => if shift { 'Z' } else { 'z' },
            Self::X => if shift { 'X' } else { 'x' },
            Self::C => if shift { 'C' } else { 'c' },
            Self::V => if shift { 'V' } else { 'v' },
            Self::B => if shift { 'B' } else { 'b' },
            Self::N => if shift { 'N' } else { 'n' },
            Self::M => if shift { 'M' } else { 'm' },
            Self::Comma => if shift { '<' } else { ',' },
            Self::Period => if shift { '>' } else { '.' },
            Self::Slash => if shift { '?' } else { '/' },
            Self::Space => ' ',
            Self::Tab => '\t',
            Self::Enter => '\n',
            _ => return None,
        };
        Some(c)
    }
}

// =============================================================================
// Key Buffer
// =============================================================================

const KEY_BUFFER_SIZE: usize = 32;

/// Buffered key event
#[derive(Clone, Copy)]
pub struct BufferedKey {
    pub keycode: KeyCode,
    pub ascii: Option<char>,
    pub pressed: bool,
}

/// Key event types
#[derive(Debug, Clone, Copy)]
pub enum KeyEvent {
    Press(KeyCode),
    Release(KeyCode),
}

// =============================================================================
// Keyboard Driver
// =============================================================================

/// Keyboard state with event buffer
pub struct Keyboard {
    // Modifier state
    shift_pressed: bool,
    ctrl_pressed: bool,
    alt_pressed: bool,
    caps_lock: bool,
    num_lock: bool,
    scroll_lock: bool,

    // Scancode state
    extended: bool,

    // Ring buffer
    buffer: [Option<BufferedKey>; KEY_BUFFER_SIZE],
    write_idx: usize,
    read_idx: usize,

    // LED state
    led_state: u8,

    // Use MSIO for LEDs (recommended for Armada)
    use_msio_leds: bool,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            shift_pressed: false,
            ctrl_pressed: false,
            alt_pressed: false,
            caps_lock: false,
            num_lock: false,
            scroll_lock: false,
            extended: false,
            buffer: [None; KEY_BUFFER_SIZE],
            write_idx: 0,
            read_idx: 0,
            led_state: 0,
            use_msio_leds: true,  // Default to MSIO for Armada
        }
    }

    /// Process scancode from IRQ handler
    pub fn process_scancode(&mut self, scancode: u8) -> Option<KeyEvent> {
        // Handle extended scancode prefix
        if scancode == 0xE0 {
            self.extended = true;
            return None;
        }

        // Skip E1 prefix (Pause key)
        if scancode == 0xE1 {
            return None;
        }

        let released = (scancode & 0x80) != 0;
        let keycode = KeyCode::from_scancode(scancode, self.extended);
        self.extended = false;

        // Update modifier state
        match keycode {
            KeyCode::LeftShift | KeyCode::RightShift => {
                self.shift_pressed = !released;
            }
            KeyCode::LeftCtrl | KeyCode::RightCtrl => {
                self.ctrl_pressed = !released;
            }
            KeyCode::LeftAlt | KeyCode::RightAlt => {
                self.alt_pressed = !released;
            }
            KeyCode::CapsLock if !released => {
                self.caps_lock = !self.caps_lock;
                self.update_leds();
            }
            KeyCode::NumLock if !released => {
                self.num_lock = !self.num_lock;
                self.update_leds();
            }
            KeyCode::ScrollLock if !released => {
                self.scroll_lock = !self.scroll_lock;
                self.update_leds();
            }
            _ => {}
        }

        // Calculate ASCII (with caps lock effect)
        let shift_effective = self.shift_pressed ^ self.caps_lock;
        let ascii = keycode.to_ascii(shift_effective);

        // Buffer the event
        let event = BufferedKey {
            keycode,
            ascii,
            pressed: !released,
        };

        let next_write = (self.write_idx + 1) % KEY_BUFFER_SIZE;
        if next_write != self.read_idx {
            self.buffer[self.write_idx] = Some(event);
            self.write_idx = next_write;
        }

        if released {
            Some(KeyEvent::Release(keycode))
        } else {
            Some(KeyEvent::Press(keycode))
        }
    }

    /// Get next buffered key event
    pub fn get_key(&mut self) -> Option<BufferedKey> {
        if self.read_idx == self.write_idx {
            return None;
        }
        let key = self.buffer[self.read_idx].take();
        self.read_idx = (self.read_idx + 1) % KEY_BUFFER_SIZE;
        key
    }

    /// Check if key is currently pressed
    pub fn is_shift_pressed(&self) -> bool { self.shift_pressed }
    pub fn is_ctrl_pressed(&self) -> bool { self.ctrl_pressed }
    pub fn is_alt_pressed(&self) -> bool { self.alt_pressed }
    pub fn is_caps_lock(&self) -> bool { self.caps_lock }
    pub fn is_num_lock(&self) -> bool { self.num_lock }
    pub fn is_scroll_lock(&self) -> bool { self.scroll_lock }

    // =========================================================================
    // LED Control
    // =========================================================================

    /// Update LED state
    fn update_leds(&mut self) {
        let mut state = 0u8;
        if self.scroll_lock { state |= led::SCROLL_LOCK; }
        if self.num_lock { state |= led::NUM_LOCK; }
        if self.caps_lock { state |= led::CAPS_LOCK; }

        if state != self.led_state {
            self.led_state = state;

            if self.use_msio_leds {
                self.set_leds_via_msio(state);
            } else {
                self.set_leds_via_ps2(state);
            }
        }
    }

    /// Set LEDs via standard PS/2 command (may not work on Armada!)
    fn set_leds_via_ps2(&self, state: u8) {
        unsafe {
            // Wait for input buffer to be empty
            for _ in 0..10000 {
                if (inb(PS2_STATUS_PORT) & status::INPUT_BUFFER_FULL) == 0 {
                    break;
                }
            }

            // Send LED command
            outb(PS2_DATA_PORT, ps2_cmd::SET_LEDS);

            // Wait and send state
            for _ in 0..10000 {
                if (inb(PS2_STATUS_PORT) & status::INPUT_BUFFER_FULL) == 0 {
                    break;
                }
            }
            outb(PS2_DATA_PORT, state);
        }
    }

    /// Set LEDs via MSIO GPIO (recommended for Armada E500/V300)
    ///
    /// On Armada systems, keyboard LEDs are controlled by GPIOs:
    /// - GPIO16 = Caps Lock LED (active low)
    /// - GPIO20 = Num Lock LED (active low)
    /// - GPIO21 = Scroll Lock LED (active low)
    ///
    /// Access is through the ACPI Embedded Controller interface.
    fn set_leds_via_msio(&self, state: u8) {
        // Note: This is a simplified implementation.
        // Full implementation would need proper EC communication protocol.
        unsafe {
            // Wait for EC to be ready
            for _ in 0..10000 {
                if (inb(msio::EC_CMD_PORT) & msio::EC_IBF) == 0 {
                    break;
                }
            }

            // LEDs are active low, so invert the state
            let gpio_state = !state;

            // Send write command
            outb(msio::EC_CMD_PORT, msio::EC_WRITE_CMD);

            // Wait and send register address
            for _ in 0..10000 {
                if (inb(msio::EC_CMD_PORT) & msio::EC_IBF) == 0 {
                    break;
                }
            }
            outb(msio::EC_DATA_PORT, msio::GPIO_OUT_REG);

            // Wait and send data
            for _ in 0..10000 {
                if (inb(msio::EC_CMD_PORT) & msio::EC_IBF) == 0 {
                    break;
                }
            }

            // Map LED bits to GPIO bits
            // This mapping may need adjustment based on actual hardware
            let mut gpio_val = 0u8;
            if (gpio_state & led::CAPS_LOCK) != 0 {
                gpio_val |= 1 << (msio::GPIO16_CAPS - 16);  // Offset in GPIO out register
            }
            // Note: GPIO20 and GPIO21 are in different registers
            // Full implementation would write to appropriate registers

            outb(msio::EC_DATA_PORT, gpio_val);
        }
    }

    /// Enable MSIO-based LED control (recommended for Armada)
    pub fn use_msio_leds(&mut self, enable: bool) {
        self.use_msio_leds = enable;
    }
}

// =============================================================================
// Controller Commands
// =============================================================================

/// Wait for keyboard controller to be ready for input
fn wait_for_write() -> bool {
    for _ in 0..10000 {
        if unsafe { inb(PS2_STATUS_PORT) & status::INPUT_BUFFER_FULL } == 0 {
            return true;
        }
    }
    false
}

/// Wait for data to be available
fn wait_for_read() -> bool {
    for _ in 0..10000 {
        if unsafe { inb(PS2_STATUS_PORT) & status::OUTPUT_BUFFER_FULL } != 0 {
            return true;
        }
    }
    false
}

/// Send command to keyboard
pub fn send_command(cmd: u8) -> Option<u8> {
    if !wait_for_write() {
        return None;
    }

    unsafe {
        outb(PS2_DATA_PORT, cmd);
    }

    if !wait_for_read() {
        return None;
    }

    Some(unsafe { inb(PS2_DATA_PORT) })
}

/// Reset keyboard
pub fn reset_keyboard() -> bool {
    if let Some(response) = send_command(ps2_cmd::RESET) {
        if response == ps2_resp::ACK {
            // Wait for BAT result
            if wait_for_read() {
                let bat = unsafe { inb(PS2_DATA_PORT) };
                return bat == ps2_resp::BAT_OK;
            }
        }
    }
    false
}

/// Enable keyboard scanning
pub fn enable_keyboard() -> bool {
    matches!(send_command(ps2_cmd::ENABLE_SCANNING), Some(ps2_resp::ACK))
}

// =============================================================================
// Global Instance
// =============================================================================

/// Global keyboard driver instance
pub static mut KEYBOARD: Keyboard = Keyboard::new();

pub fn get_key() -> Option<BufferedKey> {
    unsafe { KEYBOARD.get_key() }
}

/// Initialize keyboard driver
pub fn init() {
    // Enable keyboard IRQ (handled by PIC init)
    // Optionally reset keyboard
    // reset_keyboard();
    enable_keyboard();
}
