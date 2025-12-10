//! GameBoy Input Integration
//!
//! Maps Rustacean OS keyboard driver (KeyCode) to GameBoy keypad buttons.
//! Uses the existing `drivers::keyboard` infrastructure.
//!
//! # Default Mapping
//!
//! | Keyboard    | GameBoy |
//! |-------------|---------|
//! | Arrow keys  | D-pad   |
//! | A           | A       |
//! | S           | B       |
//! | Enter       | Start   |
//! | Space       | Select  |
//! | Z           | A (alt) |
//! | X           | B (alt) |

use super::keypad::KeypadKey;
use crate::drivers::keyboard::KeyCode;

/// Input state tracker
pub struct InputState {
    /// Current pressed state (bitfield)
    pressed: u8,
}

impl InputState {
    pub fn new() -> Self {
        InputState { pressed: 0 }
    }

    /// Map Rustacean OS KeyCode to GameBoy KeypadKey
    pub fn map_keycode(&self, keycode: KeyCode) -> Option<KeypadKey> {
        match keycode {
            // D-pad
            KeyCode::Up => Some(KeypadKey::Up),
            KeyCode::Down => Some(KeypadKey::Down),
            KeyCode::Left => Some(KeypadKey::Left),
            KeyCode::Right => Some(KeypadKey::Right),
            
            // A/B buttons (primary: A/S, alternate: Z/X)
            KeyCode::A => Some(KeypadKey::A),
            KeyCode::S => Some(KeypadKey::B),
            KeyCode::Z => Some(KeypadKey::A),
            KeyCode::X => Some(KeypadKey::B),
            
            // Start/Select
            KeyCode::Enter => Some(KeypadKey::Start),
            KeyCode::Space => Some(KeypadKey::Select),
            
            _ => None,
        }
    }

    /// Update internal state tracking
    pub fn update(&mut self, keycode: KeyCode, pressed: bool) -> Option<KeypadKey> {
        if let Some(key) = self.map_keycode(keycode) {
            let bit = key_to_bit(key);
            if pressed {
                self.pressed |= bit;
            } else {
                self.pressed &= !bit;
            }
            Some(key)
        } else {
            None
        }
    }

    /// Check if a key is currently pressed
    pub fn is_pressed(&self, key: KeypadKey) -> bool {
        let bit = key_to_bit(key);
        self.pressed & bit != 0
    }
}

fn key_to_bit(key: KeypadKey) -> u8 {
    match key {
        KeypadKey::Right => 0x01,
        KeypadKey::Left => 0x02,
        KeypadKey::Up => 0x04,
        KeypadKey::Down => 0x08,
        KeypadKey::A => 0x10,
        KeypadKey::B => 0x20,
        KeypadKey::Select => 0x40,
        KeypadKey::Start => 0x80,
    }
}

/// Alternate input configuration for WASD controls
pub struct WasdInputState {
    pressed: u8,
}

impl WasdInputState {
    pub fn new() -> Self {
        WasdInputState { pressed: 0 }
    }

    /// Map with WASD for D-pad
    pub fn map_keycode(&self, keycode: KeyCode) -> Option<KeypadKey> {
        match keycode {
            // WASD D-pad
            KeyCode::W => Some(KeypadKey::Up),
            KeyCode::S => Some(KeypadKey::Down),
            KeyCode::A => Some(KeypadKey::Left),
            KeyCode::D => Some(KeypadKey::Right),
            
            // Arrow keys also work
            KeyCode::Up => Some(KeypadKey::Up),
            KeyCode::Down => Some(KeypadKey::Down),
            KeyCode::Left => Some(KeypadKey::Left),
            KeyCode::Right => Some(KeypadKey::Right),
            
            // J/K for A/B (like vim)
            KeyCode::J => Some(KeypadKey::A),
            KeyCode::K => Some(KeypadKey::B),
            
            // Start/Select
            KeyCode::Enter => Some(KeypadKey::Start),
            KeyCode::Space => Some(KeypadKey::Select),
            
            _ => None,
        }
    }
}
