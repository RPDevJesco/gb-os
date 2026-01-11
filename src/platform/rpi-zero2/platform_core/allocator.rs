//! TLSF-Inspired Allocator
//!
//! A fast, low-fragmentation allocator for bare-metal environments.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::{self, NonNull};

// ============================================================================
// Configuration
// ============================================================================

/// Minimum block size: header (8) + next (8) + prev (8) + footer (8) = 32
const MIN_BLOCK_SIZE: usize = 32;

/// Number of size classes (covers 32 bytes to 2GB)
const NUM_SIZE_CLASSES: usize = 26;

/// Minimum alignment
const MIN_ALIGN: usize = 8;

/// Size of block header
const HEADER_SIZE: usize = core::mem::size_of::<usize>();

/// Size of block footer (only present in free blocks)
const FOOTER_SIZE: usize = core::mem::size_of::<usize>();

// ============================================================================
// Block Header
// ============================================================================

/// Block header flags (stored in lower bits of size)
mod flags {
    pub const FREE: usize = 0b01;
    pub const PREV_FREE: usize = 0b10;
    pub const MASK: usize = 0b11;
}

/// Block header - present at start of every block
///
/// Size field stores block size with flags in lower 2 bits.
/// Actual size is always aligned to MIN_ALIGN (8), so lower bits are free.
#[repr(C)]
struct BlockHeader {
    size_and_flags: usize,
}

impl BlockHeader {
    #[inline]
    fn size(&self) -> usize {
        self.size_and_flags & !flags::MASK
    }

    #[inline]
    fn set_size(&mut self, size: usize) {
        debug_assert!(size & flags::MASK == 0, "size must be aligned");
        self.size_and_flags = size | (self.size_and_flags & flags::MASK);
    }

    #[inline]
    fn is_free(&self) -> bool {
        self.size_and_flags & flags::FREE != 0
    }

    #[inline]
    fn set_free(&mut self, free: bool) {
        if free {
            self.size_and_flags |= flags::FREE;
        } else {
            self.size_and_flags &= !flags::FREE;
        }
    }

    #[inline]
    fn is_prev_free(&self) -> bool {
        self.size_and_flags & flags::PREV_FREE != 0
    }

    #[inline]
    fn set_prev_free(&mut self, prev_free: bool) {
        if prev_free {
            self.size_and_flags |= flags::PREV_FREE;
        } else {
            self.size_and_flags &= !flags::PREV_FREE;
        }
    }

    /// Pointer to payload (data area after header)
    #[inline]
    fn payload_ptr(&self) -> *mut u8 {
        unsafe { (self as *const Self as *mut u8).add(HEADER_SIZE) }
    }

    /// Get header from payload pointer
    #[inline]
    unsafe fn from_payload(payload: *mut u8) -> *mut Self {
        payload.sub(HEADER_SIZE) as *mut Self
    }

    /// Get next block in memory
    #[inline]
    unsafe fn next_block(&self) -> *mut Self {
        (self as *const Self as *mut u8).add(self.size()) as *mut Self
    }

    /// Get previous block using footer (only valid if prev_free flag set)
    #[inline]
    unsafe fn prev_block(&self) -> *mut Self {
        let footer_ptr = (self as *const Self as *const usize).sub(1);
        let prev_size = *footer_ptr;
        (self as *const Self as *mut u8).sub(prev_size) as *mut Self
    }
}

// ============================================================================
// Free Block List Pointers
// ============================================================================

/// Free list node - stored in payload area of free blocks
///
/// This is NOT a separate allocation - it overlays the payload area
/// of a free block. The actual block structure in memory is:
///
/// ```text
/// Offset 0:           BlockHeader (8 bytes)
/// Offset 8:           FreeListNode.next (8 bytes)
/// Offset 16:          FreeListNode.prev (8 bytes)
/// Offset 24..size-8:  Unused
/// Offset size-8:      Footer (8 bytes) - copy of size for coalescing
/// ```
#[repr(C)]
struct FreeListNode {
    next: Option<NonNull<FreeListNode>>,
    prev: Option<NonNull<FreeListNode>>,
}

// ============================================================================
// Free Block Operations
// ============================================================================

/// Operations on free blocks (header + free list node + footer)
struct FreeBlock;

impl FreeBlock {
    /// Get the free list node from a block header
    #[inline]
    unsafe fn list_node(header: *mut BlockHeader) -> *mut FreeListNode {
        (*header).payload_ptr() as *mut FreeListNode
    }

    /// Get block header from a free list node
    #[inline]
    unsafe fn header_from_node(node: *mut FreeListNode) -> *mut BlockHeader {
        (node as *mut u8).sub(HEADER_SIZE) as *mut BlockHeader
    }

    /// Write footer at end of free block (stores size for coalescing)
    #[inline]
    unsafe fn write_footer(header: *mut BlockHeader) {
        let size = (*header).size();
        let footer_ptr = (header as *mut u8).add(size).sub(FOOTER_SIZE) as *mut usize;
        *footer_ptr = size;
    }

    /// Calculate size class for a given size
    #[inline]
    fn size_class(size: usize) -> usize {
        if size <= MIN_BLOCK_SIZE {
            return 0;
        }
        // Find position of highest set bit
        let bits = usize::BITS - size.leading_zeros() - 1;
        let min_bits = usize::BITS - MIN_BLOCK_SIZE.leading_zeros() - 1;
        ((bits - min_bits) as usize).min(NUM_SIZE_CLASSES - 1)
    }
}

// ============================================================================
// Allocator
// ============================================================================

pub struct TlsfAllocator {
    /// Bitmap: bit N set means free_lists[N] is non-empty
    free_bitmap: UnsafeCell<u32>,

    /// Segregated free lists by size class
    free_lists: UnsafeCell<[Option<NonNull<FreeListNode>>; NUM_SIZE_CLASSES]>,

    /// Heap bounds
    heap_start: UnsafeCell<usize>,
    heap_end: UnsafeCell<usize>,

    /// Statistics
    allocated: UnsafeCell<usize>,
    free: UnsafeCell<usize>,
}

unsafe impl Sync for TlsfAllocator {}

impl TlsfAllocator {
    pub const fn new() -> Self {
        Self {
            free_bitmap: UnsafeCell::new(0),
            free_lists: UnsafeCell::new([None; NUM_SIZE_CLASSES]),
            heap_start: UnsafeCell::new(0),
            heap_end: UnsafeCell::new(0),
            allocated: UnsafeCell::new(0),
            free: UnsafeCell::new(0),
        }
    }

    /// Initialize with a memory region
    ///
    /// # Safety
    /// Must be called once before any allocations. Region must be valid.
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) {
        // Align heap bounds
        let start = align_up(heap_start, MIN_ALIGN);
        let end = (heap_start + heap_size) & !(MIN_ALIGN - 1);
        let size = end - start;

        *self.heap_start.get() = start;
        *self.heap_end.get() = end;
        *self.free.get() = size;
        *self.allocated.get() = 0;

        // Create one large free block
        let header = start as *mut BlockHeader;
        (*header).size_and_flags = size | flags::FREE;

        let node = FreeBlock::list_node(header);
        (*node).next = None;
        (*node).prev = None;

        FreeBlock::write_footer(header);

        // Add to free list
        let class = FreeBlock::size_class(size);
        (*self.free_lists.get())[class] = NonNull::new(node);
        *self.free_bitmap.get() = 1 << class;
    }

    /// Find a free block >= requested size
    unsafe fn find_block(&self, size: usize) -> Option<*mut BlockHeader> {
        let min_class = FreeBlock::size_class(size);
        let bitmap = *self.free_bitmap.get();

        // Mask off classes smaller than what we need
        let available = bitmap & !((1 << min_class) - 1);
        if available == 0 {
            return None;
        }

        // First available class
        let class = available.trailing_zeros() as usize;
        let node = (*self.free_lists.get())[class]?;

        Some(FreeBlock::header_from_node(node.as_ptr()))
    }

    /// Remove a block from its free list
    unsafe fn remove_free(&self, header: *mut BlockHeader) {
        let size = (*header).size();
        let class = FreeBlock::size_class(size);
        let node = FreeBlock::list_node(header);

        // Unlink from list
        if let Some(prev) = (*node).prev {
            (*prev.as_ptr()).next = (*node).next;
        } else {
            // Was head of list
            (*self.free_lists.get())[class] = (*node).next;
        }

        if let Some(next) = (*node).next {
            (*next.as_ptr()).prev = (*node).prev;
        }

        // Clear bitmap bit if list is now empty
        if (*self.free_lists.get())[class].is_none() {
            *self.free_bitmap.get() &= !(1 << class);
        }
    }

    /// Add a block to its free list
    unsafe fn insert_free(&self, header: *mut BlockHeader) {
        let size = (*header).size();
        let class = FreeBlock::size_class(size);
        let node = FreeBlock::list_node(header);

        // Insert at head
        let old_head = (*self.free_lists.get())[class];
        (*node).prev = None;
        (*node).next = old_head;

        if let Some(old) = old_head {
            (*old.as_ptr()).prev = NonNull::new(node);
        }

        (*self.free_lists.get())[class] = NonNull::new(node);
        *self.free_bitmap.get() |= 1 << class;
    }

    /// Split a block, returning remainder as a new free block (if large enough)
    unsafe fn split(&self, header: *mut BlockHeader, needed: usize) {
        let block_size = (*header).size();
        let remainder = block_size - needed;

        // Only split if remainder can hold a valid free block
        if remainder < MIN_BLOCK_SIZE {
            return;
        }

        // Shrink original
        (*header).set_size(needed);

        // Create remainder block
        let rem_header = (header as *mut u8).add(needed) as *mut BlockHeader;
        (*rem_header).size_and_flags = remainder | flags::FREE;
        (*rem_header).set_prev_free(false); // Previous (original) will be allocated

        let rem_node = FreeBlock::list_node(rem_header);
        (*rem_node).next = None;
        (*rem_node).prev = None;

        FreeBlock::write_footer(rem_header);
        self.insert_free(rem_header);

        // Update next block's prev_free flag
        let next = (*rem_header).next_block() as usize;
        if next < *self.heap_end.get() {
            (*(next as *mut BlockHeader)).set_prev_free(true);
        }
    }

    /// Coalesce a free block with adjacent free blocks
    unsafe fn coalesce(&self, mut header: *mut BlockHeader) -> *mut BlockHeader {
        let heap_end = *self.heap_end.get();

        // Merge with next block if free
        let next_addr = (*header).next_block() as usize;
        if next_addr < heap_end {
            let next = next_addr as *mut BlockHeader;
            if (*next).is_free() {
                self.remove_free(next);
                let new_size = (*header).size() + (*next).size();
                (*header).set_size(new_size);
            }
        }

        // Merge with previous block if free
        if (*header).is_prev_free() {
            let prev = (*header).prev_block();
            self.remove_free(prev);
            let new_size = (*prev).size() + (*header).size();
            (*prev).set_size(new_size);
            header = prev;
        }

        // Update footer
        FreeBlock::write_footer(header);

        // Update next block's prev_free flag
        let next_addr = (*header).next_block() as usize;
        if next_addr < heap_end {
            (*(next_addr as *mut BlockHeader)).set_prev_free(true);
        }

        header
    }

    /// Get usage statistics: (allocated_bytes, free_bytes)
    pub fn stats(&self) -> (usize, usize) {
        unsafe { (*self.allocated.get(), *self.free.get()) }
    }
}

unsafe impl GlobalAlloc for TlsfAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(MIN_ALIGN);
        let payload_size = layout.size().max(core::mem::size_of::<FreeListNode>());

        // Total size: header + payload, aligned
        let size = align_up(HEADER_SIZE + payload_size, MIN_ALIGN);
        let size = size.max(MIN_BLOCK_SIZE);

        // Find suitable block
        let header = match self.find_block(size) {
            Some(h) => h,
            None => return ptr::null_mut(),
        };

        self.remove_free(header);

        // Split if much larger than needed
        self.split(header, size);

        // Mark allocated
        (*header).set_free(false);

        // Update next block's prev_free
        let final_size = (*header).size();
        let next_addr = (*header).next_block() as usize;
        if next_addr < *self.heap_end.get() {
            (*(next_addr as *mut BlockHeader)).set_prev_free(false);
        }

        // Update stats
        *self.allocated.get() += final_size;
        *self.free.get() -= final_size;

        (*header).payload_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let header = BlockHeader::from_payload(ptr);
        let size = (*header).size();

        // Update stats
        *self.allocated.get() -= size;
        *self.free.get() += size;

        // Mark free
        (*header).set_free(true);

        // Coalesce and add to free list
        let header = self.coalesce(header);
        self.insert_free(header);
    }
}

// ============================================================================
// Helpers
// ============================================================================

#[inline]
const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

// ============================================================================
// Global Instance
// ============================================================================

#[global_allocator]
pub static ALLOCATOR: TlsfAllocator = TlsfAllocator::new();

/// Initialize the heap
///
/// # Safety
/// Call once before any allocations.
pub unsafe fn init() {
    unsafe extern "C" {
        static __heap_start: u8;
        static __heap_end: u8;
    }

    let start = &__heap_start as *const u8 as usize;
    let end = &__heap_end as *const u8 as usize;

    ALLOCATOR.init(start, end - start);
}

/// Get heap statistics
pub fn stats() -> (usize, usize) {
    ALLOCATOR.stats()
}
