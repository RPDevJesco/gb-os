//! Timer Hardware Abstraction
//!
//! Abstracts the differences between:
//! - x86: PIT (Programmable Interval Timer) at 1.19318 MHz
//! - ARM: BCM2835 System Timer at 1 MHz

/// Timer trait for timing operations
pub trait Timer {
    /// Get current tick count (platform-specific resolution)
    fn ticks(&self) -> u64;
    
    /// Get timer frequency in Hz
    fn frequency(&self) -> u32;
    
    /// Convert ticks to microseconds
    fn ticks_to_us(&self, ticks: u64) -> u64 {
        ticks * 1_000_000 / self.frequency() as u64
    }
    
    /// Convert microseconds to ticks
    fn us_to_ticks(&self, us: u64) -> u64 {
        us * self.frequency() as u64 / 1_000_000
    }
    
    /// Delay for specified microseconds
    fn delay_us(&self, us: u64);
    
    /// Delay for specified milliseconds
    fn delay_ms(&self, ms: u64) {
        self.delay_us(ms * 1000);
    }
}

/// Frame timing helper
pub struct FramePacer {
    /// Target frame time in ticks
    target_ticks: u64,
    /// Last frame start tick
    last_frame: u64,
}

impl FramePacer {
    /// Create a new frame pacer for target FPS
    pub fn new<T: Timer>(timer: &T, target_fps: u32) -> Self {
        let target_ticks = timer.frequency() as u64 / target_fps as u64;
        Self {
            target_ticks,
            last_frame: timer.ticks(),
        }
    }
    
    /// Game Boy runs at 59.7275 Hz (approximately)
    pub fn new_gameboy<T: Timer>(timer: &T) -> Self {
        // 4194304 Hz CPU / 70224 cycles per frame = 59.7275 fps
        // Use integer math: frequency * 70224 / 4194304
        let target_ticks = timer.frequency() as u64 * 70224 / 4194304;
        Self {
            target_ticks,
            last_frame: timer.ticks(),
        }
    }
    
    /// Start timing a new frame
    pub fn start_frame<T: Timer>(&mut self, timer: &T) {
        self.last_frame = timer.ticks();
    }
    
    /// Wait until frame time has elapsed
    /// Returns number of ticks waited (0 if frame took too long)
    pub fn end_frame<T: Timer>(&mut self, timer: &T) -> u64 {
        let elapsed = timer.ticks() - self.last_frame;
        
        if elapsed < self.target_ticks {
            let wait_ticks = self.target_ticks - elapsed;
            let wait_us = timer.ticks_to_us(wait_ticks);
            timer.delay_us(wait_us);
            wait_ticks
        } else {
            0
        }
    }
    
    /// Check if we're running behind (frame took too long)
    pub fn is_behind<T: Timer>(&self, timer: &T) -> bool {
        let elapsed = timer.ticks() - self.last_frame;
        elapsed > self.target_ticks
    }
}

/// Game Boy timing constants
pub mod gb_timing {
    /// CPU clock frequency (Hz)
    pub const CPU_FREQ: u32 = 4_194_304;
    
    /// Cycles per frame (including VBlank)
    pub const CYCLES_PER_FRAME: u32 = 70_224;
    
    /// Target FPS (59.7275 Hz)
    pub const TARGET_FPS_X100: u32 = 5973;
    
    /// Frame time in microseconds (~16742.7 us)
    pub const FRAME_TIME_US: u32 = 16743;
    
    /// Cycles per scanline
    pub const CYCLES_PER_LINE: u32 = 456;
    
    /// Visible scanlines
    pub const VISIBLE_LINES: u32 = 144;
    
    /// VBlank scanlines
    pub const VBLANK_LINES: u32 = 10;
    
    /// Total scanlines
    pub const TOTAL_LINES: u32 = 154;
}
