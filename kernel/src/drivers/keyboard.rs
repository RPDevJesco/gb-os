//! PS/2 Keyboard Driver
//!
//! Handles PS/2 keyboard input with a buffer for polling from main loop.
//! The IRQ handler fills the buffer, main loop drains it.

use crate::arch::x86::io::inb;

/// Key event types
#[derive(Debug, Clone, Copy)]
pub enum KeyEvent {
    Press(KeyCode),
    Release(KeyCode),
}

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
    // Extended keys (0xE0 prefix)
    Up = 0x48,
    Left = 0x4B,
    Right = 0x4D,
    Down = 0x50,
    Unknown = 0xFF,
}

impl KeyCode {
    pub fn from_scancode(scancode: u8) -> Self {
        match scancode & 0x7F {
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
            0x44 => Self::F10,
            0x48 => Self::Up, 0x4B => Self::Left,
            0x4D => Self::Right, 0x50 => Self::Down,
            _ => Self::Unknown,
        }
    }

    pub fn to_ascii(self, shift: bool) -> Option<char> {
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
            _ => return None,
        };
        Some(c)
    }
}

// =============================================================================
// Key Buffer - filled by IRQ, drained by main loop
// =============================================================================

const KEY_BUFFER_SIZE: usize = 16;

/// Buffered key press with ASCII translation
#[derive(Clone, Copy)]
pub struct BufferedKey {
    pub keycode: KeyCode,
    pub ascii: Option<char>,
    pub pressed: bool,
}

/// Keyboard state with event buffer
pub struct Keyboard {
    shift_pressed: bool,
    ctrl_pressed: bool,
    alt_pressed: bool,
    caps_lock: bool,
    extended: bool,  // E0 prefix seen
    // Ring buffer for key events
    buffer: [Option<BufferedKey>; KEY_BUFFER_SIZE],
    write_idx: usize,
    read_idx: usize,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            shift_pressed: false,
            ctrl_pressed: false,
            alt_pressed: false,
            caps_lock: false,
            extended: false,
            buffer: [None; KEY_BUFFER_SIZE],
            write_idx: 0,
            read_idx: 0,
        }
    }

    /// Process scancode (called from IRQ handler)
    pub fn process_scancode(&mut self, scancode: u8) -> Option<KeyEvent> {
        // Handle E0 prefix for extended keys
        if scancode == 0xE0 {
            self.extended = true;
            return None;
        }

        let released = scancode & 0x80 != 0;
        let keycode = KeyCode::from_scancode(scancode);

        // Update modifier state
        match keycode {
            KeyCode::LeftShift | KeyCode::RightShift => {
                self.shift_pressed = !released;
            }
            KeyCode::LeftCtrl => {
                self.ctrl_pressed = !released;
            }
            KeyCode::LeftAlt => {
                self.alt_pressed = !released;
            }
            KeyCode::CapsLock if !released => {
                self.caps_lock = !self.caps_lock;
            }
            _ => {}
        }

        self.extended = false;

        // Buffer the key event for main loop
        if !released {
            let shift = self.shift_pressed ^ self.caps_lock;
            let ascii = keycode.to_ascii(shift);

            let key = BufferedKey {
                keycode,
                ascii,
                pressed: true,
            };

            // Add to ring buffer
            self.buffer[self.write_idx] = Some(key);
            self.write_idx = (self.write_idx + 1) % KEY_BUFFER_SIZE;
        }

        if released {
            Some(KeyEvent::Release(keycode))
        } else {
            Some(KeyEvent::Press(keycode))
        }
    }

    /// Get next key from buffer (called from main loop)
    pub fn get_key(&mut self) -> Option<BufferedKey> {
        if self.read_idx == self.write_idx {
            return None;  // Buffer empty
        }

        let key = self.buffer[self.read_idx].take();
        self.read_idx = (self.read_idx + 1) % KEY_BUFFER_SIZE;
        key
    }

    /// Get ASCII for a keycode using current modifier state
    pub fn get_ascii(&self, keycode: KeyCode) -> Option<char> {
        let shift = self.shift_pressed ^ self.caps_lock;
        keycode.to_ascii(shift)
    }

    /// Check if shift is pressed
    pub fn shift(&self) -> bool {
        self.shift_pressed
    }
}

/// Global keyboard instance
pub static mut KEYBOARD: Keyboard = Keyboard::new();

/// Read scancode directly (for polling, not recommended)
pub fn read_scancode() -> u8 {
    unsafe { inb(0x60) }
}

/// Get next buffered key (safe wrapper)
pub fn get_key() -> Option<BufferedKey> {
    unsafe { KEYBOARD.get_key() }
}
