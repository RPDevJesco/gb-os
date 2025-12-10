//! Boot Information Parser (Extended for GameBoy Mode)
//!
//! Parses the boot info structure created by stage2 bootloader at 0x500.
//!
//! # Boot Info Structure Layout (at 0x500)
//!
//! ```text
//! Offset  Size  Field
//! 0x00    4     Magic ('RUST' = 0x54535552 or 'GBOY' = 0x594F4247)
//! 0x04    4     E820 map address
//! 0x08    4     VESA enabled (0 or 1)
//! 0x0C    4     Framebuffer address
//! 0x10    4     Screen width
//! 0x14    4     Screen height
//! 0x18    4     Bits per pixel
//! 0x1C    4     Pitch (bytes per scanline)
//! 0x20    4     ROM address (0 if no game loaded)
//! 0x24    4     ROM size in bytes (0 if no game)
//! 0x28    32    ROM title (null-terminated, for GameBoy mode)
//! ```

/// Magic value: 'RUST' in little-endian (standard Rustacean OS)
pub const BOOT_MAGIC: u32 = 0x54535552;

/// Magic value: 'GBOY' in little-endian (GameBoy mode)
pub const BOOT_MAGIC_GAMEBOY: u32 = 0x594F4247;

/// Boot information passed from bootloader to kernel
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BootInfo {
    /// Magic number ('RUST' or 'GBOY')
    pub magic: u32,
    /// Address of E820 memory map
    pub e820_map_addr: u32,
    /// Whether VESA mode is enabled (vs VGA text)
    pub vesa_enabled: bool,
    /// Physical address of framebuffer
    pub framebuffer_addr: u32,
    /// Screen width in pixels (or columns for text mode)
    pub screen_width: u32,
    /// Screen height in pixels (or rows for text mode)
    pub screen_height: u32,
    /// Bits per pixel (or 16 for text mode = 2 bytes per cell)
    pub bits_per_pixel: u32,
    /// Pitch: bytes per scanline
    pub pitch: u32,
    /// ROM load address (0 if no ROM loaded) - GameBoy mode only
    pub rom_addr: u32,
    /// ROM size in bytes (0 if no ROM loaded) - GameBoy mode only
    pub rom_size: u32,
}

/// Raw boot info structure as stored in memory (extended)
#[repr(C, packed)]
pub struct RawBootInfo {
    pub magic: u32,
    pub e820_map_addr: u32,
    pub vesa_enabled: u32,
    pub framebuffer_addr: u32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub bits_per_pixel: u32,
    pub pitch: u32,
    pub rom_addr: u32,
    pub rom_size: u32,
    pub rom_title: [u8; 32],
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
            vesa_enabled: raw.vesa_enabled != 0,
            framebuffer_addr: raw.framebuffer_addr,
            screen_width: raw.screen_width,
            screen_height: raw.screen_height,
            bits_per_pixel: raw.bits_per_pixel,
            pitch: raw.pitch,
            rom_addr: raw.rom_addr,
            rom_size: raw.rom_size,
        }
    }
    
    /// Verify the boot magic is correct
    pub fn verify_magic(&self) -> bool {
        self.magic == BOOT_MAGIC || self.magic == BOOT_MAGIC_GAMEBOY
    }

    /// Check if we're in GameBoy mode (ROM loaded by bootloader)
    pub fn is_gameboy_mode(&self) -> bool {
        self.magic == BOOT_MAGIC_GAMEBOY && self.rom_addr != 0 && self.rom_size != 0
    }

    /// Check if a ROM was loaded
    pub fn has_rom(&self) -> bool {
        self.rom_addr != 0 && self.rom_size != 0
    }

    /// Get ROM data as a slice
    ///
    /// # Safety
    /// 
    /// ROM address must be valid and ROM must have been loaded by bootloader
    pub unsafe fn rom_slice(&self) -> Option<&'static [u8]> {
        if self.has_rom() {
            Some(core::slice::from_raw_parts(
                self.rom_addr as *const u8,
                self.rom_size as usize
            ))
        } else {
            None
        }
    }

    /// Get ROM title from boot info (GameBoy mode only)
    ///
    /// # Safety
    ///
    /// Must only be called if boot info is valid
    pub unsafe fn rom_title(&self) -> &'static str {
        if !self.has_rom() {
            return "";
        }
        
        let raw = &*(0x500 as *const RawBootInfo);
        let title_bytes = &raw.rom_title;
        
        // Find null terminator
        let len = title_bytes.iter()
            .position(|&b| b == 0)
            .unwrap_or(32);
        
        // Filter to printable ASCII
        core::str::from_utf8(&title_bytes[..len]).unwrap_or("Unknown")
    }
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

impl E820Entry {
    /// Get the memory type
    pub fn memory_type(&self) -> E820Type {
        match self.region_type {
            1 => E820Type::Usable,
            2 => E820Type::Reserved,
            3 => E820Type::AcpiReclaimable,
            4 => E820Type::AcpiNvs,
            5 => E820Type::BadMemory,
            _ => E820Type::Reserved, // Treat unknown as reserved
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
    /// Pointer to entry array
    entries_ptr: *const E820Entry,
    /// Number of entries
    count: usize,
}

impl E820Map {
    /// Parse E820 map from address
    ///
    /// # Safety
    ///
    /// The address must point to a valid E820 map structure
    /// created by the stage2 bootloader.
    pub unsafe fn from_addr(addr: u32) -> Self {
        // First dword is entry count
        let count = *(addr as *const u32) as usize;
        let entries_ptr = (addr + 4) as *const E820Entry;
        
        E820Map { entries_ptr, count }
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if map is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get entry by index
    pub fn get(&self, index: usize) -> Option<E820Entry> {
        if index < self.count {
            unsafe {
                Some(*self.entries_ptr.add(index))
            }
        } else {
            None
        }
    }

    /// Iterate over entries
    pub fn iter(&self) -> E820MapIter {
        E820MapIter {
            map: self,
            index: 0,
        }
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
