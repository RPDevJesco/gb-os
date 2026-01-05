//! System Timer Driver
//!
//! The BCM2710 has a free-running 64-bit timer that increments
//! at 1 MHz. This provides microsecond-resolution timing for
//! frame pacing and delays.

use crate::mmio::{self, PERIPHERAL_BASE};

// ============================================================================
// Register Addresses
// ============================================================================

const SYSTIMER_BASE: usize = PERIPHERAL_BASE + 0x0000_3000;

/// Timer control/status register
const SYSTIMER_CS: usize = SYSTIMER_BASE + 0x00;

/// Timer counter lower 32 bits
const SYSTIMER_CLO: usize = SYSTIMER_BASE + 0x04;

/// Timer counter upper 32 bits
const SYSTIMER_CHI: usize = SYSTIMER_BASE + 0x08;

/// Timer compare registers (for interrupts)
const SYSTIMER_C0: usize = SYSTIMER_BASE + 0x0C;
const SYSTIMER_C1: usize = SYSTIMER_BASE + 0x10;
const SYSTIMER_C2: usize = SYSTIMER_BASE + 0x14;
const SYSTIMER_C3: usize = SYSTIMER_BASE + 0x18;

// ============================================================================
// Timer Constants
// ============================================================================

/// Timer frequency in Hz (1 MHz)
pub const TIMER_FREQ_HZ: u32 = 1_000_000;

/// Microseconds per second
pub const USEC_PER_SEC: u32 = 1_000_000;

/// Game Boy frame time in microseconds (~59.7 Hz)
pub const GB_FRAME_TIME_US: u32 = 16_750;

/// 60 FPS frame time in microseconds
pub const FRAME_TIME_60FPS_US: u32 = 16_667;

// ============================================================================
// Timer Functions
// ============================================================================

/// Read the current timer value (lower 32 bits only).
///
/// This wraps every ~71 minutes but is sufficient for frame timing.
#[inline(always)]
pub fn micros() -> u32 {
    mmio::read(SYSTIMER_CLO)
}

/// Read the full 64-bit timer value.
///
/// Note: On ARM, reading two 32-bit registers is not atomic.
/// We read CHI twice to ensure consistency.
pub fn micros64() -> u64 {
    loop {
        let hi1 = mmio::read(SYSTIMER_CHI);
        let lo = mmio::read(SYSTIMER_CLO);
        let hi2 = mmio::read(SYSTIMER_CHI);

        if hi1 == hi2 {
            return ((hi1 as u64) << 32) | (lo as u64);
        }
        // CHI changed during read, retry
    }
}

/// Get elapsed time since a start point (handles 32-bit wraparound).
#[inline(always)]
pub fn elapsed_since(start: u32) -> u32 {
    micros().wrapping_sub(start)
}

/// Check if a target time has been reached (handles wraparound).
///
/// Returns `true` if `target` is in the past.
#[inline(always)]
pub fn time_reached(target: u32) -> bool {
    // Wraparound-safe comparison:
    // If (current - target) > 0x80000000, then target is in the future
    micros().wrapping_sub(target) < 0x8000_0000
}

/// Delay for the specified number of microseconds.
pub fn delay_us(us: u32) {
    let start = micros();
    while elapsed_since(start) < us {
        core::hint::spin_loop();
    }
}

/// Delay for the specified number of milliseconds.
pub fn delay_ms(ms: u32) {
    delay_us(ms * 1000);
}

// ============================================================================
// Frame Timing
// ============================================================================

/// Frame timer for consistent frame pacing.
pub struct FrameTimer {
    last_frame: u32,
    frame_time_us: u32,
}

impl FrameTimer {
    /// Create a new frame timer with the given target frame time.
    pub fn new(frame_time_us: u32) -> Self {
        Self {
            last_frame: micros(),
            frame_time_us,
        }
    }

    /// Create a frame timer for Game Boy timing (~59.7 Hz).
    pub fn gameboy() -> Self {
        Self::new(GB_FRAME_TIME_US)
    }

    /// Create a frame timer for 60 FPS.
    pub fn fps_60() -> Self {
        Self::new(FRAME_TIME_60FPS_US)
    }

    /// Wait until it's time for the next frame.
    ///
    /// Returns the number of microseconds we were late (0 if on time).
    pub fn wait_for_frame(&mut self) -> u32 {
        let target = self.last_frame.wrapping_add(self.frame_time_us);

        // Check if we're already past the target (running slow)
        let now = micros();
        let late = if now.wrapping_sub(target) < 0x8000_0000 {
            now.wrapping_sub(target)
        } else {
            // Wait for target time
            while !time_reached(target) {
                core::hint::spin_loop();
            }
            0
        };

        self.last_frame = target;
        late
    }

    /// Reset the timer (call after pausing or loading).
    pub fn reset(&mut self) {
        self.last_frame = micros();
    }

    /// Get the target frame time in microseconds.
    pub fn frame_time(&self) -> u32 {
        self.frame_time_us
    }

    /// Set a new target frame time.
    pub fn set_frame_time(&mut self, us: u32) {
        self.frame_time_us = us;
    }
}

// ============================================================================
// Compare Registers (for future interrupt support)
// ============================================================================

/// Set compare register 1 (channels 0 and 2 are used by GPU).
pub fn set_compare1(value: u32) {
    mmio::write(SYSTIMER_C1, value);
}

/// Set compare register 3.
pub fn set_compare3(value: u32) {
    mmio::write(SYSTIMER_C3, value);
}

/// Clear timer match flag for channel 1.
pub fn clear_match1() {
    mmio::write(SYSTIMER_CS, 1 << 1);
}

/// Clear timer match flag for channel 3.
pub fn clear_match3() {
    mmio::write(SYSTIMER_CS, 1 << 3);
}

/// Check if timer match occurred for channel 1.
pub fn match1_pending() -> bool {
    (mmio::read(SYSTIMER_CS) & (1 << 1)) != 0
}

/// Check if timer match occurred for channel 3.
pub fn match3_pending() -> bool {
    (mmio::read(SYSTIMER_CS) & (1 << 3)) != 0
}
