//! Timer emulation (DIV, TIMA, TMA, TAC registers)

use alloc::vec::Vec;

/// Timer state and register emulation
pub struct Timer {
    /// Divider register (0xFF04) - increments at 16384 Hz
    divider: u8,
    /// Timer counter (0xFF05) - increments at TAC-specified rate
    counter: u8,
    /// Timer modulo (0xFF06) - value loaded on counter overflow
    modulo: u8,
    /// Timer control (0xFF07)
    enabled: bool,
    /// Clock divider step (16, 64, 256, or 1024)
    step: u32,
    /// Internal counter for TIMA
    internal_cnt: u32,
    /// Internal counter for DIV
    internal_div: u32,
    /// Pending interrupt flag
    pub interrupt: u8,
}

impl Timer {
    /// Create a new timer
    pub fn new() -> Self {
        Self {
            divider: 0,
            counter: 0,
            modulo: 0,
            enabled: false,
            step: 1024,
            internal_cnt: 0,
            internal_div: 0,
            interrupt: 0,
        }
    }

    /// Read a timer register
    #[inline]
    pub fn rb(&self, address: u16) -> u8 {
        match address {
            0xFF04 => self.divider,
            0xFF05 => self.counter,
            0xFF06 => self.modulo,
            0xFF07 => {
                0xF8 | (if self.enabled { 0x04 } else { 0 })
                    | match self.step {
                        16 => 1,
                        64 => 2,
                        256 => 3,
                        _ => 0,
                    }
            }
            _ => 0xFF,
        }
    }

    /// Write to a timer register
    #[inline]
    pub fn wb(&mut self, address: u16, value: u8) {
        match address {
            0xFF04 => {
                self.divider = 0;
                self.internal_div = 0;
            }
            0xFF05 => {
                self.counter = value;
            }
            0xFF06 => {
                self.modulo = value;
            }
            0xFF07 => {
                self.enabled = value & 0x04 != 0;
                self.step = match value & 0x03 {
                    1 => 16,   // 262144 Hz
                    2 => 64,   // 65536 Hz
                    3 => 256,  // 16384 Hz
                    _ => 1024, // 4096 Hz
                };
            }
            _ => {}
        }
    }

    /// Advance the timer by the given number of CPU cycles
    #[inline]
    pub fn do_cycle(&mut self, cycles: u32) {
        // Update DIV register (increments every 256 cycles)
        self.internal_div += cycles;
        while self.internal_div >= 256 {
            self.divider = self.divider.wrapping_add(1);
            self.internal_div -= 256;
        }

        // Update TIMA register if enabled
        if self.enabled {
            self.internal_cnt += cycles;

            while self.internal_cnt >= self.step {
                self.counter = self.counter.wrapping_add(1);
                if self.counter == 0 {
                    self.counter = self.modulo;
                    self.interrupt |= 0x04;
                }
                self.internal_cnt -= self.step;
            }
        }
    }

    /// Serialize timer state
    pub fn serialize(&self, output: &mut Vec<u8>) {
        output.push(self.divider);
        output.push(self.counter);
        output.push(self.modulo);
        output.push(self.enabled as u8);
        output.extend_from_slice(&self.step.to_le_bytes());
        output.extend_from_slice(&self.internal_cnt.to_le_bytes());
        output.extend_from_slice(&self.internal_div.to_le_bytes());
    }

    /// Deserialize timer state
    pub fn deserialize(&mut self, data: &[u8]) -> Result<usize, ()> {
        if data.len() < 16 {
            return Err(());
        }
        self.divider = data[0];
        self.counter = data[1];
        self.modulo = data[2];
        self.enabled = data[3] != 0;
        self.step = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        self.internal_cnt = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        self.internal_div = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        Ok(16)
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_divider() {
        let mut timer = Timer::new();

        timer.do_cycle(256);
        assert_eq!(timer.rb(0xFF04), 1);

        timer.do_cycle(256);
        assert_eq!(timer.rb(0xFF04), 2);
    }

    #[test]
    fn test_timer_overflow() {
        let mut timer = Timer::new();

        // Enable timer at fastest rate (16 cycles)
        timer.wb(0xFF07, 0x05);
        timer.wb(0xFF05, 0xFF); // Counter = 255
        timer.wb(0xFF06, 0x80); // Modulo = 128

        timer.do_cycle(16);
        assert_eq!(timer.rb(0xFF05), 0x80); // Should have wrapped to modulo
        assert_eq!(timer.interrupt, 0x04); // Interrupt should be set
    }
}
