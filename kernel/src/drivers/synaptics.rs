//! Synaptics PS/2 TouchPad Driver - Armada E500 Enhanced
//!
//! Improved driver based on Compaq Armada E500 Technical Reference Guide.
//!
//! Hardware Features (from Chapter 8):
//! - Capacitive X, Y, Z (pressure) sensing
//! - PS/2 compatible interface via MSIO 8051 controller
//! - 40 or 80 packets/second sample rates
//! - Edge-sensitive coasting feature
//! - Palm detection and multi-finger support (model dependent)
//!
//! The MSIO (SMC FDC37N97X) provides four PS/2 channels:
//! - Internal TouchPad/EasyPoint
//! - External keyboard
//! - External mouse
//! - Unused (reserved)

use crate::arch::x86::io::{inb, outb};

// Import hardware constants (would be from the hw module in actual project)
mod hw {
    pub const PS2_DATA: u16 = 0x60;
    pub const PS2_STATUS: u16 = 0x64;
    pub const PS2_COMMAND: u16 = 0x64;

    // Status bits
    pub const STATUS_OUTPUT_FULL: u8 = 0x01;
    pub const STATUS_INPUT_FULL: u8 = 0x02;
    pub const STATUS_AUX_FULL: u8 = 0x20;

    // Controller commands
    pub const CMD_READ_CONFIG: u8 = 0x20;
    pub const CMD_WRITE_CONFIG: u8 = 0x60;
    pub const CMD_ENABLE_AUX: u8 = 0xA8;
    pub const CMD_WRITE_AUX: u8 = 0xD4;

    // Configuration bits
    pub const CFG_INT_AUX: u8 = 0x02;

    // Device commands
    pub const DEV_RESET: u8 = 0xFF;
    pub const DEV_SET_DEFAULTS: u8 = 0xF6;
    pub const DEV_ENABLE: u8 = 0xF4;
    pub const DEV_SET_RATE: u8 = 0xF3;

    // Synaptics specific
    pub const SYNAPTICS_ID: u8 = 0x47;
    pub const KNOCK_SET_RES: u8 = 0xE8;
    pub const KNOCK_STATUS: u8 = 0xE9;

    // Mode bits
    pub const MODE_ABSOLUTE: u8 = 0x80;
    pub const MODE_HIGH_RATE: u8 = 0x40;
}

/// Synaptics capability flags
#[derive(Debug, Clone, Copy, Default)]
pub struct SynapticsCaps {
    pub extended: bool,
    pub middle_button: bool,
    pub four_buttons: bool,
    pub multi_finger: bool,
    pub palm_detect: bool,
    pub w_mode: bool,
}

/// Touchpad packet types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PacketMode {
    /// Standard 3-byte relative PS/2 packets
    Relative,
    /// 6-byte Synaptics absolute mode packets
    Absolute,
}

/// Touchpad event for GUI consumption
#[derive(Debug, Clone, Copy)]
pub struct TouchpadEvent {
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
    pub pressure: u8,
}

/// Enhanced Synaptics TouchPad driver
pub struct SynapticsTouchpad {
    // Initialization state
    pub is_initialized: bool,
    is_synaptics: bool,
    capabilities: SynapticsCaps,
    model_id: u32,
    firmware_id: u32,

    // Packet handling
    packet_mode: PacketMode,
    packet: [u8; 6],
    packet_idx: usize,

    // Screen dimensions
    screen_width: u32,
    screen_height: u32,

    // Cursor state
    cursor_x: i32,
    cursor_y: i32,
    buttons: u8,
    pressure: u8,

    // Configuration
    sensitivity: i32,
    sample_rate: u8,

    // Edge coasting state (from Tech Ref: edge-sensitive feature)
    edge_coasting: bool,
    coast_x: i32,
    coast_y: i32,

    // Statistics for debugging
    packets_received: u32,
    sync_errors: u32,
}

impl SynapticsTouchpad {
    pub const fn new() -> Self {
        Self {
            is_initialized: false,
            is_synaptics: false,
            capabilities: SynapticsCaps {
                extended: false,
                middle_button: false,
                four_buttons: false,
                multi_finger: false,
                palm_detect: false,
                w_mode: false,
            },
            model_id: 0,
            firmware_id: 0,

            packet_mode: PacketMode::Relative,
            packet: [0; 6],
            packet_idx: 0,

            screen_width: 800,
            screen_height: 600,

            cursor_x: 400,
            cursor_y: 300,
            buttons: 0,
            pressure: 0,

            sensitivity: 2,
            sample_rate: 80,

            edge_coasting: false,
            coast_x: 0,
            coast_y: 0,

            packets_received: 0,
            sync_errors: 0,
        }
    }

    /// Set screen dimensions for cursor clamping
    pub fn set_screen_size(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
        self.cursor_x = (width / 2) as i32;
        self.cursor_y = (height / 2) as i32;
    }

    /// Initialize the touchpad
    /// Attempts Synaptics identification, falls back to standard PS/2 mode
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Step 1: Enable auxiliary device (via MSIO 8051)
        // Per Tech Ref: Host accesses 8051 via ports 60h/64h
        self.ps2_command(hw::CMD_ENABLE_AUX)?;

        // Step 2: Enable auxiliary interrupts (IRQ12)
        self.ps2_command(hw::CMD_READ_CONFIG)?;
        let config = self.ps2_read_timeout(50).unwrap_or(0);
        self.ps2_command(hw::CMD_WRITE_CONFIG)?;
        self.ps2_write_data(config | hw::CFG_INT_AUX)?;

        // Step 3: Reset the device
        self.aux_command(hw::DEV_RESET)?;
        let _ = self.ps2_read_timeout(500); // ACK
        let bat = self.ps2_read_timeout(500).unwrap_or(0); // BAT result
        let id = self.ps2_read_timeout(500).unwrap_or(0);  // Device ID

        if bat != 0xAA {
            // BAT failed, but continue anyway - some devices don't respond properly
        }

        // Step 4: Set defaults
        self.aux_command(hw::DEV_SET_DEFAULTS)?;

        // Step 5: Try Synaptics identification
        self.is_synaptics = self.identify_synaptics();

        if self.is_synaptics {
            // Query extended capabilities
            self.query_capabilities();

            // Try to enable absolute mode for better precision
            // Note: We fall back to relative if this fails
            if self.try_absolute_mode() {
                self.packet_mode = PacketMode::Absolute;
            }
        }

        // Step 6: Configure sample rate
        // Tech Ref: TouchPad supports 40 or 80 samples/second
        self.set_sample_rate(self.sample_rate)?;

        // Step 7: Set resolution (8 counts/mm for best precision)
        self.aux_command(hw::KNOCK_SET_RES)?;
        self.aux_write(3)?; // 8 counts/mm

        // Step 8: Enable data reporting
        self.aux_command(hw::DEV_ENABLE)?;

        self.is_initialized = true;
        self.packet_idx = 0;

        Ok(())
    }

    /// Perform Synaptics identification sequence
    /// Uses the "magic knock" pattern documented in Synaptics protocol
    fn identify_synaptics(&mut self) -> bool {
        // Magic knock sequence: Set resolution to 0 four times, then status request
        for _ in 0..4 {
            if self.aux_command(hw::KNOCK_SET_RES).is_err() {
                return false;
            }
            if self.aux_write(0).is_err() {
                return false;
            }
        }

        // Status request
        if self.aux_command(hw::KNOCK_STATUS).is_err() {
            return false;
        }

        // Read 3-byte response
        let byte1 = self.ps2_read_timeout(100).unwrap_or(0);
        let byte2 = self.ps2_read_timeout(100).unwrap_or(0);
        let byte3 = self.ps2_read_timeout(100).unwrap_or(0);

        // Byte 2 should be 0x47 (Synaptics magic number)
        if byte2 == hw::SYNAPTICS_ID {
            // Extract minor version from response
            self.firmware_id = ((byte1 as u32) << 16) | ((byte2 as u32) << 8) | (byte3 as u32);
            return true;
        }

        false
    }

    /// Query Synaptics extended capabilities
    fn query_capabilities(&mut self) {
        // Query model ID (mode 0x03)
        if let Some(model) = self.synaptics_query(0x03) {
            self.model_id = model;
        }

        // Query capabilities (mode 0x02)
        if let Some(caps) = self.synaptics_query(0x02) {
            self.capabilities.extended = (caps & 0x800000) != 0;
            self.capabilities.middle_button = (caps & 0x040000) != 0;
            self.capabilities.four_buttons = (caps & 0x020000) != 0;
            self.capabilities.multi_finger = (caps & 0x010000) != 0;
            self.capabilities.palm_detect = (caps & 0x008000) != 0;
            self.capabilities.w_mode = (caps & 0x000001) != 0;
        }
    }

    /// Send a Synaptics query command
    fn synaptics_query(&mut self, mode: u8) -> Option<u32> {
        // Set resolution sequence to encode mode
        for i in 0..4 {
            if self.aux_command(hw::KNOCK_SET_RES).is_err() {
                return None;
            }
            let nibble = (mode >> (6 - i * 2)) & 0x03;
            if self.aux_write(nibble).is_err() {
                return None;
            }
        }

        // Status request
        if self.aux_command(hw::KNOCK_STATUS).is_err() {
            return None;
        }

        // Read 3-byte response
        let b1 = self.ps2_read_timeout(100)?;
        let b2 = self.ps2_read_timeout(100)?;
        let b3 = self.ps2_read_timeout(100)?;

        Some(((b1 as u32) << 16) | ((b2 as u32) << 8) | (b3 as u32))
    }

    /// Try to enable Synaptics absolute mode
    fn try_absolute_mode(&mut self) -> bool {
        // Send mode byte via special sequence
        let mode_byte = hw::MODE_ABSOLUTE | hw::MODE_HIGH_RATE;

        // Encode mode byte in resolution commands
        for i in 0..4 {
            if self.aux_command(hw::KNOCK_SET_RES).is_err() {
                return false;
            }
            let nibble = (mode_byte >> (6 - i * 2)) & 0x03;
            if self.aux_write(nibble).is_err() {
                return false;
            }
        }

        // Send set sample rate command with special rate
        if self.aux_command(hw::DEV_SET_RATE).is_err() {
            return false;
        }
        if self.aux_write(0x14).is_err() { // Magic rate for mode set
            return false;
        }

        // Verify mode was set by checking for 6-byte packets
        // (We'll validate this when we receive the first packet)
        true
    }

    /// Set the sample rate
    pub fn set_sample_rate(&mut self, rate: u8) -> Result<(), &'static str> {
        self.aux_command(hw::DEV_SET_RATE)?;
        self.aux_write(rate)?;
        self.sample_rate = rate;
        Ok(())
    }

    /// Process a byte from the touchpad (called from IRQ handler)
    pub fn process_byte(&mut self, byte: u8) -> bool {
        match self.packet_mode {
            PacketMode::Relative => self.process_relative_byte(byte),
            PacketMode::Absolute => self.process_absolute_byte(byte),
        }
    }

    /// Process relative mode packets (3 bytes)
    fn process_relative_byte(&mut self, byte: u8) -> bool {
        // Basic packet sync: first byte should have bit 3 set
        if self.packet_idx == 0 {
            if byte & 0x08 == 0 {
                self.sync_errors += 1;
                return false;
            }
        }

        self.packet[self.packet_idx] = byte;
        self.packet_idx += 1;

        if self.packet_idx >= 3 {
            self.packet_idx = 0;
            self.parse_relative_packet();
            self.packets_received += 1;
            return true;
        }

        false
    }

    /// Process absolute mode packets (6 bytes)
    fn process_absolute_byte(&mut self, byte: u8) -> bool {
        // Synaptics 6-byte packet sync
        // Byte 0: 1wwwww00 (w = W value bits)
        // Byte 3: 1wwwww01 (w = W value bits)

        if self.packet_idx == 0 {
            // First byte must have bits 7=1, 1:0=00
            if (byte & 0xC3) != 0x80 {
                // Try falling back to relative mode
                if byte & 0x08 != 0 {
                    // Looks like relative packet, switch modes
                    self.packet_mode = PacketMode::Relative;
                    return self.process_relative_byte(byte);
                }
                self.sync_errors += 1;
                return false;
            }
        } else if self.packet_idx == 3 {
            // Fourth byte must have bits 7=1, 1:0=01
            if (byte & 0xC3) != 0x81 {
                self.sync_errors += 1;
                self.packet_idx = 0;
                return false;
            }
        }

        self.packet[self.packet_idx] = byte;
        self.packet_idx += 1;

        if self.packet_idx >= 6 {
            self.packet_idx = 0;
            self.parse_absolute_packet();
            self.packets_received += 1;
            return true;
        }

        false
    }

    /// Parse a 3-byte relative packet
    fn parse_relative_packet(&mut self) {
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

        // Check for edge coasting (Tech Ref: edge-sensitive feature)
        self.update_edge_coasting(dx, dy);

        // Apply movement with sensitivity scaling
        if self.edge_coasting {
            self.cursor_x += self.coast_x;
            self.cursor_y += self.coast_y;
        } else {
            self.cursor_x += dx * self.sensitivity;
            self.cursor_y -= dy * self.sensitivity; // Y is inverted
        }

        // Clamp to screen
        self.clamp_cursor();
    }

    /// Parse a 6-byte absolute packet
    fn parse_absolute_packet(&mut self) {
        // Extract absolute coordinates
        // X: bits from bytes 1, 2, 4
        // Y: bits from bytes 4, 5
        let x_low = self.packet[1] as u32;
        let x_mid = (self.packet[2] as u32) << 8;
        let x_high = ((self.packet[4] & 0x0F) as u32) << 12;
        let abs_x = x_low | x_mid | x_high;

        let y_low = self.packet[5] as u32;
        let y_mid = (self.packet[4] as u32 & 0xF0) << 4;
        let abs_y = y_low | y_mid;

        // Extract pressure (Z)
        let z = self.packet[2];
        self.pressure = z;

        // Extract buttons
        let left = (self.packet[0] & 0x01) != 0;
        let right = (self.packet[0] & 0x02) != 0;
        self.buttons = (left as u8) | ((right as u8) << 1);

        // Convert absolute to screen coordinates
        // Typical Synaptics range: 0-6143 (13 bits)
        const TOUCHPAD_MAX: u32 = 6143;

        let new_x = (abs_x * self.screen_width as u32 / TOUCHPAD_MAX) as i32;
        let new_y = ((TOUCHPAD_MAX - abs_y) * self.screen_height as u32 / TOUCHPAD_MAX) as i32;

        // Smooth movement (apply some filtering)
        if z > 10 { // Only update if finger is on pad (pressure threshold)
            self.cursor_x = (self.cursor_x * 3 + new_x) / 4;
            self.cursor_y = (self.cursor_y * 3 + new_y) / 4;
        }

        self.clamp_cursor();
    }

    /// Handle edge coasting feature (from Tech Ref Guide)
    /// When finger reaches edge, cursor continues moving in that direction
    fn update_edge_coasting(&mut self, dx: i32, dy: i32) {
        // Define edge thresholds
        let edge_margin = 20;
        let at_left = self.cursor_x <= edge_margin;
        let at_right = self.cursor_x >= self.screen_width as i32 - edge_margin;
        let at_top = self.cursor_y <= edge_margin;
        let at_bottom = self.cursor_y >= self.screen_height as i32 - edge_margin;

        // If at edge and still receiving movement in that direction, coast
        if (at_left && dx < 0) || (at_right && dx > 0) ||
            (at_top && dy > 0) || (at_bottom && dy < 0) {
            self.edge_coasting = true;
            self.coast_x = if dx != 0 { dx.signum() * 2 } else { 0 };
            self.coast_y = if dy != 0 { -dy.signum() * 2 } else { 0 };
        } else if dx.abs() > 2 || dy.abs() > 2 {
            // Movement away from edge, stop coasting
            self.edge_coasting = false;
            self.coast_x = 0;
            self.coast_y = 0;
        }
    }

    /// Clamp cursor to screen bounds
    fn clamp_cursor(&mut self) {
        self.cursor_x = self.cursor_x.max(0).min(self.screen_width as i32 - 1);
        self.cursor_y = self.cursor_y.max(0).min(self.screen_height as i32 - 1);
    }

    // =========================================================================
    // Public Accessors
    // =========================================================================

    pub fn get_position(&self) -> (i32, i32) {
        (self.cursor_x, self.cursor_y)
    }

    pub fn get_buttons(&self) -> u8 {
        self.buttons
    }

    pub fn is_left_pressed(&self) -> bool {
        self.buttons & 0x01 != 0
    }

    pub fn is_right_pressed(&self) -> bool {
        self.buttons & 0x02 != 0
    }

    pub fn is_middle_pressed(&self) -> bool {
        self.buttons & 0x04 != 0
    }

    pub fn get_pressure(&self) -> u8 {
        self.pressure
    }

    pub fn is_synaptics(&self) -> bool {
        self.is_synaptics
    }

    pub fn get_capabilities(&self) -> &SynapticsCaps {
        &self.capabilities
    }

    pub fn get_model_id(&self) -> u32 {
        self.model_id
    }

    pub fn get_packet_mode(&self) -> PacketMode {
        self.packet_mode
    }

    pub fn get_stats(&self) -> (u32, u32) {
        (self.packets_received, self.sync_errors)
    }

    /// Set sensitivity (1-10, default 2)
    pub fn set_sensitivity(&mut self, sens: i32) {
        self.sensitivity = sens.max(1).min(10);
    }

    // =========================================================================
    // PS/2 Low-level Operations
    // =========================================================================

    fn ps2_wait_write(&self) -> Result<(), &'static str> {
        for _ in 0..10000 {
            if unsafe { inb(hw::PS2_STATUS) } & hw::STATUS_INPUT_FULL == 0 {
                return Ok(());
            }
        }
        Err("PS/2 write timeout")
    }

    fn ps2_wait_read(&self) -> Result<(), &'static str> {
        for _ in 0..10000 {
            if unsafe { inb(hw::PS2_STATUS) } & hw::STATUS_OUTPUT_FULL != 0 {
                return Ok(());
            }
        }
        Err("PS/2 read timeout")
    }

    fn ps2_command(&mut self, cmd: u8) -> Result<(), &'static str> {
        self.ps2_wait_write()?;
        unsafe { outb(hw::PS2_COMMAND, cmd); }
        Ok(())
    }

    fn ps2_write_data(&mut self, data: u8) -> Result<(), &'static str> {
        self.ps2_wait_write()?;
        unsafe { outb(hw::PS2_DATA, data); }
        Ok(())
    }

    fn ps2_read_timeout(&mut self, ms: u32) -> Option<u8> {
        for _ in 0..(ms * 1000) {
            if unsafe { inb(hw::PS2_STATUS) } & hw::STATUS_OUTPUT_FULL != 0 {
                return Some(unsafe { inb(hw::PS2_DATA) });
            }
            // Small delay
            for _ in 0..100 {
                unsafe { core::arch::asm!("nop"); }
            }
        }
        None
    }

    fn aux_command(&mut self, cmd: u8) -> Result<(), &'static str> {
        self.ps2_command(hw::CMD_WRITE_AUX)?;
        self.ps2_write_data(cmd)?;
        // Wait for ACK
        let _ = self.ps2_read_timeout(50);
        Ok(())
    }

    fn aux_write(&mut self, data: u8) -> Result<(), &'static str> {
        self.ps2_command(hw::CMD_WRITE_AUX)?;
        self.ps2_write_data(data)?;
        let _ = self.ps2_read_timeout(50);
        Ok(())
    }
}

// =============================================================================
// Global Instance and Public API
// =============================================================================

pub static mut TOUCHPAD: SynapticsTouchpad = SynapticsTouchpad::new();

/// Initialize the touchpad driver
pub fn init(screen_width: u32, screen_height: u32) -> Result<(), &'static str> {
    unsafe {
        TOUCHPAD.set_screen_size(screen_width, screen_height);
        TOUCHPAD.init()
    }
}

/// Get current cursor position
pub fn get_position() -> (i32, i32) {
    unsafe { TOUCHPAD.get_position() }
}

/// Get button state
pub fn get_buttons() -> u8 {
    unsafe { TOUCHPAD.get_buttons() }
}

/// Check if Synaptics protocol detected
pub fn is_synaptics() -> bool {
    unsafe { TOUCHPAD.is_synaptics() }
}

/// Check if driver is initialized
pub fn is_initialized() -> bool {
    unsafe { TOUCHPAD.is_initialized }
}

/// Handle IRQ byte from interrupt handler
pub fn handle_irq_byte(byte: u8) -> bool {
    unsafe { TOUCHPAD.process_byte(byte) }
}

/// Get driver statistics (packets_received, sync_errors)
pub fn get_stats() -> (u32, u32) {
    unsafe { TOUCHPAD.get_stats() }
}
