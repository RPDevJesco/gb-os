//! GameBoy Keypad Emulation
//!
//! Emulates the GameBoy's joypad register at 0xFF00

/// GameBoy button/direction keys
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeypadKey {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

/// Keypad state
pub struct Keypad {
    /// Direction buttons (right, left, up, down)
    row0: u8,
    /// Action buttons (A, B, Select, Start)
    row1: u8,
    /// Current register value
    data: u8,
    /// Pending interrupt flag
    pub interrupt: u8,
}

impl Keypad {
    pub fn new() -> Keypad {
        Keypad {
            row0: 0x0F,
            row1: 0x0F,
            data: 0xFF,
            interrupt: 0,
        }
    }

    /// Read joypad register (0xFF00)
    pub fn rb(&self) -> u8 {
        self.data
    }

    /// Write joypad register (0xFF00)
    pub fn wb(&mut self, value: u8) {
        self.data = (self.data & 0xCF) | (value & 0x30);
        self.update();
    }

    fn update(&mut self) {
        let old_values = self.data & 0xF;
        let mut new_values = 0xF;

        if self.data & 0x10 == 0x00 {
            new_values &= self.row0;
        }
        if self.data & 0x20 == 0x00 {
            new_values &= self.row1;
        }

        // Trigger interrupt on button press (high->low transition)
        if old_values == 0xF && new_values != 0xF {
            self.interrupt |= 0x10;
        }

        self.data = (self.data & 0xF0) | new_values;
    }

    /// Handle key press
    pub fn keydown(&mut self, key: KeypadKey) {
        match key {
            KeypadKey::Right => self.row0 &= !(1 << 0),
            KeypadKey::Left => self.row0 &= !(1 << 1),
            KeypadKey::Up => self.row0 &= !(1 << 2),
            KeypadKey::Down => self.row0 &= !(1 << 3),
            KeypadKey::A => self.row1 &= !(1 << 0),
            KeypadKey::B => self.row1 &= !(1 << 1),
            KeypadKey::Select => self.row1 &= !(1 << 2),
            KeypadKey::Start => self.row1 &= !(1 << 3),
        }
        self.update();
    }

    /// Handle key release
    pub fn keyup(&mut self, key: KeypadKey) {
        match key {
            KeypadKey::Right => self.row0 |= 1 << 0,
            KeypadKey::Left => self.row0 |= 1 << 1,
            KeypadKey::Up => self.row0 |= 1 << 2,
            KeypadKey::Down => self.row0 |= 1 << 3,
            KeypadKey::A => self.row1 |= 1 << 0,
            KeypadKey::B => self.row1 |= 1 << 1,
            KeypadKey::Select => self.row1 |= 1 << 2,
            KeypadKey::Start => self.row1 |= 1 << 3,
        }
        self.update();
    }
}
