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
//! 0x20    4     ROM address (0 if no ROM)
//! 0x24    4     ROM size in bytes
//! 0x28    32    ROM title (null-terminated)
//! 0x48    4     Boot type (0=raw, 1=partition)
//! 0x4C    4     Boot drive
//! 0x50    4     Partition start LBA (if boot_type=1)
//! ```

/// Magic value: 'GBOY' in little-endian
pub const BOOT_MAGIC: u32 = 0x594F4247;

/// Default boot info address
pub const BOOT_INFO_ADDR: u32 = 0x500;

/// Boot types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BootType {
    /// Booting from raw media (floppy, CD, USB)
    Raw = 0,
    /// Booting from a partition (installed on hard disk)
    Partition = 1,
}

impl From<u32> for BootType {
    fn from(value: u32) -> Self {
        match value {
            1 => BootType::Partition,
            _ => BootType::Raw,
        }
    }
}

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
    /// ROM address (0 if no ROM loaded)
    pub rom_addr: u32,
    /// ROM size in bytes
    pub rom_size: u32,
    /// Boot type (raw or partition)
    pub boot_type: BootType,
    /// Boot drive (BIOS drive number)
    pub boot_drive: u32,
    /// Partition start LBA (if booting from partition)
    pub partition_start: u32,
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
    pub rom_addr: u32,
    pub rom_size: u32,
    pub rom_title: [u8; 32],
    pub boot_type: u32,
    pub boot_drive: u32,
    pub partition_start: u32,
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
            rom_addr: raw.rom_addr,
            rom_size: raw.rom_size,
            boot_type: BootType::from(raw.boot_type),
            boot_drive: raw.boot_drive,
            partition_start: raw.partition_start,
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

    /// Check if a ROM is loaded
    pub fn has_rom(&self) -> bool {
        self.rom_addr != 0 && self.rom_size > 0
    }

    /// Check if booting from installed partition
    pub fn is_partition_boot(&self) -> bool {
        self.boot_type == BootType::Partition
    }

    /// Get ROM as a slice
    ///
    /// # Safety
    ///
    /// Caller must ensure rom_addr points to valid memory
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

    /// Get ROM title as string
    pub unsafe fn rom_title(&self) -> &str {
        let raw = &*(BOOT_INFO_ADDR as *const RawBootInfo);
        let title_bytes = &raw.rom_title;

        // Find null terminator
        let len = title_bytes.iter()
            .position(|&b| b == 0)
            .unwrap_or(32);

        core::str::from_utf8_unchecked(&title_bytes[..len])
    }
}

/// Get boot info from default address
///
/// # Safety
///
/// Must be called after stage2 bootloader has set up the boot info structure.
pub unsafe fn get_boot_info() -> Option<BootInfo> {
    let info = BootInfo::from_ptr(BOOT_INFO_ADDR as *const u8);
    if info.verify_magic() {
        Some(info)
    } else {
        None
    }
}

// ============================================================================
// E820 Memory Map Types
// ============================================================================

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
    pub base: u64,
    pub length: u64,
    pub region_type: u32,
    pub acpi_extended: u32,
}

impl E820Entry {
    pub fn entry_type(&self) -> E820Type {
        match self.region_type {
            1 => E820Type::Usable,
            2 => E820Type::Reserved,
            3 => E820Type::AcpiReclaimable,
            4 => E820Type::AcpiNvs,
            5 => E820Type::BadMemory,
            _ => E820Type::Reserved,
        }
    }

    pub fn memory_type(&self) -> E820Type {
        self.entry_type()
    }

    pub fn start(&self) -> u64 {
        self.base
    }

    pub fn end(&self) -> u64 {
        self.base + self.length
    }
}

/// E820 Memory Map
///
/// The first 4 bytes at the map address contain the entry count,
/// followed by the entries.
pub struct E820Map {
    pub count: u32,
    pub entries_ptr: *const E820Entry,
}

impl E820Map {
    /// Create E820Map from address
    ///
    /// # Safety
    ///
    /// The address must point to a valid E820 map created by the bootloader
    pub unsafe fn from_addr(addr: u32) -> Self {
        let count = *(addr as *const u32);
        let entries_ptr = (addr + 4) as *const E820Entry;
        Self { count, entries_ptr }
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.count as usize
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get entry by index
    pub fn get(&self, index: usize) -> Option<E820Entry> {
        if index < self.count as usize {
            unsafe {
                Some(core::ptr::read_unaligned(self.entries_ptr.add(index)))
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
        if entry.is_some() {
            self.index += 1;
        }
        entry
    }
}
