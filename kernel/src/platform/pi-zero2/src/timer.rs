//! System Timer Driver
//!
//! The BCM2710 has a free-running 64-bit timer that increments
//! at 1 MHz. This provides microsecond-resolution timing for
//! frame pacing and delays.
//!
//! # Usage
//!
//! ```rust
//! // Simple delay
//! timer::delay_ms(100);
//!
//! // Measure elapsed time
//! let start = timer::micros();
//! do_work();
//! let elapsed = timer::elapsed_since(start);
//!
//! // Frame pacing
//! let mut frame_timer = timer::FrameTimer::gameboy();
//! loop {
//!     run_frame();
//!     let late = frame_timer.wait_for_frame();
//! }
//! ```

use crate::mmio::{self, PERIPHERAL_BASE};

// ============================================================================
// Register Addresses
// ============================================================================

const SYSTIMER_BASE: usize = PERIPHERAL_BASE + 0x0000_3000;

/// Timer control/status register
const SYSTIMER_CS: usize = SYSTIMER_BASE + 0x00;

/// Timer counter lower 32 bits (1 MHz clock)
const SYSTIMER_CLO: usize = SYSTIMER_BASE + 0x04;

/// Timer counter upper 32 bits
const SYSTIMER_CHI: usize = SYSTIMER_BASE + 0x08;

/// Timer compare registers (for interrupts)
const SYSTIMER_C0: usize = SYSTIMER_BASE + 0x0C; // Used by GPU
const SYSTIMER_C1: usize = SYSTIMER_BASE + 0x10; // Available
const SYSTIMER_C2: usize = SYSTIMER_BASE + 0x14; // Used by GPU
const SYSTIMER_C3: usize = SYSTIMER_BASE + 0x18; // Available

// ============================================================================
// Timer Constants
// ============================================================================

/// Timer frequency in Hz (1 MHz)
pub const TIMER_FREQ_HZ: u32 = 1_000_000;

/// Microseconds per second
pub const USEC_PER_SEC: u32 = 1_000_000;

/// Microseconds per millisecond
pub const USEC_PER_MSEC: u32 = 1_000;

// ============================================================================
// Frame Timing Constants
// ============================================================================

/// Game Boy frame time in microseconds.
///
/// The Game Boy runs at exactly 59.7275 Hz:
/// - CPU clock: 4.194304 MHz (2^22 Hz)
/// - Cycles per frame: 70224
/// - Frame rate: 4194304 / 70224 = 59.7275 Hz
/// - Frame time: 1000000 / 59.7275 = 16742.7 µs
///
/// We use 16743 for slightly better accuracy than truncating to 16742.
pub const GB_FRAME_TIME_US: u32 = 16_743;

/// Game Boy CPU cycles per frame.
pub const GB_CYCLES_PER_FRAME: u32 = 70_224;

/// Game Boy CPU frequency in Hz.
pub const GB_CPU_FREQ_HZ: u32 = 4_194_304;

/// 60 FPS frame time in microseconds (16666.67 µs)
pub const FRAME_TIME_60FPS_US: u32 = 16_667;

/// 30 FPS frame time in microseconds
pub const FRAME_TIME_30FPS_US: u32 = 33_333;

/// NTSC frame time in microseconds (59.94 Hz)
pub const FRAME_TIME_NTSC_US: u32 = 16_683;

/// PAL frame time in microseconds (50 Hz)
pub const FRAME_TIME_PAL_US: u32 = 20_000;

// ============================================================================
// Basic Timer Functions
// ============================================================================

/// Read the current timer value (lower 32 bits only).
///
/// This wraps every ~71 minutes (2^32 µs) but is sufficient for
/// most timing operations when using wraparound-safe comparisons.
#[inline(always)]
pub fn micros() -> u32 {
    mmio::read(SYSTIMER_CLO)
}

/// Read the full 64-bit timer value.
///
/// This won't wrap for ~584,942 years. Use for long-duration timing
/// or when you need absolute timestamps.
///
/// Note: Reading two 32-bit registers is not atomic, so we read CHI
/// twice to ensure consistency.
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
///
/// This correctly handles the case where the timer has wrapped around
/// since `start` was captured, as long as no more than ~71 minutes
/// have elapsed.
#[inline(always)]
pub fn elapsed_since(start: u32) -> u32 {
    micros().wrapping_sub(start)
}

/// Check if a target time has been reached (handles wraparound).
///
/// Returns `true` if `target` is in the past or now.
/// Returns `false` if `target` is in the future.
///
/// This uses signed comparison on the unsigned difference to handle
/// wraparound correctly, assuming target is within ~35 minutes.
#[inline(always)]
pub fn time_reached(target: u32) -> bool {
    // If (current - target) interpreted as signed is >= 0, target is reached
    // This works for differences up to 2^31 µs (~35 minutes)
    (micros().wrapping_sub(target) as i32) >= 0
}

/// Check if a deadline has passed.
///
/// Alias for `time_reached` with clearer semantics for deadline checking.
#[inline(always)]
pub fn deadline_passed(deadline: u32) -> bool {
    time_reached(deadline)
}

// ============================================================================
// Delay Functions
// ============================================================================

/// Delay for the specified number of microseconds.
///
/// This is a busy-wait delay. For delays longer than a few milliseconds,
/// consider using interrupts or a sleep mechanism instead.
pub fn delay_us(us: u32) {
    let start = micros();
    while elapsed_since(start) < us {
        core::hint::spin_loop();
    }
}

/// Delay for the specified number of milliseconds.
#[inline]
pub fn delay_ms(ms: u32) {
    delay_us(ms.saturating_mul(USEC_PER_MSEC));
}

/// Delay for the specified number of seconds.
#[inline]
pub fn delay_secs(secs: u32) {
    delay_us(secs.saturating_mul(USEC_PER_SEC));
}

/// Delay until a specific target time is reached.
///
/// Returns immediately if target is already in the past.
pub fn delay_until(target: u32) {
    while !time_reached(target) {
        core::hint::spin_loop();
    }
}

// ============================================================================
// Frame Timer
// ============================================================================

/// Frame timer for consistent frame pacing.
///
/// This handles the timing to maintain a steady frame rate, accounting
/// for variable frame processing times. It tracks when frames are late
/// to help identify performance issues.
///
/// # Example
///
/// ```rust
/// let mut timer = FrameTimer::gameboy();
/// loop {
///     process_frame();
///     render_frame();
///
///     let late_by = timer.wait_for_frame();
///     if late_by > 1000 {
///         // Frame was more than 1ms late, maybe skip next frame
///     }
/// }
/// ```
pub struct FrameTimer {
    /// Time when last frame started
    last_frame: u32,
    /// Target frame time in microseconds
    frame_time_us: u32,
    /// Accumulated fractional microseconds for precise timing
    fractional_accum: u32,
    /// Fractional microseconds to add each frame (for non-integer frame times)
    fractional_per_frame: u32,
}

impl FrameTimer {
    /// Create a new frame timer with the given target frame time.
    ///
    /// # Arguments
    /// * `frame_time_us` - Target time per frame in microseconds
    pub fn new(frame_time_us: u32) -> Self {
        Self {
            last_frame: micros(),
            frame_time_us,
            fractional_accum: 0,
            fractional_per_frame: 0,
        }
    }

    /// Create a new frame timer with fractional microsecond precision.
    ///
    /// # Arguments
    /// * `frame_time_us` - Integer part of frame time
    /// * `fractional_1000` - Fractional part in 1/1000ths of a microsecond
    ///
    /// For example, for 16742.7 µs, use `new_precise(16742, 700)`.
    pub fn new_precise(frame_time_us: u32, fractional_1000: u32) -> Self {
        Self {
            last_frame: micros(),
            frame_time_us,
            fractional_accum: 0,
            fractional_per_frame: fractional_1000,
        }
    }

    /// Create a frame timer for Game Boy timing (~59.7275 Hz).
    ///
    /// Uses precise timing: 16742.7 µs per frame.
    pub fn gameboy() -> Self {
        Self::new_precise(16742, 706) // 16742.706 µs ≈ 59.7275 Hz
    }

    /// Create a frame timer for 60 FPS.
    pub fn fps_60() -> Self {
        Self::new_precise(16666, 667) // 16666.667 µs = 60 Hz
    }

    /// Create a frame timer for 30 FPS.
    pub fn fps_30() -> Self {
        Self::new(FRAME_TIME_30FPS_US)
    }

    /// Create a frame timer for NTSC timing (59.94 Hz).
    pub fn ntsc() -> Self {
        Self::new_precise(16683, 333) // 16683.333 µs ≈ 59.94 Hz
    }

    /// Create a frame timer for PAL timing (50 Hz).
    pub fn pal() -> Self {
        Self::new(FRAME_TIME_PAL_US)
    }

    /// Wait until it's time for the next frame.
    ///
    /// # Returns
    /// The number of microseconds we were late (0 if on time or early).
    pub fn wait_for_frame(&mut self) -> u32 {
        // Calculate target time with fractional accumulation
        let mut frame_time = self.frame_time_us;
        self.fractional_accum += self.fractional_per_frame;
        if self.fractional_accum >= 1000 {
            self.fractional_accum -= 1000;
            frame_time += 1;
        }

        let target = self.last_frame.wrapping_add(frame_time);

        // Check if we're already past the target (running slow)
        let now = micros();
        let late = if time_reached(target) {
            now.wrapping_sub(target)
        } else {
            // Wait for target time
            delay_until(target);
            0
        };

        self.last_frame = target;
        late
    }

    /// Reset the timer (call after pausing, loading, or menu navigation).
    ///
    /// This prevents a large "catch-up" delay after the system was paused.
    pub fn reset(&mut self) {
        self.last_frame = micros();
        self.fractional_accum = 0;
    }

    /// Get the target frame time in microseconds.
    #[inline]
    pub fn frame_time(&self) -> u32 {
        self.frame_time_us
    }

    /// Set a new target frame time.
    pub fn set_frame_time(&mut self, us: u32) {
        self.frame_time_us = us;
        self.fractional_per_frame = 0;
        self.fractional_accum = 0;
    }

    /// Set a new target frame time with fractional precision.
    pub fn set_frame_time_precise(&mut self, us: u32, fractional_1000: u32) {
        self.frame_time_us = us;
        self.fractional_per_frame = fractional_1000;
        self.fractional_accum = 0;
    }

    /// Get target frame rate in Hz (approximate).
    pub fn frame_rate(&self) -> u32 {
        if self.frame_time_us == 0 {
            0
        } else {
            USEC_PER_SEC / self.frame_time_us
        }
    }
}

// ============================================================================
// Stopwatch
// ============================================================================

/// Simple stopwatch for measuring elapsed time.
///
/// # Example
///
/// ```rust
/// let sw = Stopwatch::start();
/// do_work();
/// let elapsed_us = sw.elapsed_us();
/// ```
pub struct Stopwatch {
    start: u32,
}

impl Stopwatch {
    /// Start a new stopwatch.
    #[inline]
    pub fn start() -> Self {
        Self { start: micros() }
    }

    /// Get elapsed time in microseconds.
    #[inline]
    pub fn elapsed_us(&self) -> u32 {
        elapsed_since(self.start)
    }

    /// Get elapsed time in milliseconds.
    #[inline]
    pub fn elapsed_ms(&self) -> u32 {
        self.elapsed_us() / USEC_PER_MSEC
    }

    /// Reset the stopwatch.
    #[inline]
    pub fn reset(&mut self) {
        self.start = micros();
    }

    /// Reset and return elapsed time in microseconds.
    #[inline]
    pub fn lap_us(&mut self) -> u32 {
        let elapsed = self.elapsed_us();
        self.reset();
        elapsed
    }

    /// Check if at least `us` microseconds have elapsed.
    #[inline]
    pub fn has_elapsed_us(&self, us: u32) -> bool {
        self.elapsed_us() >= us
    }

    /// Check if at least `ms` milliseconds have elapsed.
    #[inline]
    pub fn has_elapsed_ms(&self, ms: u32) -> bool {
        self.elapsed_ms() >= ms
    }
}

// ============================================================================
// Performance Counter
// ============================================================================

/// Performance counter for tracking frame timing statistics.
///
/// Useful for debugging performance issues by tracking min/max/average
/// times for operations.
pub struct PerfCounter {
    count: u32,
    total_us: u64,
    min_us: u32,
    max_us: u32,
    current_start: u32,
}

impl PerfCounter {
    /// Create a new performance counter.
    pub const fn new() -> Self {
        Self {
            count: 0,
            total_us: 0,
            min_us: u32::MAX,
            max_us: 0,
            current_start: 0,
        }
    }

    /// Start timing a new sample.
    #[inline]
    pub fn start(&mut self) {
        self.current_start = micros();
    }

    /// Stop timing and record the sample.
    #[inline]
    pub fn stop(&mut self) {
        let elapsed = elapsed_since(self.current_start);
        self.count += 1;
        self.total_us += elapsed as u64;
        self.min_us = self.min_us.min(elapsed);
        self.max_us = self.max_us.max(elapsed);
    }

    /// Record a sample directly (if you already measured the time).
    #[inline]
    pub fn record(&mut self, us: u32) {
        self.count += 1;
        self.total_us += us as u64;
        self.min_us = self.min_us.min(us);
        self.max_us = self.max_us.max(us);
    }

    /// Get number of samples recorded.
    #[inline]
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Get average time in microseconds.
    pub fn average_us(&self) -> u32 {
        if self.count == 0 {
            0
        } else {
            (self.total_us / self.count as u64) as u32
        }
    }

    /// Get minimum time in microseconds.
    #[inline]
    pub fn min_us(&self) -> u32 {
        if self.count == 0 {
            0
        } else {
            self.min_us
        }
    }

    /// Get maximum time in microseconds.
    #[inline]
    pub fn max_us(&self) -> u32 {
        self.max_us
    }

    /// Reset all statistics.
    pub fn reset(&mut self) {
        self.count = 0;
        self.total_us = 0;
        self.min_us = u32::MAX;
        self.max_us = 0;
    }
}

impl Default for PerfCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Compare Registers (for interrupt-driven timing)
// ============================================================================

/// Timer channel for compare register operations.
///
/// Channels 0 and 2 are used by the GPU, so only 1 and 3 are available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerChannel {
    /// Channel 1 (available for ARM)
    C1 = 1,
    /// Channel 3 (available for ARM)
    C3 = 3,
}

/// Set compare register for a timer channel.
pub fn set_compare(channel: TimerChannel, value: u32) {
    let reg = match channel {
        TimerChannel::C1 => SYSTIMER_C1,
        TimerChannel::C3 => SYSTIMER_C3,
    };
    mmio::write(reg, value);
}

/// Clear timer match flag for a channel.
pub fn clear_match(channel: TimerChannel) {
    let bit = 1 << (channel as u32);
    mmio::write(SYSTIMER_CS, bit);
}

/// Check if timer match occurred for a channel.
pub fn match_pending(channel: TimerChannel) -> bool {
    let bit = 1 << (channel as u32);
    (mmio::read(SYSTIMER_CS) & bit) != 0
}

/// Set up a one-shot timer interrupt after `delay_us` microseconds.
pub fn set_oneshot(channel: TimerChannel, delay_us: u32) {
    let target = micros().wrapping_add(delay_us);
    clear_match(channel);
    set_compare(channel, target);
}

/// Set up a repeating timer interrupt every `interval_us` microseconds.
///
/// Call this again from the interrupt handler to set up the next interval.
pub fn set_interval(channel: TimerChannel, interval_us: u32) {
    let reg = match channel {
        TimerChannel::C1 => SYSTIMER_C1,
        TimerChannel::C3 => SYSTIMER_C3,
    };
    let current = mmio::read(reg);
    let next = current.wrapping_add(interval_us);
    clear_match(channel);
    set_compare(channel, next);
}
