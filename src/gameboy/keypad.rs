//! Keypad input handling
//!
//! The Game Boy has 8 buttons arranged in a matrix that can be
//! read through the P1/JOYP register (0xFF00).

use alloc::vec::Vec;

/// Keypad button identifiers
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

/// Keypad state and register emulation
pub struct Keypad {
    /// Direction buttons (Right, Left, Up, Down)
    row0: u8,
    /// Action buttons (A, B, Select, Start)
    row1: u8,
    /// Current register value
    data: u8,
    /// Pending interrupt flag
    pub interrupt: u8,
}

impl Keypad {
    /// Create a new keypad with all buttons released
    pub fn new() -> Self {
        Self {
            row0: 0x0F,
            row1: 0x0F,
            data: 0xFF,
            interrupt: 0,
        }
    }

    /// Read the P1/JOYP register
    #[inline]
    pub fn rb(&self) -> u8 {
        self.data
    }

    /// Write to the P1/JOYP register (selects button rows)
    #[inline]
    pub fn wb(&mut self, value: u8) {
        self.data = (self.data & 0xCF) | (value & 0x30);
        self.update();
    }

    /// Update register value based on current button states
    fn update(&mut self) {
        let old_values = self.data & 0x0F;
        let mut new_values = 0x0F;

        // P14 - Direction buttons
        if self.data & 0x10 == 0x00 {
            new_values &= self.row0;
        }
        // P15 - Action buttons
        if self.data & 0x20 == 0x00 {
            new_values &= self.row1;
        }

        // Generate interrupt on high-to-low transition
        if old_values == 0x0F && new_values != 0x0F {
            self.interrupt |= 0x10;
        }

        self.data = (self.data & 0xF0) | new_values;
    }

    /// Press a button
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

    /// Release a button
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

    /// Check if a key is currently pressed
    pub fn is_pressed(&self, key: KeypadKey) -> bool {
        match key {
            KeypadKey::Right => self.row0 & (1 << 0) == 0,
            KeypadKey::Left => self.row0 & (1 << 1) == 0,
            KeypadKey::Up => self.row0 & (1 << 2) == 0,
            KeypadKey::Down => self.row0 & (1 << 3) == 0,
            KeypadKey::A => self.row1 & (1 << 0) == 0,
            KeypadKey::B => self.row1 & (1 << 1) == 0,
            KeypadKey::Select => self.row1 & (1 << 2) == 0,
            KeypadKey::Start => self.row1 & (1 << 3) == 0,
        }
    }

    /// Serialize keypad state
    pub fn serialize(&self, output: &mut Vec<u8>) {
        output.push(self.row0);
        output.push(self.row1);
        output.push(self.data);
    }

    /// Deserialize keypad state
    pub fn deserialize(&mut self, data: &[u8]) -> Result<usize, ()> {
        if data.len() < 3 {
            return Err(());
        }
        self.row0 = data[0];
        self.row1 = data[1];
        self.data = data[2];
        Ok(3)
    }
}

impl Default for Keypad {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypress() {
        let mut keypad = Keypad::new();

        keypad.keydown(KeypadKey::A);
        assert!(keypad.is_pressed(KeypadKey::A));

        keypad.keyup(KeypadKey::A);
        assert!(!keypad.is_pressed(KeypadKey::A));
    }

    #[test]
    fn test_register_read() {
        let mut keypad = Keypad::new();

        keypad.keydown(KeypadKey::A);

        // Select action buttons (P15=0, P14=1 -> 0x10)
        keypad.wb(0x10);
        assert_eq!(keypad.rb() & 0x0F, 0x0E); // A pressed (bit 0 low)

        // Select direction buttons (P15=1, P14=0 -> 0x20)
        keypad.wb(0x20);
        assert_eq!(keypad.rb() & 0x0F, 0x0F); // Nothing pressed
    }
}
