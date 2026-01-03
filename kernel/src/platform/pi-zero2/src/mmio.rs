//! Memory-mapped I/O utilities.

use core::ptr::{read_volatile, write_volatile};

/// Read a 32-bit value from a memory-mapped register.
#[inline(always)]
pub fn read(addr: usize) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}

/// Write a 32-bit value to a memory-mapped register.
#[inline(always)]
pub fn write(addr: usize, value: u32) {
    unsafe { write_volatile(addr as *mut u32, value) }
}

/// Read a 32-bit value, modify with mask and value, write back.
#[inline(always)]
pub fn modify(addr: usize, mask: u32, value: u32) {
    let current = read(addr);
    write(addr, (current & !mask) | (value & mask));
}

/// Set bits in a register.
#[inline(always)]
pub fn set_bits(addr: usize, bits: u32) {
    let current = read(addr);
    write(addr, current | bits);
}

/// Clear bits in a register.
#[inline(always)]
pub fn clear_bits(addr: usize, bits: u32) {
    let current = read(addr);
    write(addr, current & !bits);
}

/// Spin delay (very rough timing).
#[inline(always)]
pub fn delay(cycles: u32) {
    for _ in 0..cycles {
        core::hint::spin_loop();
    }
}
