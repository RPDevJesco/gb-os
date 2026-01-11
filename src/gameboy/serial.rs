//! Serial port emulation
//!
//! The Game Boy serial port can be used for:
//! - Link cable communication
//! - Game Boy Printer
//! - Other peripherals

use alloc::boxed::Box;
use alloc::vec::Vec;

/// Serial link callback trait - implement this for link cable or printer support
pub trait SerialLink: Send {
    /// Called when a byte is sent over serial
    /// Returns the received byte, or None if no device is connected
    fn transfer(&mut self, value: u8) -> Option<u8>;
}

/// Null serial link (no device connected)
pub struct NullSerial;

impl SerialLink for NullSerial {
    fn transfer(&mut self, _value: u8) -> Option<u8> {
        None
    }
}

/// Serial port state
pub struct Serial {
    /// Serial data register (0xFF01)
    data: u8,
    /// Serial control register (0xFF02)
    control: u8,
    /// Attached device callback
    callback: Option<Box<dyn SerialLink>>,
    /// Pending interrupt
    pub interrupt: u8,
}

impl Serial {
    /// Create a new serial port with no attached device
    pub fn new() -> Self {
        Self {
            data: 0,
            control: 0,
            callback: None,
            interrupt: 0,
        }
    }

    /// Create a new serial port with an attached device
    pub fn new_with_callback(callback: Box<dyn SerialLink>) -> Self {
        Self {
            data: 0,
            control: 0,
            callback: Some(callback),
            interrupt: 0,
        }
    }

    /// Read a serial register
    #[inline]
    pub fn rb(&self, address: u16) -> u8 {
        match address {
            0xFF01 => self.data,
            0xFF02 => self.control | 0b0111_1110, // Unused bits read as 1
            _ => 0xFF,
        }
    }

    /// Write to a serial register
    #[inline]
    pub fn wb(&mut self, address: u16, value: u8) {
        match address {
            0xFF01 => self.data = value,
            0xFF02 => {
                self.control = value;
                // If transfer is requested and we're the master clock
                if value & 0x81 == 0x81 {
                    if let Some(callback) = &mut self.callback {
                        if let Some(result) = callback.transfer(self.data) {
                            self.data = result;
                            self.interrupt = 0x08;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Attach a serial device
    pub fn set_callback(&mut self, callback: Box<dyn SerialLink>) {
        self.callback = Some(callback);
    }

    /// Detach the serial device
    pub fn unset_callback(&mut self) {
        self.callback = None;
    }

    /// Check if a device is attached
    pub fn has_callback(&self) -> bool {
        self.callback.is_some()
    }

    /// Serialize serial state (excluding callback)
    pub fn serialize(&self, output: &mut Vec<u8>) {
        output.push(self.data);
        output.push(self.control);
    }

    /// Deserialize serial state
    pub fn deserialize(&mut self, data: &[u8]) -> Result<usize, ()> {
        if data.len() < 2 {
            return Err(());
        }
        self.data = data[0];
        self.control = data[1];
        Ok(2)
    }
}

impl Default for Serial {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Game Boy Printer implementation
// ============================================================================

/// Game Boy Printer emulator
pub struct GbPrinter {
    status: u8,
    state: u32,
    data: [u8; 0x280 * 9],
    packet: [u8; 0x400],
    count: usize,
    data_count: usize,
    data_size: usize,
    result: u8,
    print_count: u8,
    /// Callback for when a page is printed
    on_print: Option<Box<dyn FnMut(&[u8], usize, usize) + Send>>,
}

impl GbPrinter {
    /// Create a new Game Boy Printer
    pub fn new() -> Self {
        Self {
            status: 0,
            state: 0,
            data: [0; 0x280 * 9],
            packet: [0; 0x400],
            count: 0,
            data_count: 0,
            data_size: 0,
            result: 0,
            print_count: 0,
            on_print: None,
        }
    }

    /// Set callback for print output
    pub fn set_print_callback<F>(&mut self, callback: F)
    where
        F: FnMut(&[u8], usize, usize) + Send + 'static,
    {
        self.on_print = Some(Box::new(callback));
    }

    fn check_crc(&self) -> bool {
        let mut crc = 0u16;
        for i in 2..(6 + self.data_size) {
            crc = crc.wrapping_add(self.packet[i] as u16);
        }
        let msg_crc = (self.packet[6 + self.data_size] as u16)
            .wrapping_add((self.packet[7 + self.data_size] as u16) << 8);
        crc == msg_crc
    }

    fn reset(&mut self) {
        self.state = 0;
        self.data_size = 0;
        self.data_count = 0;
        self.count = 0;
        self.status = 0;
        self.result = 0;
    }

    fn show(&mut self) {
        let image_height = self.data_count / 40;
        if image_height == 0 {
            return;
        }

        if let Some(callback) = &mut self.on_print {
            callback(&self.data[..self.data_count], 160, image_height);
        }

        self.print_count += 1;
    }

    fn receive(&mut self) {
        if self.packet[3] != 0 {
            // Compressed data
            let mut data_idx = 6;
            let mut dest_idx = self.data_count;

            while data_idx - 6 < self.data_size {
                let control = self.packet[data_idx];
                data_idx += 1;

                if control & 0x80 != 0 {
                    let len = ((control & 0x7F) + 2) as usize;
                    for i in 0..len {
                        if dest_idx + i < self.data.len() {
                            self.data[dest_idx + i] = self.packet[data_idx];
                        }
                    }
                    data_idx += 1;
                    dest_idx += len;
                } else {
                    let len = (control + 1) as usize;
                    for i in 0..len {
                        if dest_idx + i < self.data.len() && data_idx + i < self.packet.len() {
                            self.data[dest_idx + i] = self.packet[data_idx + i];
                        }
                    }
                    dest_idx += len;
                    data_idx += len;
                }
            }
            self.data_count = dest_idx;
        } else {
            // Uncompressed data
            for i in 0..self.data_size {
                if self.data_count + i < self.data.len() {
                    self.data[self.data_count + i] = self.packet[6 + i];
                }
            }
            self.data_count += self.data_size;
        }
    }

    fn command(&mut self) {
        match self.packet[2] {
            0x01 => {
                // Initialize
                self.data_count = 0;
                self.status = 0;
            }
            0x02 => {
                // Print
                self.show();
            }
            0x04 => {
                // Data
                self.receive();
            }
            _ => {}
        }
    }

    fn send(&mut self, value: u8) -> u8 {
        self.packet[self.count] = value;
        self.count += 1;

        match self.state {
            0 => {
                if value == 0x88 {
                    self.state = 1;
                } else {
                    self.reset();
                }
            }
            1 => {
                if value == 0x33 {
                    self.state = 2;
                } else {
                    self.reset();
                }
            }
            2 => {
                if self.count == 6 {
                    self.data_size =
                        self.packet[4] as usize + ((self.packet[5] as usize) << 8);
                    if self.data_size > 0 {
                        self.state = 3;
                    } else {
                        self.state = 4;
                    }
                }
            }
            3 => {
                if self.count == self.data_size + 6 {
                    self.state = 4;
                }
            }
            4 => {
                self.state = 5;
            }
            5 => {
                if self.check_crc() {
                    self.command();
                }
                self.state = 6;
            }
            6 => {
                self.result = 0x81;
                self.state = 7;
            }
            7 => {
                self.result = self.status;
                self.state = 0;
                self.count = 0;
            }
            _ => self.reset(),
        }

        self.result
    }
}

impl Default for GbPrinter {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialLink for GbPrinter {
    fn transfer(&mut self, value: u8) -> Option<u8> {
        Some(self.send(value))
    }
}

// ============================================================================
// Stdout printer for debugging
// ============================================================================

/// Simple serial output that prints to a buffer
pub struct SerialBuffer {
    buffer: Vec<u8>,
}

impl SerialBuffer {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn get_output(&self) -> &[u8] {
        &self.buffer
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for SerialBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialLink for SerialBuffer {
    fn transfer(&mut self, value: u8) -> Option<u8> {
        self.buffer.push(value);
        None
    }
}
