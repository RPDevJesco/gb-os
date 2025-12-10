//! Synaptics PS/2 TouchPad Driver - Simplified
//!
//! Uses relative mode for reliability on vintage hardware.

use crate::arch::x86::io::{inb, outb};

const PS2_DATA: u16 = 0x60;
const PS2_STATUS: u16 = 0x64;
const PS2_COMMAND: u16 = 0x64;

/// Touchpad driver (relative mode for reliability)
pub struct SynapticsTouchpad {
    pub is_initialized: bool,
    is_synaptics: bool,
    packet: [u8; 3],
    packet_idx: usize,
    screen_width: u32,
    screen_height: u32,
    cursor_x: i32,
    cursor_y: i32,
    buttons: u8,
    sensitivity: i32,
}

impl SynapticsTouchpad {
    pub const fn new() -> Self {
        Self {
            is_initialized: false,
            is_synaptics: false,
            packet: [0; 3],
            packet_idx: 0,
            screen_width: 800,
            screen_height: 600,
            cursor_x: 400,
            cursor_y: 300,
            buttons: 0,
            sensitivity: 2, // Lower = less sensitive
        }
    }

    pub fn set_screen_size(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
        self.cursor_x = (width / 2) as i32;
        self.cursor_y = (height / 2) as i32;
    }

    /// Initialize in simple relative (PS/2 mouse) mode
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Enable auxiliary device
        self.ps2_command(0xA8)?;

        // Enable auxiliary device interrupts
        self.ps2_command(0x20)?; // Read config
        let config = self.ps2_read_timeout(50).unwrap_or(0);
        self.ps2_command(0x60)?; // Write config
        self.ps2_write_data(config | 0x02)?; // Enable aux interrupt

        // Reset mouse
        self.aux_command(0xFF)?;
        let _ = self.ps2_read_timeout(500); // ACK
        let _ = self.ps2_read_timeout(500); // BAT result
        let _ = self.ps2_read_timeout(500); // Device ID

        // Set defaults
        self.aux_command(0xF6)?;

        // Try Synaptics identify (optional, we'll use relative mode anyway)
        self.is_synaptics = self.try_identify_synaptics();

        // Set sample rate to 100/sec
        self.aux_command(0xF3)?;
        self.aux_write(100)?;

        // Set resolution to 8 counts/mm
        self.aux_command(0xE8)?;
        self.aux_write(3)?;

        // Enable data reporting
        self.aux_command(0xF4)?;

        self.is_initialized = true;
        self.packet_idx = 0;

        Ok(())
    }

    /// Try to identify as Synaptics (just for info, we use relative mode)
    fn try_identify_synaptics(&mut self) -> bool {
        // Magic knock sequence
        let _ = self.aux_command(0xE8); self.aux_write(0).ok();
        let _ = self.aux_command(0xE8); self.aux_write(0).ok();
        let _ = self.aux_command(0xE8); self.aux_write(0).ok();
        let _ = self.aux_command(0xE8); self.aux_write(0).ok();
        let _ = self.aux_command(0xE9); // Status request

        let _ = self.ps2_read_timeout(100);
        let id = self.ps2_read_timeout(100).unwrap_or(0);
        let _ = self.ps2_read_timeout(100);

        id == 0x47 // Synaptics signature
    }

    /// Process a byte from the touchpad
    pub fn process_byte(&mut self, byte: u8) -> bool {
        // Basic packet sync: first byte should have bit 3 set
        if self.packet_idx == 0 {
            if byte & 0x08 == 0 {
                // Out of sync, skip this byte
                return false;
            }
        }

        self.packet[self.packet_idx] = byte;
        self.packet_idx += 1;

        if self.packet_idx >= 3 {
            self.packet_idx = 0;
            self.parse_packet();
            return true;
        }

        false
    }

    /// Parse a complete 3-byte packet
    fn parse_packet(&mut self) {
        let flags = self.packet[0];
        let mut dx = self.packet[1] as i32;
        let mut dy = self.packet[2] as i32;

        // Sign extend using flag bits
        if flags & 0x10 != 0 { dx -= 256; }
        if flags & 0x20 != 0 { dy -= 256; }

        // Check for overflow
        if flags & 0x40 != 0 { dx = 0; }
        if flags & 0x80 != 0 { dy = 0; }

        // Update buttons
        self.buttons = flags & 0x07;

        // Apply movement with sensitivity scaling
        self.cursor_x += dx * self.sensitivity;
        self.cursor_y -= dy * self.sensitivity; // Y is inverted

        // Clamp to screen
        self.cursor_x = self.cursor_x.max(0).min(self.screen_width as i32 - 1);
        self.cursor_y = self.cursor_y.max(0).min(self.screen_height as i32 - 1);
    }

    pub fn get_position(&self) -> (i32, i32) {
        (self.cursor_x, self.cursor_y)
    }

    pub fn get_buttons(&self) -> u8 {
        self.buttons
    }

    pub fn is_synaptics(&self) -> bool {
        self.is_synaptics
    }

    // =========================================================================
    // PS/2 Low-level
    // =========================================================================

    fn ps2_wait_write(&self) -> Result<(), &'static str> {
        for _ in 0..10000 {
            if unsafe { inb(PS2_STATUS) } & 0x02 == 0 {
                return Ok(());
            }
        }
        Err("PS/2 write timeout")
    }

    fn ps2_wait_read(&self) -> Result<(), &'static str> {
        for _ in 0..10000 {
            if unsafe { inb(PS2_STATUS) } & 0x01 != 0 {
                return Ok(());
            }
        }
        Err("PS/2 read timeout")
    }

    fn ps2_command(&mut self, cmd: u8) -> Result<(), &'static str> {
        self.ps2_wait_write()?;
        unsafe { outb(PS2_COMMAND, cmd); }
        Ok(())
    }

    fn ps2_write_data(&mut self, data: u8) -> Result<(), &'static str> {
        self.ps2_wait_write()?;
        unsafe { outb(PS2_DATA, data); }
        Ok(())
    }

    fn ps2_read_timeout(&mut self, ms: u32) -> Result<u8, &'static str> {
        for _ in 0..(ms * 1000) {
            if unsafe { inb(PS2_STATUS) } & 0x01 != 0 {
                return Ok(unsafe { inb(PS2_DATA) });
            }
            for _ in 0..100 { unsafe { core::arch::asm!("nop"); } }
        }
        Err("PS/2 read timeout")
    }

    fn aux_command(&mut self, cmd: u8) -> Result<(), &'static str> {
        self.ps2_command(0xD4)?; // Write to auxiliary device
        self.ps2_write_data(cmd)?;
        // Wait for ACK
        let _ = self.ps2_read_timeout(50);
        Ok(())
    }

    fn aux_write(&mut self, data: u8) -> Result<(), &'static str> {
        self.ps2_command(0xD4)?;
        self.ps2_write_data(data)?;
        let _ = self.ps2_read_timeout(50);
        Ok(())
    }
}

// =============================================================================
// Global Instance
// =============================================================================

pub static mut TOUCHPAD: SynapticsTouchpad = SynapticsTouchpad::new();

pub fn init(screen_width: u32, screen_height: u32) -> Result<(), &'static str> {
    unsafe {
        TOUCHPAD.set_screen_size(screen_width, screen_height);
        TOUCHPAD.init()
    }
}

pub fn get_position() -> (i32, i32) {
    unsafe { TOUCHPAD.get_position() }
}

pub fn get_buttons() -> u8 {
    unsafe { TOUCHPAD.get_buttons() }
}

pub fn is_synaptics() -> bool {
    unsafe { TOUCHPAD.is_synaptics() }
}

pub fn is_initialized() -> bool {
    unsafe { TOUCHPAD.is_initialized }
}

pub fn handle_irq_byte(byte: u8) -> bool {
    unsafe { TOUCHPAD.process_byte(byte) }
}
