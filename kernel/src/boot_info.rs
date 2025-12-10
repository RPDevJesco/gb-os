//! Boot Information Parser for RetroFutureGB
//!
//! Parses the boot info structure created by stage2 bootloader at 0x500.
//!
//! # Boot Info Structure Layout (at 0x500)
//!
//! ```text
//! Offset  Size  Field
//! 0x00    4     Magic ('GBOY' = 0x594F4247)
//! 0x04    4     E820 map address
//! 0x08    4     VGA mode (0x13 for mode 13h)
//! 0x0C    4     Framebuffer address (0xA0000)
//! 0x10    4     Screen width (320)
//! 0x14    4     Screen height (200)
//! 0x18    4     Bits per pixel (8)
//! 0x1C    4     Pitch (320)
//! ```

/// Magic value: 'GBOY' in little-endian
pub const BOOT_MAGIC: u32 = 0x594F4247;

/// Boot information passed from bootloader to kernel
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BootInfo {
    /// Magic number ('GBOY')
    pub magic: u32,
    /// Address of E820 memory map
    pub e820_map_addr: u32,
    /// VGA mode (0x13 for 320x200x256)
    pub vga_mode: u32,
    /// Physical address of framebuffer
    pub framebuffer_addr: u32,
    /// Screen width in pixels
    pub screen_width: u32,
    /// Screen height in pixels
    pub screen_height: u32,
    /// Bits per pixel
    pub bits_per_pixel: u32,
    /// Pitch: bytes per scanline
    pub pitch: u32,
}

/// Raw boot info structure as stored in memory
#[repr(C, packed)]
pub struct RawBootInfo {
    pub magic: u32,
    pub e820_map_addr: u32,
    pub vga_mode: u32,
    pub framebuffer_addr: u32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub bits_per_pixel: u32,
    pub pitch: u32,
}

impl BootInfo {
    /// Parse boot info from raw pointer
    ///
    /// # Safety
    ///
    /// The pointer must point to a valid boot info structure
    /// created by the stage2 bootloader.
    pub unsafe fn from_ptr(ptr: *const u8) -> Self {
        let raw = &*(ptr as *const RawBootInfo);

        Self {
            magic: raw.magic,
            e820_map_addr: raw.e820_map_addr,
            vga_mode: raw.vga_mode,
            framebuffer_addr: raw.framebuffer_addr,
            screen_width: raw.screen_width,
            screen_height: raw.screen_height,
            bits_per_pixel: raw.bits_per_pixel,
            pitch: raw.pitch,
        }
    }

    /// Verify the boot magic is correct
    pub fn verify_magic(&self) -> bool {
        self.magic == BOOT_MAGIC
    }

    /// Check if we're in VGA mode 13h
    pub fn is_mode_13h(&self) -> bool {
        self.vga_mode == 0x13
    }
}

/// E820 memory region types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum E820Type {
    /// Usable RAM
    Usable = 1,
    /// Reserved by system
    Reserved = 2,
    /// ACPI reclaimable
    AcpiReclaimable = 3,
    /// ACPI NVS (non-volatile storage)
    AcpiNvs = 4,
    /// Bad memory
    BadMemory = 5,
}

/// E820 Memory Map Entry
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct E820Entry {
    /// Base address of memory region
    pub base: u64,
    /// Length of memory region in bytes
    pub length: u64,
    /// Type of memory region
    pub region_type: u32,
    /// ACPI 3.0 extended attributes (may be 0)
    pub acpi_attrs: u32,
}

impl E820Entry {
    /// Get the memory type
    pub fn memory_type(&self) -> E820Type {
        match self.region_type {
            1 => E820Type::Usable,
            2 => E820Type::Reserved,
            3 => E820Type::AcpiReclaimable,
            4 => E820Type::AcpiNvs,
            5 => E820Type::BadMemory,
            _ => E820Type::Reserved,
        }
    }

    /// Check if this region is usable RAM
    pub fn is_usable(&self) -> bool {
        self.region_type == 1
    }

    /// Get end address of this region
    pub fn end(&self) -> u64 {
        self.base + self.length
    }
}

/// E820 Memory Map
pub struct E820Map {
    entries_ptr: *const E820Entry,
    count: usize,
}

impl E820Map {
    /// Parse E820 map from address
    ///
    /// # Safety
    ///
    /// The address must point to a valid E820 map structure
    pub unsafe fn from_addr(addr: u32) -> Self {
        let count = *(addr as *const u32) as usize;
        let entries_ptr = (addr + 4) as *const E820Entry;
        E820Map { entries_ptr, count }
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get entry by index
    pub fn get(&self, index: usize) -> Option<E820Entry> {
        if index < self.count {
            unsafe { Some(*self.entries_ptr.add(index)) }
        } else {
            None
        }
    }

    /// Iterate over entries
    pub fn iter(&self) -> E820MapIter {
        E820MapIter { map: self, index: 0 }
    }
}

/// Iterator over E820 map entries
pub struct E820MapIter<'a> {
    map: &'a E820Map,
    index: usize,
}

impl<'a> Iterator for E820MapIter<'a> {
    type Item = E820Entry;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.map.get(self.index);
        self.index += 1;
        entry
    }
}
