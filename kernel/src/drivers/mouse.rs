//! PS/2 Mouse Driver
//!
//! Handles PS/2 mouse input for the GUI.

use crate::arch::x86::io::{inb, outb};

/// PS/2 controller ports
const PS2_DATA: u16 = 0x60;
const PS2_STATUS: u16 = 0x64;
const PS2_COMMAND: u16 = 0x64;

/// PS/2 commands
const PS2_CMD_WRITE_MOUSE: u8 = 0xD4;
const PS2_CMD_ENABLE_AUX: u8 = 0xA8;
const PS2_CMD_GET_COMPAQ: u8 = 0x20;
const PS2_CMD_SET_COMPAQ: u8 = 0x60;

/// Mouse commands
const MOUSE_CMD_RESET: u8 = 0xFF;
const MOUSE_CMD_ENABLE: u8 = 0xF4;
const MOUSE_CMD_DISABLE: u8 = 0xF5;
const MOUSE_CMD_SET_DEFAULTS: u8 = 0xF6;
const MOUSE_CMD_SET_SAMPLE_RATE: u8 = 0xF3;

/// Mouse state
pub struct Mouse {
    /// Current X position
    pub x: i32,
    /// Current Y position
    pub y: i32,
    /// Button state (bit 0 = left, bit 1 = right, bit 2 = middle)
    pub buttons: u8,
    /// Packet buffer
    packet: [u8; 3],
    /// Current byte in packet
    packet_idx: u8,
    /// Screen bounds
    max_x: i32,
    max_y: i32,
}

impl Mouse {
    pub const fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            buttons: 0,
            packet: [0; 3],
            packet_idx: 0,
            max_x: 800,
            max_y: 600,
        }
    }
    
    /// Set screen bounds for clamping
    pub fn set_bounds(&mut self, width: u32, height: u32) {
        self.max_x = width as i32;
        self.max_y = height as i32;
        self.x = width as i32 / 2;
        self.y = height as i32 / 2;
    }
    
    /// Process a byte from the mouse
    /// Returns true if a complete packet was processed
    pub fn process_byte(&mut self, byte: u8) -> bool {
        // First byte must have bit 3 set (always 1)
        if self.packet_idx == 0 && (byte & 0x08) == 0 {
            // Out of sync, wait for valid first byte
            return false;
        }
        
        self.packet[self.packet_idx as usize] = byte;
        self.packet_idx += 1;
        
        if self.packet_idx >= 3 {
            self.packet_idx = 0;
            self.process_packet();
            return true;
        }
        
        false
    }
    
    /// Process a complete 3-byte packet
    fn process_packet(&mut self) {
        let flags = self.packet[0];
        let mut dx = self.packet[1] as i32;
        let mut dy = self.packet[2] as i32;
        
        // Handle sign extension
        if flags & 0x10 != 0 {
            dx -= 256;
        }
        if flags & 0x20 != 0 {
            dy -= 256;
        }
        
        // Check for overflow
        if flags & 0x40 != 0 {
            dx = 0;
        }
        if flags & 0x80 != 0 {
            dy = 0;
        }
        
        // Update position (Y is inverted in PS/2)
        self.x = (self.x + dx).max(0).min(self.max_x - 1);
        self.y = (self.y - dy).max(0).min(self.max_y - 1);
        
        // Update buttons
        self.buttons = flags & 0x07;
    }
    
    /// Check if left button is pressed
    pub fn left_button(&self) -> bool {
        self.buttons & 0x01 != 0
    }
    
    /// Check if right button is pressed
    pub fn right_button(&self) -> bool {
        self.buttons & 0x02 != 0
    }
    
    /// Check if middle button is pressed
    pub fn middle_button(&self) -> bool {
        self.buttons & 0x04 != 0
    }
}

/// Global mouse instance
pub static mut MOUSE: Mouse = Mouse::new();

/// Wait for PS/2 controller to be ready for reading
fn wait_read() {
    let mut timeout = 100000u32;
    while timeout > 0 {
        if unsafe { inb(PS2_STATUS) } & 0x01 != 0 {
            return;
        }
        timeout -= 1;
    }
}

/// Wait for PS/2 controller to be ready for writing
fn wait_write() {
    let mut timeout = 100000u32;
    while timeout > 0 {
        if unsafe { inb(PS2_STATUS) } & 0x02 == 0 {
            return;
        }
        timeout -= 1;
    }
}

/// Write a byte to the mouse
fn mouse_write(byte: u8) {
    wait_write();
    unsafe { outb(PS2_COMMAND, PS2_CMD_WRITE_MOUSE); }
    wait_write();
    unsafe { outb(PS2_DATA, byte); }
}

/// Read a byte from the mouse
fn mouse_read() -> u8 {
    wait_read();
    unsafe { inb(PS2_DATA) }
}

/// Initialize the PS/2 mouse
pub fn init(screen_width: u32, screen_height: u32) {
    unsafe {
        // Set bounds first
        MOUSE.set_bounds(screen_width, screen_height);
        
        // Enable auxiliary device (mouse)
        wait_write();
        outb(PS2_COMMAND, PS2_CMD_ENABLE_AUX);
        
        // Get current compaq status byte
        wait_write();
        outb(PS2_COMMAND, PS2_CMD_GET_COMPAQ);
        wait_read();
        let mut status = inb(PS2_DATA);
        
        // Enable IRQ12 (mouse interrupt) and enable mouse clock
        status |= 0x02;  // Enable IRQ12
        status &= !0x20; // Enable mouse clock
        
        // Set compaq status byte
        wait_write();
        outb(PS2_COMMAND, PS2_CMD_SET_COMPAQ);
        wait_write();
        outb(PS2_DATA, status);
        
        // Try to enable the mouse without reset (gentler for trackpads)
        mouse_write(MOUSE_CMD_ENABLE);
        // Ignore response - some trackpads don't ACK properly
        
        // Drain any pending data
        for _ in 0..10 {
            if inb(PS2_STATUS) & 0x01 != 0 {
                let _ = inb(PS2_DATA);
            }
        }
    }
}

/// Read and process mouse data (called from IRQ12 handler)
pub fn handle_irq() -> bool {
    // Check if data is from mouse (bit 5 of status)
    let status = unsafe { inb(PS2_STATUS) };
    if status & 0x20 == 0 {
        return false; // Not mouse data
    }
    
    let byte = unsafe { inb(PS2_DATA) };
    unsafe { MOUSE.process_byte(byte) }
}

/// Get current mouse position
pub fn get_position() -> (i32, i32) {
    unsafe { (MOUSE.x, MOUSE.y) }
}

/// Get button state
pub fn get_buttons() -> u8 {
    unsafe { MOUSE.buttons }
}
