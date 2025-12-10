//! GameBoy Timer Emulation
//!
//! Emulates DIV (0xFF04), TIMA (0xFF05), TMA (0xFF06), TAC (0xFF07)

/// Timer state
pub struct Timer {
    /// Divider register (increments at 16384 Hz)
    div: u16,
    /// Timer counter
    tima: u8,
    /// Timer modulo (reload value)
    tma: u8,
    /// Timer control
    tac: u8,
    /// Internal cycle counter
    cycles: u32,
    /// Pending interrupt flag
    pub interrupt: u8,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            cycles: 0,
            interrupt: 0,
        }
    }

    /// Read timer register
    pub fn rb(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8,
            _ => 0xFF,
        }
    }

    /// Write timer register
    pub fn wb(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF04 => self.div = 0,
            0xFF05 => self.tima = value,
            0xFF06 => self.tma = value,
            0xFF07 => self.tac = value & 0x07,
            _ => {}
        }
    }

    /// Advance timer by given CPU cycles
    pub fn do_cycle(&mut self, cycles: u32) {
        // Update DIV (always runs)
        let old_div = self.div;
        self.div = self.div.wrapping_add(cycles as u16);

        // Timer enabled?
        if self.tac & 0x04 == 0 {
            return;
        }

        // Determine which DIV bit to check based on TAC frequency
        let bit = match self.tac & 0x03 {
            0 => 9,  // 4096 Hz
            1 => 3,  // 262144 Hz
            2 => 5,  // 65536 Hz
            3 => 7,  // 16384 Hz
            _ => unreachable!(),
        };

        // Check for falling edge on the selected bit
        let old_bit = (old_div >> bit) & 1;
        let new_bit = (self.div >> bit) & 1;

        if old_bit == 1 && new_bit == 0 {
            // Increment TIMA
            let (new_tima, overflow) = self.tima.overflowing_add(1);
            if overflow {
                self.tima = self.tma;
                self.interrupt |= 0x04; // Timer interrupt
            } else {
                self.tima = new_tima;
            }
        }
    }
}
