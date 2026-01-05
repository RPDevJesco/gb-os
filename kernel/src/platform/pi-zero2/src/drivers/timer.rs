//! BCM2835 System Timer Driver
//!
//! The BCM2835 has a free-running 64-bit timer at 1 MHz.
//! This is the recommended timer for bare-metal applications.

use crate::hal::timer::Timer;

// ============================================================================
// BCM2835 System Timer Registers
// ============================================================================

const PERIPHERAL_BASE: usize = 0x3F00_0000;  // BCM2837 on Pi Zero 2W
const TIMER_BASE: usize = PERIPHERAL_BASE + 0x3000;

// Timer register offsets
const TIMER_CS: usize = 0x00;   // Control/Status
const TIMER_CLO: usize = 0x04;  // Counter Lower 32 bits
const TIMER_CHI: usize = 0x08;  // Counter Upper 32 bits
const TIMER_C0: usize = 0x0C;   // Compare 0
const TIMER_C1: usize = 0x10;   // Compare 1 (used by GPU)
const TIMER_C2: usize = 0x14;   // Compare 2
const TIMER_C3: usize = 0x18;   // Compare 3 (used by GPU)

// Timer runs at 1 MHz
const TIMER_FREQ: u32 = 1_000_000;

// ============================================================================
// System Timer Driver
// ============================================================================

pub struct SystemTimer;

impl SystemTimer {
    pub const fn new() -> Self {
        Self
    }
    
    /// Read the 64-bit counter value
    #[inline]
    pub fn read_counter(&self) -> u64 {
        // Must read CHI, CLO, CHI again and check for rollover
        loop {
            let hi1 = self.read_reg(TIMER_CHI);
            let lo = self.read_reg(TIMER_CLO);
            let hi2 = self.read_reg(TIMER_CHI);
            
            if hi1 == hi2 {
                return ((hi1 as u64) << 32) | (lo as u64);
            }
            // Rollover occurred, try again
        }
    }
    
    /// Read register
    #[inline]
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe {
            core::ptr::read_volatile((TIMER_BASE + offset) as *const u32)
        }
    }
    
    /// Write register
    #[inline]
    #[allow(dead_code)]
    fn write_reg(&self, offset: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile((TIMER_BASE + offset) as *mut u32, value);
        }
    }
}

impl Timer for SystemTimer {
    fn ticks(&self) -> u64 {
        self.read_counter()
    }
    
    fn frequency(&self) -> u32 {
        TIMER_FREQ
    }
    
    fn delay_us(&self, us: u64) {
        let start = self.read_counter();
        let target = start + us;
        
        while self.read_counter() < target {
            // Busy wait
            // Could add WFE here for power saving, but timing might be less accurate
        }
    }
}

// ============================================================================
// ARM Generic Timer (alternative, higher precision)
// ============================================================================

/// ARM Generic Timer accessed via system registers
/// Runs at the CPU frequency / CNTFRQ
pub struct ArmTimer;

impl ArmTimer {
    pub const fn new() -> Self {
        Self
    }
    
    /// Read CNTPCT_EL0 (physical counter)
    #[inline]
    pub fn read_counter(&self) -> u64 {
        let count: u64;
        unsafe {
            core::arch::asm!(
                "mrs {}, cntpct_el0",
                out(reg) count
            );
        }
        count
    }
    
    /// Read CNTFRQ_EL0 (counter frequency)
    #[inline]
    pub fn read_frequency(&self) -> u64 {
        let freq: u64;
        unsafe {
            core::arch::asm!(
                "mrs {}, cntfrq_el0",
                out(reg) freq
            );
        }
        freq
    }
}

impl Timer for ArmTimer {
    fn ticks(&self) -> u64 {
        self.read_counter()
    }
    
    fn frequency(&self) -> u32 {
        // Pi Zero 2W typically runs at 54 MHz for the generic timer
        self.read_frequency() as u32
    }
    
    fn delay_us(&self, us: u64) {
        let freq = self.read_frequency();
        let start = self.read_counter();
        let ticks = us * freq / 1_000_000;
        let target = start + ticks;
        
        while self.read_counter() < target {
            // Busy wait
        }
    }
}

// ============================================================================
// Global Timer Instance
// ============================================================================

static SYSTEM_TIMER: SystemTimer = SystemTimer::new();

/// Get the system timer
pub fn get_timer() -> &'static SystemTimer {
    &SYSTEM_TIMER
}

/// Convenience function to get current time in microseconds
pub fn micros() -> u64 {
    SYSTEM_TIMER.read_counter()
}

/// Convenience function to delay for microseconds
pub fn delay_us(us: u64) {
    SYSTEM_TIMER.delay_us(us);
}

/// Convenience function to delay for milliseconds
pub fn delay_ms(ms: u64) {
    SYSTEM_TIMER.delay_us(ms * 1000);
}
