//! Programmable Interval Timer (8253/8254 PIT) - Armada E500 Enhanced
//!
//! The PIT provides the system timer interrupt (IRQ 0).
//!
//! Hardware Details (from Armada E500 Technical Reference Guide):
//! - PIT is integrated into the PIIX4M Southbridge
//! - Reference clock: 14.318 MHz from clock synthesizer
//! - PIT input frequency: 14.318 MHz / 12 = 1.193182 MHz (standard PC frequency)
//! - This matches the industry-standard 1193182 Hz PIT frequency
//!
//! Clock Path (from Tech Ref Chapter 2):
//!   14.318 MHz Crystal → Clock Generator → 14 MHZ_PIIX4 → PIIX4M → PIT
//!   The PIIX4M internally divides by 12 to get the standard 1.193182 MHz
//!
//! Timer Channels:
//! - Channel 0: System timer (IRQ 0) - we use this
//! - Channel 1: DRAM refresh (not used by software)
//! - Channel 2: PC speaker (optional)
//!
//! Typical Usage for Emulation:
//! - Game Boy runs at ~59.7 fps
//! - Setting PIT to 1000 Hz gives 1ms resolution
//! - 17 ticks ≈ 16.7ms ≈ 60 fps frame timing

use super::io::outb;
use core::sync::atomic::{AtomicU32, AtomicBool, Ordering};

// =============================================================================
// Hardware Constants
// =============================================================================

/// PIT I/O ports
mod port {
    pub const CHANNEL_0: u16 = 0x40;   // System timer
    pub const CHANNEL_1: u16 = 0x41;   // DRAM refresh (legacy)
    pub const CHANNEL_2: u16 = 0x42;   // PC speaker
    pub const COMMAND: u16 = 0x43;     // Mode/Command register
}

/// PIT base frequency
///
/// This is derived from the 14.318 MHz reference clock divided by 12:
///   14.31818 MHz / 12 = 1.19318167 MHz ≈ 1193182 Hz
///
/// This is the standard PC timer frequency and is common across all
/// x86 PCs regardless of CPU speed.
pub const PIT_FREQUENCY: u32 = 1193182;

/// Reference clock from Armada E500 clock synthesizer
pub const REFERENCE_CLOCK_MHZ: f32 = 14.318;

/// Divisor to get PIT frequency
pub const CLOCK_DIVISOR: u32 = 12;

/// Command byte fields
mod cmd {
    // Channel select (bits 7:6)
    pub const CHANNEL_0: u8 = 0b00_000000;
    pub const CHANNEL_2: u8 = 0b10_000000;

    // Access mode (bits 5:4)
    pub const ACCESS_LATCH: u8 = 0b00_00_0000;
    pub const ACCESS_LOBYTE: u8 = 0b00_01_0000;
    pub const ACCESS_HIBYTE: u8 = 0b00_10_0000;
    pub const ACCESS_LOHI: u8 = 0b00_11_0000;

    // Operating mode (bits 3:1)
    pub const MODE_0_INTERRUPT: u8 = 0b0000_000_0;   // Interrupt on terminal count
    pub const MODE_2_RATE_GEN: u8 = 0b0000_010_0;    // Rate generator (periodic)
    pub const MODE_3_SQUARE: u8 = 0b0000_011_0;      // Square wave generator

    // BCD mode (bit 0)
    pub const BINARY: u8 = 0b0000_0000;
}

// Default tick rate: 1000 Hz = 1ms per tick (good for emulation timing)
const DEFAULT_HZ: u32 = 1000;

// =============================================================================
// Timer State
// =============================================================================

/// System tick counter
static TICK_COUNT: AtomicU32 = AtomicU32::new(0);

/// Current timer frequency in Hz
static TIMER_HZ: AtomicU32 = AtomicU32::new(DEFAULT_HZ);

/// Timer initialized flag
static INITIALIZED: AtomicBool = AtomicBool::new(false);

// =============================================================================
// Public API
// =============================================================================

/// Initialize the PIT with default frequency (1000 Hz)
pub fn init() {
    init_with_frequency(DEFAULT_HZ);
}

/// Initialize the PIT with custom frequency
///
/// Recommended frequencies:
/// - 100 Hz: Standard Linux/Windows tick rate (10ms resolution)
/// - 1000 Hz: Good for timing-sensitive applications (1ms resolution)
/// - 60 Hz: Match typical display refresh rate
pub fn init_with_frequency(hz: u32) {
    set_frequency(hz);
    INITIALIZED.store(true, Ordering::SeqCst);
}

/// Set the timer frequency in Hz
///
/// The divisor is calculated as: PIT_FREQUENCY / hz
/// Valid range: 19 Hz to 1193182 Hz
///
/// Note: The actual frequency may differ slightly due to integer division.
/// For example, requesting 1000 Hz gives divisor 1193, actual frequency = 1000.15 Hz
pub fn set_frequency(hz: u32) {
    let hz = hz.clamp(19, PIT_FREQUENCY);
    let divisor = PIT_FREQUENCY / hz;

    TIMER_HZ.store(hz, Ordering::SeqCst);

    unsafe {
        // Channel 0, lo/hi byte access, mode 2 (rate generator), binary
        outb(port::COMMAND, cmd::CHANNEL_0 | cmd::ACCESS_LOHI | cmd::MODE_2_RATE_GEN | cmd::BINARY);

        // Send divisor (low byte first, then high byte)
        outb(port::CHANNEL_0, (divisor & 0xFF) as u8);
        outb(port::CHANNEL_0, ((divisor >> 8) & 0xFF) as u8);
    }
}

/// Get the current timer frequency in Hz
pub fn frequency() -> u32 {
    TIMER_HZ.load(Ordering::Relaxed)
}

/// Get the tick period in microseconds
pub fn tick_period_us() -> u32 {
    1_000_000 / TIMER_HZ.load(Ordering::Relaxed)
}

/// Called by timer interrupt handler (IRQ 0)
#[inline]
pub fn tick() {
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Get current tick count
pub fn ticks() -> u32 {
    TICK_COUNT.load(Ordering::Relaxed)
}

/// Get uptime in milliseconds
pub fn uptime_ms() -> u32 {
    let ticks = TICK_COUNT.load(Ordering::Relaxed);
    let hz = TIMER_HZ.load(Ordering::Relaxed);
    // Avoid overflow for large tick counts
    (ticks / hz) * 1000 + ((ticks % hz) * 1000) / hz
}

/// Get uptime in seconds
pub fn uptime_secs() -> u32 {
    let ticks = TICK_COUNT.load(Ordering::Relaxed);
    let hz = TIMER_HZ.load(Ordering::Relaxed);
    ticks / hz
}

/// Check if timer is initialized
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::SeqCst)
}

// =============================================================================
// Delay Functions
// =============================================================================

/// Busy-wait delay in milliseconds
///
/// Note: This is a blocking busy-wait. For non-blocking delays,
/// use proper scheduler sleep functions.
pub fn delay_ms(ms: u32) {
    let start = ticks();
    let hz = TIMER_HZ.load(Ordering::Relaxed);
    let wait_ticks = (ms * hz) / 1000;

    while ticks().wrapping_sub(start) < wait_ticks {
        // HLT instruction to save power while waiting
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Busy-wait delay in microseconds
///
/// Note: Resolution is limited by timer frequency. At 1000 Hz,
/// actual minimum delay is 1ms regardless of us parameter.
pub fn delay_us(us: u32) {
    let start = ticks();
    let hz = TIMER_HZ.load(Ordering::Relaxed);
    let wait_ticks = (us * hz) / 1_000_000;

    // Minimum 1 tick
    let wait_ticks = wait_ticks.max(1);

    while ticks().wrapping_sub(start) < wait_ticks {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Spin delay (no HLT) for very short delays
///
/// Uses NOPs instead of HLT for situations where you can't
/// afford to wait for an interrupt.
pub fn spin_delay_us(us: u32) {
    // Approximate: ~10 cycles per NOP on modern CPUs
    // This is very imprecise and CPU-speed dependent
    let loops = us * 100;  // Tune for your target CPU
    for _ in 0..loops {
        unsafe {
            core::arch::asm!("nop");
        }
    }
}

// =============================================================================
// PC Speaker Support (Channel 2)
// =============================================================================

/// Port 0x61 bits for PC speaker control
mod speaker {
    pub const PORT: u16 = 0x61;
    pub const GATE: u8 = 0x01;   // Enable PIT channel 2
    pub const DATA: u8 = 0x02;   // Enable speaker
}

/// Play a tone on the PC speaker
///
/// frequency_hz: Tone frequency in Hz (e.g., 440 for A4)
pub fn speaker_on(frequency_hz: u32) {
    if frequency_hz == 0 {
        return;
    }

    let divisor = PIT_FREQUENCY / frequency_hz;

    unsafe {
        // Set up channel 2 for square wave
        outb(port::COMMAND, cmd::CHANNEL_2 | cmd::ACCESS_LOHI | cmd::MODE_3_SQUARE | cmd::BINARY);
        outb(port::CHANNEL_2, (divisor & 0xFF) as u8);
        outb(port::CHANNEL_2, ((divisor >> 8) & 0xFF) as u8);

        // Enable speaker
        let port61 = crate::arch::x86::io::inb(speaker::PORT);
        outb(speaker::PORT, port61 | speaker::GATE | speaker::DATA);
    }
}

/// Turn off the PC speaker
pub fn speaker_off() {
    unsafe {
        let port61 = crate::arch::x86::io::inb(speaker::PORT);
        outb(speaker::PORT, port61 & !(speaker::GATE | speaker::DATA));
    }
}

/// Play a beep (blocking)
pub fn beep(frequency_hz: u32, duration_ms: u32) {
    speaker_on(frequency_hz);
    delay_ms(duration_ms);
    speaker_off();
}

// =============================================================================
// Timing Utilities for Emulation
// =============================================================================

/// Calculate ticks needed for a target frame rate
///
/// For Game Boy emulation at 59.7 fps with 1000 Hz timer:
/// ticks_per_frame(59.7) ≈ 17 ticks
pub fn ticks_per_frame(target_fps: f32) -> u32 {
    let hz = TIMER_HZ.load(Ordering::Relaxed) as f32;
    (hz / target_fps) as u32
}

/// Frame timing helper
pub struct FrameTimer {
    last_tick: u32,
    ticks_per_frame: u32,
}

impl FrameTimer {
    /// Create a new frame timer for target FPS
    pub fn new(target_fps: f32) -> Self {
        Self {
            last_tick: ticks(),
            ticks_per_frame: ticks_per_frame(target_fps),
        }
    }

    /// Wait until next frame time
    /// Returns true if we're on time, false if we're behind
    pub fn wait_for_frame(&mut self) -> bool {
        let target = self.last_tick.wrapping_add(self.ticks_per_frame);
        let current = ticks();

        // Check if we're behind
        if current.wrapping_sub(self.last_tick) > self.ticks_per_frame * 2 {
            // Too far behind, reset
            self.last_tick = current;
            return false;
        }

        // Wait for target time
        while ticks().wrapping_sub(target) > 0x80000000 {
            unsafe {
                core::arch::asm!("hlt");
            }
        }

        self.last_tick = target;
        true
    }

    /// Update target FPS
    pub fn set_target_fps(&mut self, fps: f32) {
        self.ticks_per_frame = ticks_per_frame(fps);
    }
}
