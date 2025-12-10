//! GameBoy Serial Port Emulation
//!
//! Emulates serial registers at 0xFF01-0xFF02
//! In bare metal mode, we just stub this out since there's no link cable.

/// Serial callback trait (for future expansion)
pub trait SerialCallback {
    fn call(&mut self, v: u8) -> Option<u8>;
}

/// Serial port state
pub struct Serial {
    /// Serial transfer data
    data: u8,
    /// Serial transfer control
    control: u8,
    /// Pending interrupt flag
    pub interrupt: u8,
    /// Transfer in progress
    transferring: bool,
    /// Cycles until transfer complete
    cycles: u32,
}

impl Serial {
    pub fn new() -> Serial {
        Serial {
            data: 0x00,
            control: 0x00,
            interrupt: 0,
            transferring: false,
            cycles: 0,
        }
    }

    /// Read serial register
    pub fn rb(&self, addr: u16) -> u8 {
        match addr {
            0xFF01 => self.data,
            0xFF02 => self.control | 0x7E,
            _ => 0xFF,
        }
    }

    /// Write serial register
    pub fn wb(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF01 => self.data = value,
            0xFF02 => {
                self.control = value;
                // Start transfer if master clock selected and transfer requested
                if value & 0x81 == 0x81 {
                    self.transferring = true;
                    self.cycles = 4096; // ~1ms at 4MHz (8 bits * 512 cycles)
                }
            }
            _ => {}
        }
    }

    /// Advance serial transfer (called each frame or so)
    pub fn do_cycle(&mut self, cycles: u32) {
        if !self.transferring {
            return;
        }

        if cycles >= self.cycles {
            // Transfer complete - no external device, so we get 0xFF
            self.data = 0xFF;
            self.control &= !0x80; // Clear transfer flag
            self.interrupt |= 0x08; // Serial interrupt
            self.transferring = false;
        } else {
            self.cycles -= cycles;
        }
    }
}
