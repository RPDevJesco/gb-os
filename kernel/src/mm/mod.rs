//! Memory Management
//!
//! Rustacean OS memory management using pooled intrusive lists.
//! No EventChains here - raw performance is critical.

pub mod intrusive;
pub mod pmm;

pub mod heap;

use crate::boot_info::{E820Map, E820Type};

/// Memory information returned by init
#[derive(Debug, Clone, Copy)]
pub struct MemoryInfo {
    /// Total physical memory in KB
    pub total_kb: u64,
    /// Usable memory in KB
    pub usable_kb: u64,
    /// Number of E820 entries
    pub e820_entries: usize,
}

/// Initialize memory management
pub fn init(e820_map_addr: u32) -> MemoryInfo {
    // Parse E820 memory map
    let e820_map = unsafe { E820Map::from_addr(e820_map_addr) };
    
    let mut total_memory: u64 = 0;
    let mut usable_memory: u64 = 0;
    
    // Calculate memory totals
    for entry in e820_map.iter() {
        let end = entry.end();
        if end > total_memory {
            total_memory = end;
        }
        if entry.memory_type() == E820Type::Usable {
            usable_memory += entry.length;
        }
    }
    
    // Initialize physical memory manager
    pmm::init(&e820_map);
    
    MemoryInfo {
        total_kb: total_memory / 1024,
        usable_kb: usable_memory / 1024,
        e820_entries: e820_map.len(),
    }
}
