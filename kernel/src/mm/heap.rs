//! Ultra-Simple Bump Allocator for Rustacean OS
//!
//! No atomics, no frills - just bumps a pointer forward.
//! NOT thread-safe, but we're single-threaded anyway.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::cell::UnsafeCell;

// =============================================================================
// Heap Configuration - 16MB mark, 4MB size
// =============================================================================

const HEAP_START: usize = 0x0100_0000;  // 16MB
const HEAP_SIZE: usize = 0x0040_0000;   // 4MB
const HEAP_END: usize = HEAP_START + HEAP_SIZE;

// =============================================================================
// Simple Bump Allocator (no atomics)
// =============================================================================

pub struct SimpleBumpAllocator {
    next: UnsafeCell<usize>,
}

// We're single-threaded, so this is safe
unsafe impl Sync for SimpleBumpAllocator {}

impl SimpleBumpAllocator {
    pub const fn new() -> Self {
        Self {
            next: UnsafeCell::new(HEAP_START),
        }
    }

    pub unsafe fn init(&self) {
        *self.next.get() = HEAP_START;
    }

    /// Align address up
    fn align_up(addr: usize, align: usize) -> usize {
        (addr + align - 1) & !(align - 1)
    }
}

unsafe impl GlobalAlloc for SimpleBumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let next_ptr = self.next.get();
        let current = *next_ptr;

        // Align
        let alloc_start = Self::align_up(current, layout.align());
        let alloc_end = alloc_start + layout.size();

        // Bounds check
        if alloc_end > HEAP_END {
            return ptr::null_mut();
        }

        // Bump
        *next_ptr = alloc_end;

        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't free
    }
}

#[global_allocator]
static ALLOCATOR: SimpleBumpAllocator = SimpleBumpAllocator::new();

pub unsafe fn init() {
    ALLOCATOR.init();
}

/// Heap stats
pub struct HeapStats {
    pub used: usize,
    pub free: usize,
}

pub fn stats() -> HeapStats {
    let used = unsafe { *ALLOCATOR.next.get() } - HEAP_START;
    HeapStats {
        used,
        free: HEAP_SIZE - used,
    }
}
