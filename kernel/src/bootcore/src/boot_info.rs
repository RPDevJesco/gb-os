//! Boot information passed to loaded kernel/payload.
//!
//! This structure is placed at a known location and provides
//! the loaded payload with essential system information.

use crate::MemoryRegion;

/// Maximum number of memory regions we track.
pub const MAX_MEMORY_REGIONS: usize = 16;

/// Magic number to identify valid boot info.
pub const BOOT_INFO_MAGIC: u64 = 0x5255_5354_424F_4F54; // "RUSTBOOT"

/// Boot information structure passed to payload.
///
/// This is placed at a known address and passed to the loaded
/// kernel or application.
#[repr(C)]
#[derive(Clone)]
pub struct BootInfo {
    /// Magic number for validation (BOOT_INFO_MAGIC).
    pub magic: u64,

    /// Version of this structure (for compatibility).
    pub version: u32,

    /// Size of this structure in bytes.
    pub size: u32,

    /// Platform identifier (null-terminated ASCII).
    pub platform: [u8; 32],

    /// Entry point where payload was loaded.
    pub entry_point: u64,

    /// Physical address where payload was loaded.
    pub load_address: u64,

    /// Size of loaded payload in bytes.
    pub payload_size: u64,

    /// Stack pointer set up for payload.
    pub stack_pointer: u64,

    /// Device tree blob address (0 if none).
    pub dtb_address: u64,

    /// Device tree blob size (0 if none).
    pub dtb_size: u64,

    /// Command line address (0 if none).
    pub cmdline_address: u64,

    /// Command line length (0 if none).
    pub cmdline_len: u32,

    /// Number of valid memory regions.
    pub memory_region_count: u32,

    /// Memory map.
    pub memory_regions: [MemoryRegion; MAX_MEMORY_REGIONS],

    /// Framebuffer base address (0 if none).
    pub framebuffer_address: u64,

    /// Framebuffer width in pixels.
    pub framebuffer_width: u32,

    /// Framebuffer height in pixels.
    pub framebuffer_height: u32,

    /// Framebuffer pitch (bytes per row).
    pub framebuffer_pitch: u32,

    /// Framebuffer bits per pixel.
    pub framebuffer_bpp: u32,

    /// Reserved for future use.
    pub _reserved: [u64; 8],
}

impl BootInfo {
    /// Create a new empty boot info structure.
    pub const fn new() -> Self {
        Self {
            magic: BOOT_INFO_MAGIC,
            version: 1,
            size: core::mem::size_of::<Self>() as u32,
            platform: [0u8; 32],
            entry_point: 0,
            load_address: 0,
            payload_size: 0,
            stack_pointer: 0,
            dtb_address: 0,
            dtb_size: 0,
            cmdline_address: 0,
            cmdline_len: 0,
            memory_region_count: 0,
            memory_regions: [MemoryRegion {
                base: 0,
                size: 0,
                kind: crate::MemoryKind::Reserved,
            }; MAX_MEMORY_REGIONS],
            framebuffer_address: 0,
            framebuffer_width: 0,
            framebuffer_height: 0,
            framebuffer_pitch: 0,
            framebuffer_bpp: 0,
            _reserved: [0u64; 8],
        }
    }

    /// Set the platform name.
    pub fn set_platform(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(31);
        self.platform[..len].copy_from_slice(&bytes[..len]);
        self.platform[len] = 0; // Null terminate
    }

    /// Add a memory region.
    pub fn add_memory_region(&mut self, region: MemoryRegion) -> bool {
        if (self.memory_region_count as usize) < MAX_MEMORY_REGIONS {
            self.memory_regions[self.memory_region_count as usize] = region;
            self.memory_region_count += 1;
            true
        } else {
            false
        }
    }

    /// Validate the boot info structure.
    pub fn is_valid(&self) -> bool {
        self.magic == BOOT_INFO_MAGIC
            && self.version >= 1
            && self.size as usize == core::mem::size_of::<Self>()
    }
}

impl Default for BootInfo {
    fn default() -> Self {
        Self::new()
    }
}
