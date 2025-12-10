//! Physical Memory Manager
//!
//! Manages physical memory pages using a pooled intrusive free list.
//! This is performance-critical code - no EventChains overhead here.

use crate::boot_info::{E820Map, E820Type};
use crate::mm::intrusive::{IntrusiveNode, IntrusiveStack};
use core::ptr::NonNull;

/// Page size (4KB)
pub const PAGE_SIZE: usize = 4096;

/// Page frame structure
///
/// Represents a physical page of memory. The node is embedded for
/// zero-allocation free list management.
#[repr(C)]
pub struct PageFrame {
    /// Free list linkage (embedded, not allocated)
    free_node: IntrusiveNode,
    /// Page flags
    flags: PageFlags,
    /// Reference count
    ref_count: u16,
    /// Reserved for future use
    _reserved: u16,
}

impl PageFrame {
    /// Create a new page frame
    pub const fn new() -> Self {
        Self {
            free_node: IntrusiveNode::new(),
            flags: PageFlags::empty(),
            ref_count: 0,
            _reserved: 0,
        }
    }
    
    /// Check if page is free
    pub fn is_free(&self) -> bool {
        self.flags.contains(PageFlags::FREE)
    }
    
    /// Mark page as allocated
    pub fn allocate(&mut self) {
        self.flags.remove(PageFlags::FREE);
        self.ref_count = 1;
    }
    
    /// Mark page as free
    pub fn free(&mut self) {
        self.flags.insert(PageFlags::FREE);
        self.ref_count = 0;
    }
}

bitflags::bitflags! {
    /// Page frame flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PageFlags: u16 {
        /// Page is free
        const FREE = 1 << 0;
        /// Page is reserved (not usable)
        const RESERVED = 1 << 1;
        /// Page is kernel memory
        const KERNEL = 1 << 2;
        /// Page is in use by DMA
        const DMA = 1 << 3;
    }
}

// We need a simple bitflags implementation since we're no_std
mod bitflags {
    #[macro_export]
    macro_rules! bitflags {
        (
            $(#[$outer:meta])*
            $vis:vis struct $name:ident: $T:ty {
                $(
                    $(#[$inner:meta])*
                    const $flag:ident = $value:expr;
                )*
            }
        ) => {
            $(#[$outer])*
            #[repr(transparent)]
            $vis struct $name($T);
            
            impl $name {
                $(
                    $(#[$inner])*
                    pub const $flag: Self = Self($value);
                )*
                
                pub const fn empty() -> Self {
                    Self(0)
                }
                
                pub const fn all() -> Self {
                    Self($($value)|*)
                }
                
                pub const fn bits(&self) -> $T {
                    self.0
                }
                
                pub const fn contains(&self, other: Self) -> bool {
                    (self.0 & other.0) == other.0
                }
                
                pub fn insert(&mut self, other: Self) {
                    self.0 |= other.0;
                }
                
                pub fn remove(&mut self, other: Self) {
                    self.0 &= !other.0;
                }
            }
        };
    }
    
    pub use bitflags;
}

/// Maximum number of page frames we track
/// For 256MB RAM: 256 * 1024 * 1024 / 4096 = 65536 pages
const MAX_PAGE_FRAMES: usize = 65536;

/// Page frame array (statically allocated)
/// This uses ~1MB of memory for 256MB RAM
static mut PAGE_FRAMES: [PageFrame; MAX_PAGE_FRAMES] = {
    const INIT: PageFrame = PageFrame::new();
    [INIT; MAX_PAGE_FRAMES]
};

/// Free page list
static mut FREE_LIST: Option<IntrusiveStack<PageFrame, fn(&PageFrame) -> &IntrusiveNode>> = None;

/// Statistics
static mut STATS: PmmStats = PmmStats {
    total_pages: 0,
    free_pages: 0,
    reserved_pages: 0,
    kernel_pages: 0,
};

/// PMM Statistics
#[derive(Debug, Clone, Copy)]
pub struct PmmStats {
    pub total_pages: usize,
    pub free_pages: usize,
    pub reserved_pages: usize,
    pub kernel_pages: usize,
}

/// Initialize the physical memory manager
pub fn init(e820_map: &E820Map) {
    // Node accessor function
    fn get_node(frame: &PageFrame) -> &IntrusiveNode {
        &frame.free_node
    }
    
    unsafe {
        // Initialize free list
        FREE_LIST = Some(IntrusiveStack::new(get_node));
        
        // First pass: mark all pages as reserved
        for frame in PAGE_FRAMES.iter_mut() {
            frame.flags = PageFlags::RESERVED;
        }
        
        // Second pass: mark usable regions from E820
        for entry in e820_map.iter() {
            if entry.memory_type() != E820Type::Usable {
                continue;
            }
            
            // Align to page boundaries
            let start_addr = ((entry.base + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64) * PAGE_SIZE as u64;
            let end_addr = (entry.end() / PAGE_SIZE as u64) * PAGE_SIZE as u64;
            
            if start_addr >= end_addr {
                continue;
            }
            
            let start_page = (start_addr / PAGE_SIZE as u64) as usize;
            let end_page = (end_addr / PAGE_SIZE as u64) as usize;
            
            for page_idx in start_page..end_page {
                if page_idx >= MAX_PAGE_FRAMES {
                    break;
                }
                
                // Skip first 1MB (reserved for BIOS, bootloader, kernel)
                if page_idx < 256 {
                    continue;
                }
                
                // Skip kernel region (1MB - 2MB for now)
                if page_idx >= 256 && page_idx < 512 {
                    PAGE_FRAMES[page_idx].flags = PageFlags::KERNEL;
                    STATS.kernel_pages += 1;
                    continue;
                }
                
                // Mark as free and add to free list
                PAGE_FRAMES[page_idx].flags = PageFlags::FREE;
                
                if let Some(ref mut list) = FREE_LIST {
                    list.push(&PAGE_FRAMES[page_idx]);
                }
                
                STATS.free_pages += 1;
            }
        }
        
        STATS.total_pages = STATS.free_pages + STATS.kernel_pages + STATS.reserved_pages;
    }
}

/// Allocate a physical page
///
/// Returns the physical address of the allocated page, or None if out of memory.
pub fn alloc_page() -> Option<usize> {
    unsafe {
        let list = FREE_LIST.as_mut()?;
        let frame_ptr = list.pop()?;
        let frame = frame_ptr.as_ptr();
        
        (*frame).allocate();
        STATS.free_pages -= 1;
        
        // Calculate physical address from frame index
        let frame_idx = frame_index(frame);
        Some(frame_idx * PAGE_SIZE)
    }
}

/// Free a physical page
///
/// # Safety
///
/// The address must have been allocated by alloc_page() and not already freed.
pub unsafe fn free_page(phys_addr: usize) {
    let page_idx = phys_addr / PAGE_SIZE;
    
    if page_idx >= MAX_PAGE_FRAMES {
        return;
    }
    
    let frame = &mut PAGE_FRAMES[page_idx];
    
    if frame.is_free() {
        // Double free - panic or log
        return;
    }
    
    frame.free();
    
    if let Some(ref mut list) = FREE_LIST {
        list.push(frame);
    }
    
    STATS.free_pages += 1;
}

/// Get the frame index from a frame pointer
fn frame_index(frame: *const PageFrame) -> usize {
    unsafe {
        let base = PAGE_FRAMES.as_ptr();
        (frame as usize - base as usize) / core::mem::size_of::<PageFrame>()
    }
}

/// Get PMM statistics
pub fn stats() -> PmmStats {
    unsafe { STATS }
}

/// Get free page count
pub fn free_page_count() -> usize {
    unsafe { STATS.free_pages }
}

/// Get total memory in bytes
pub fn total_memory() -> usize {
    unsafe { STATS.total_pages * PAGE_SIZE }
}

/// Get free memory in bytes
pub fn free_memory() -> usize {
    unsafe { STATS.free_pages * PAGE_SIZE }
}
