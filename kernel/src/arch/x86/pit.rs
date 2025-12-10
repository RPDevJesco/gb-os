//! Programmable Interval Timer (8253/8254 PIT)
//!
//! The PIT provides the system timer interrupt (IRQ 0).
//! We use it for scheduling and timekeeping.

use super::io::outb;
use core::sync::atomic::{AtomicU32, Ordering};

// PIT ports
const PIT_CHANNEL_0: u16 = 0x40;
const PIT_CHANNEL_1: u16 = 0x41;
const PIT_CHANNEL_2: u16 = 0x42;
const PIT_COMMAND: u16 = 0x43;

// PIT frequency (1.193182 MHz)
const PIT_FREQUENCY: u32 = 1193182;

// Default tick rate (100 Hz = 10ms per tick)
const DEFAULT_HZ: u32 = 100;

/// System tick counter (wraps after ~497 days at 100Hz)
static TICK_COUNT: AtomicU32 = AtomicU32::new(0);

/// Current timer frequency in Hz
static mut TIMER_HZ: u32 = DEFAULT_HZ;

/// Initialize the PIT
///
/// Sets up channel 0 for periodic interrupts at the specified frequency.
pub fn init() {
    set_frequency(DEFAULT_HZ);
}

/// Set the timer frequency in Hz
pub fn set_frequency(hz: u32) {
    let divisor = PIT_FREQUENCY / hz;
    
    unsafe {
        TIMER_HZ = hz;
        
        // Channel 0, lobyte/hibyte, rate generator
        outb(PIT_COMMAND, 0x36);
        
        // Set divisor
        outb(PIT_CHANNEL_0, (divisor & 0xFF) as u8);
        outb(PIT_CHANNEL_0, ((divisor >> 8) & 0xFF) as u8);
    }
}

/// Get the current timer frequency
pub fn frequency() -> u32 {
    unsafe { TIMER_HZ }
}

/// Called by timer interrupt handler
pub fn tick() {
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Get the current tick count
pub fn ticks() -> u32 {
    TICK_COUNT.load(Ordering::Relaxed)
}

/// Get uptime in milliseconds
pub fn uptime_ms() -> u32 {
    let ticks = TICK_COUNT.load(Ordering::Relaxed);
    let hz = unsafe { TIMER_HZ };
    (ticks / hz) * 1000 + ((ticks % hz) * 1000) / hz
}

/// Get uptime in seconds
pub fn uptime_secs() -> u32 {
    let ticks = TICK_COUNT.load(Ordering::Relaxed);
    let hz = unsafe { TIMER_HZ };
    ticks / hz
}

/// Simple busy-wait delay in milliseconds
/// 
/// Note: This is a blocking busy-wait, not suitable for real scheduling.
/// Use proper scheduler sleep for non-blocking delays.
pub fn delay_ms(ms: u32) {
    let start = ticks();
    let hz = unsafe { TIMER_HZ };
    let wait_ticks = (ms * hz) / 1000;
    
    while ticks().wrapping_sub(start) < wait_ticks {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
