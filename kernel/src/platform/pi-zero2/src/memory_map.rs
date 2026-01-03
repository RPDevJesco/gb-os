//! Pi Zero 2 W Memory Map
//!
//! Simplified layout:
//!   - GPU:    32MB  (top of RAM, configured via config.txt)
//!   - Kernel: 10MB  (code, stacks, DMA, page tables)
//!   - Heap:   470MB (everything else)
//!
//! # Physical Memory Layout
//!
//! ```text
//! 0x0000_0000 ┌─────────────────────────────────────────┐
//!             │ Reserved (GPU vectors, firmware)        │ 512KB
//! 0x0008_0000 ├─────────────────────────────────────────┤ ← Kernel load address
//!             │ Kernel .text                            │
//!             │ Kernel .rodata                          │
//!             │ Kernel .data                            │
//!             │ Kernel .bss                             │
//!             │─────────────────────────────────────────│
//!             │ CPU Stacks (4 × 64KB = 256KB)           │
//!             │─────────────────────────────────────────│

#![allow(dead_code)]
//!             │ DMA Buffers (mailbox, EMMC) 256KB       │
//!             │─────────────────────────────────────────│
//!             │ Page Tables (1MB)                       │
//!             │─────────────────────────────────────────│
//!             │ Kernel reserved/growth                  │
//! 0x00A8_0000 ├─────────────────────────────────────────┤ ← 10MB kernel region ends
//!             │                                         │
//!             │              HEAP                       │
//!             │            470 MB                       │
//!             │                                         │
//! 0x1E00_0000 ├─────────────────────────────────────────┤ ← ARM memory ends
//!             │           GPU Memory                    │ 32MB
//!             │      (VideoCore firmware, FB)           │
//! 0x2000_0000 └─────────────────────────────────────────┘ ← 512MB
//! ```
//!
//! # config.txt setting
//!
//! ```text
//! gpu_mem=32
//! ```

// ============================================================================
// Top-Level Memory Split
// ============================================================================

/// Total physical RAM
pub const RAM_SIZE: usize = 512 * 1024 * 1024; // 512MB = 0x2000_0000

/// GPU memory size (set gpu_mem=32 in config.txt)
pub const GPU_MEM_SIZE: usize = 32 * 1024 * 1024; // 32MB = 0x0200_0000

/// ARM-accessible memory (total - GPU)
pub const ARM_MEM_SIZE: usize = RAM_SIZE - GPU_MEM_SIZE; // 480MB = 0x1E00_0000

/// Kernel region size
pub const KERNEL_REGION_SIZE: usize = 10 * 1024 * 1024; // 10MB = 0x00A0_0000

/// Heap size (ARM memory - reserved low - kernel)
pub const HEAP_SIZE: usize = ARM_MEM_SIZE - KERNEL_BASE - KERNEL_REGION_SIZE; // ~470MB

// ============================================================================
// Address Boundaries
// ============================================================================

/// Reserved low memory (GPU vectors, ARM stubs)
pub const RESERVED_LOW_BASE: usize = 0x0000_0000;
pub const RESERVED_LOW_SIZE: usize = 0x0008_0000; // 512KB

/// Kernel load address (where GPU firmware loads kernel8.img)
pub const KERNEL_BASE: usize = 0x0008_0000;

/// Kernel region end
pub const KERNEL_END: usize = KERNEL_BASE + KERNEL_REGION_SIZE; // 0x00A8_0000

/// Heap start
pub const HEAP_BASE: usize = KERNEL_END; // 0x00A8_0000

/// Heap end (ARM memory boundary)
pub const HEAP_END: usize = ARM_MEM_SIZE; // 0x1E00_0000

/// GPU memory start
pub const GPU_MEM_BASE: usize = ARM_MEM_SIZE; // 0x1E00_0000

// ============================================================================
// Kernel Region Subdivision (within 10MB)
// ============================================================================

/// Maximum kernel code/data size
pub const KERNEL_CODE_MAX: usize = 2 * 1024 * 1024; // 2MB for code/rodata/data/bss

/// Stack configuration
pub const STACK_SIZE_PER_CORE: usize = 64 * 1024; // 64KB per core
pub const NUM_CORES: usize = 4;
pub const STACK_REGION_SIZE: usize = STACK_SIZE_PER_CORE * NUM_CORES; // 256KB

/// Stack region base (after generous code allowance)
pub const STACK_REGION_BASE: usize = KERNEL_BASE + KERNEL_CODE_MAX; // 0x0028_0000

/// DMA buffer region
pub const DMA_REGION_SIZE: usize = 256 * 1024; // 256KB
pub const DMA_REGION_BASE: usize = STACK_REGION_BASE + STACK_REGION_SIZE; // 0x002C_0000

/// Page table region
pub const PAGE_TABLE_SIZE: usize = 1024 * 1024; // 1MB
pub const PAGE_TABLE_BASE: usize = DMA_REGION_BASE + DMA_REGION_SIZE; // 0x0030_0000

/// Kernel reserved (remaining space for growth)
pub const KERNEL_RESERVED_BASE: usize = PAGE_TABLE_BASE + PAGE_TABLE_SIZE; // 0x0040_0000
pub const KERNEL_RESERVED_SIZE: usize = KERNEL_END - KERNEL_RESERVED_BASE; // ~6.5MB

// ============================================================================
// Specific Buffer Addresses (within DMA region)
// ============================================================================

/// Mailbox buffer (16-byte aligned)
pub const MAILBOX_BUFFER: usize = DMA_REGION_BASE;
pub const MAILBOX_BUFFER_SIZE: usize = 256;

/// EMMC DMA buffer
pub const EMMC_BUFFER: usize = DMA_REGION_BASE + 4096; // 4KB offset for alignment
pub const EMMC_BUFFER_SIZE: usize = 64 * 1024; // 64KB

/// DMA pool (remaining DMA region)
pub const DMA_POOL_BASE: usize = EMMC_BUFFER + EMMC_BUFFER_SIZE;
pub const DMA_POOL_SIZE: usize = DMA_REGION_BASE + DMA_REGION_SIZE - DMA_POOL_BASE;

// ============================================================================
// Stack Helpers
// ============================================================================

/// Get stack top for a core (stack grows downward)
#[inline]
pub const fn stack_top(core: usize) -> usize {
    STACK_REGION_BASE + STACK_SIZE_PER_CORE * (core + 1)
}

/// Get stack bottom for a core
#[inline]
pub const fn stack_bottom(core: usize) -> usize {
    STACK_REGION_BASE + STACK_SIZE_PER_CORE * core
}

// ============================================================================
// Peripheral Memory (not RAM)
// ============================================================================

/// BCM2710 peripheral base
pub const PERIPHERAL_BASE: usize = 0x3F00_0000;

/// ARM local peripherals (core timers, mailboxes)
pub const ARM_LOCAL_BASE: usize = 0x4000_0000;

// ============================================================================
// Bus Address Conversion (for DMA/VideoCore access)
// ============================================================================

/// Convert ARM physical address to VC bus address (uncached)
#[inline]
pub const fn phys_to_bus(addr: usize) -> u32 {
    (addr as u32) | 0xC000_0000
}

/// Convert VC bus address to ARM physical
#[inline]
pub const fn bus_to_phys(addr: u32) -> usize {
    (addr & 0x3FFF_FFFF) as usize
}

// ============================================================================
// Memory Info Struct
// ============================================================================

/// Memory configuration summary
#[derive(Debug, Clone, Copy)]
pub struct MemoryConfig {
    pub kernel_start: usize,
    pub kernel_end: usize,
    pub heap_start: usize,
    pub heap_end: usize,
    pub heap_size: usize,
    pub gpu_size: usize,
}

impl MemoryConfig {
    pub const fn default() -> Self {
        Self {
            kernel_start: KERNEL_BASE,
            kernel_end: KERNEL_END,
            heap_start: HEAP_BASE,
            heap_end: HEAP_END,
            heap_size: HEAP_SIZE,
            gpu_size: GPU_MEM_SIZE,
        }
    }
}

/// Global memory configuration
pub const MEMORY_CONFIG: MemoryConfig = MemoryConfig::default();

// ============================================================================
// Compile-Time Validation
// ============================================================================

const _: () = {
    // Verify memory math
    assert!(KERNEL_BASE == 0x0008_0000);
    assert!(KERNEL_END == 0x00A8_0000);
    assert!(HEAP_BASE == 0x00A8_0000);
    assert!(HEAP_END == 0x1E00_0000);
    assert!(GPU_MEM_BASE == 0x1E00_0000);

    // Verify sizes
    assert!(KERNEL_END - KERNEL_BASE == 10 * 1024 * 1024); // 10MB
    assert!(GPU_MEM_SIZE == 32 * 1024 * 1024); // 32MB

    // Verify no overlaps
    assert!(RESERVED_LOW_SIZE <= KERNEL_BASE);
    assert!(KERNEL_END <= HEAP_BASE);
    assert!(HEAP_END <= GPU_MEM_BASE);

    // Verify kernel subdivision fits
    assert!(STACK_REGION_BASE >= KERNEL_BASE);
    assert!(DMA_REGION_BASE >= STACK_REGION_BASE + STACK_REGION_SIZE);
    assert!(PAGE_TABLE_BASE >= DMA_REGION_BASE + DMA_REGION_SIZE);
    assert!(PAGE_TABLE_BASE + PAGE_TABLE_SIZE <= KERNEL_END);

    // Verify alignments
    assert!(KERNEL_BASE & 0xFFFF == 0); // 64KB aligned
    assert!(STACK_REGION_BASE & 0xFFF == 0); // 4KB aligned
    assert!(DMA_REGION_BASE & 0xF == 0); // 16-byte aligned
    assert!(PAGE_TABLE_BASE & 0xFFF == 0); // 4KB aligned
    assert!(HEAP_BASE & 0xFFF == 0); // 4KB aligned
};

// ============================================================================
// Debug Output
// ============================================================================

/// Print memory map (requires Serial trait)
pub fn print_memory_map<S: bootcore::Serial>(serial: &mut S) {
    use core::fmt::Write;
    use bootcore::fmt::SerialWriter;

    let mut w = SerialWriter(serial);
    let _ = writeln!(w, "Memory Map:");
    let _ = writeln!(w, "  Kernel:  0x{:08X} - 0x{:08X} ({} MB)",
                     KERNEL_BASE, KERNEL_END, KERNEL_REGION_SIZE / 1024 / 1024);
    let _ = writeln!(w, "  Heap:    0x{:08X} - 0x{:08X} ({} MB)",
                     HEAP_BASE, HEAP_END, HEAP_SIZE / 1024 / 1024);
    let _ = writeln!(w, "  GPU:     0x{:08X} - 0x{:08X} ({} MB)",
                     GPU_MEM_BASE, RAM_SIZE, GPU_MEM_SIZE / 1024 / 1024);
}
