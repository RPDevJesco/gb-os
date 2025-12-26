//! Memory Management
//! 
//! UEFI memory allocation, memory map handling, and page management.

use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, Ordering};
use crate::uefi::*;
use crate::println;

/// Global boot services pointer - set during initialization
static BOOT_SERVICES: AtomicPtr<EFI_BOOT_SERVICES> = AtomicPtr::new(core::ptr::null_mut());

/// Initialize memory services
/// 
/// # Safety
/// Caller must ensure boot_services is a valid pointer
pub unsafe fn init(boot_services: *mut EFI_BOOT_SERVICES) {
    BOOT_SERVICES.store(boot_services, Ordering::Release);
}

/// Get boot services pointer
fn bs() -> *mut EFI_BOOT_SERVICES {
    let ptr = BOOT_SERVICES.load(Ordering::Acquire);
    assert!(!ptr.is_null(), "Memory not initialized");
    ptr
}

// =============================================================================
// Page Allocation
// =============================================================================

/// Size of a UEFI page (4KB)
#[allow(dead_code)]
pub const PAGE_SIZE: usize = 4096;

/// Allocate pages of a specific memory type
#[allow(dead_code)]
pub fn allocate_pages(
    pages: usize,
    memory_type: EFI_MEMORY_TYPE,
) -> Result<EFI_PHYSICAL_ADDRESS, EFI_STATUS> {
    let mut address: EFI_PHYSICAL_ADDRESS = 0;
    
    let status = unsafe {
        ((*bs()).allocate_pages)(
            EFI_ALLOCATE_TYPE::AllocateAnyPages,
            memory_type,
            pages,
            &mut address,
        )
    };
    
    if status == EFI_SUCCESS {
        Ok(address)
    } else {
        Err(status)
    }
}

/// Allocate pages below a specific address
#[allow(dead_code)]
pub fn allocate_pages_below(
    pages: usize,
    max_address: EFI_PHYSICAL_ADDRESS,
    memory_type: EFI_MEMORY_TYPE,
) -> Result<EFI_PHYSICAL_ADDRESS, EFI_STATUS> {
    let mut address = max_address;
    
    let status = unsafe {
        ((*bs()).allocate_pages)(
            EFI_ALLOCATE_TYPE::AllocateMaxAddress,
            memory_type,
            pages,
            &mut address,
        )
    };
    
    if status == EFI_SUCCESS {
        Ok(address)
    } else {
        Err(status)
    }
}

/// Allocate pages at a specific address
#[allow(dead_code)]
pub fn allocate_pages_at(
    address: EFI_PHYSICAL_ADDRESS,
    pages: usize,
    memory_type: EFI_MEMORY_TYPE,
) -> Result<(), EFI_STATUS> {
    let mut addr = address;
    
    let status = unsafe {
        ((*bs()).allocate_pages)(
            EFI_ALLOCATE_TYPE::AllocateAddress,
            memory_type,
            pages,
            &mut addr,
        )
    };
    
    if status == EFI_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

/// Free previously allocated pages
#[allow(dead_code)]
pub fn free_pages(address: EFI_PHYSICAL_ADDRESS, pages: usize) -> Result<(), EFI_STATUS> {
    let status = unsafe { ((*bs()).free_pages)(address, pages) };
    
    if status == EFI_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

// =============================================================================
// Pool Allocation (heap-like)
// =============================================================================

/// Allocate memory from pool
pub fn allocate_pool(size: usize, memory_type: EFI_MEMORY_TYPE) -> Result<*mut c_void, EFI_STATUS> {
    let mut buffer: *mut c_void = core::ptr::null_mut();
    
    let status = unsafe { ((*bs()).allocate_pool)(memory_type, size, &mut buffer) };
    
    if status == EFI_SUCCESS {
        Ok(buffer)
    } else {
        Err(status)
    }
}

/// Free pool memory
pub fn free_pool(buffer: *mut c_void) -> Result<(), EFI_STATUS> {
    let status = unsafe { ((*bs()).free_pool)(buffer) };
    
    if status == EFI_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

// =============================================================================
// Memory Map
// =============================================================================

/// Memory map entry wrapper
#[derive(Clone, Copy)]
pub struct MemoryRegion {
    pub memory_type: EFI_MEMORY_TYPE,
    pub physical_start: u64,
    #[allow(dead_code)]
    pub virtual_start: u64,
    pub page_count: u64,
    #[allow(dead_code)]
    pub attributes: u64,
}

impl MemoryRegion {
    pub fn size(&self) -> u64 {
        self.page_count * PAGE_SIZE as u64
    }
    
    pub fn end(&self) -> u64 {
        self.physical_start + self.size()
    }
    
    pub fn is_usable(&self) -> bool {
        matches!(
            self.memory_type,
            EFI_MEMORY_TYPE::ConventionalMemory
                | EFI_MEMORY_TYPE::BootServicesCode
                | EFI_MEMORY_TYPE::BootServicesData
                | EFI_MEMORY_TYPE::LoaderCode
                | EFI_MEMORY_TYPE::LoaderData
        )
    }
}

/// Complete memory map
pub struct MemoryMap {
    buffer: *mut u8,
    #[allow(dead_code)]
    buffer_size: usize,
    pub map_key: UINTN,
    pub descriptor_size: usize,
    #[allow(dead_code)]
    pub descriptor_version: u32,
    pub entry_count: usize,
}

impl MemoryMap {
    /// Get the memory map from UEFI
    pub fn get() -> Result<Self, EFI_STATUS> {
        let mut map_size: UINTN = 0;
        let mut map_key: UINTN = 0;
        let mut descriptor_size: UINTN = 0;
        let mut descriptor_version: u32 = 0;
        
        // First call to get required buffer size
        let status = unsafe {
            ((*bs()).get_memory_map)(
                &mut map_size,
                core::ptr::null_mut(),
                &mut map_key,
                &mut descriptor_size,
                &mut descriptor_version,
            )
        };
        
        // Should return BUFFER_TOO_SMALL
        if status != EFI_BUFFER_TOO_SMALL {
            return Err(status);
        }
        
        // Add extra space for map changes during allocation
        map_size += descriptor_size * 4;
        
        // Allocate buffer
        let buffer = allocate_pool(map_size, EFI_MEMORY_TYPE::LoaderData)? as *mut u8;
        
        // Get actual memory map
        let status = unsafe {
            ((*bs()).get_memory_map)(
                &mut map_size,
                buffer as *mut EFI_MEMORY_DESCRIPTOR,
                &mut map_key,
                &mut descriptor_size,
                &mut descriptor_version,
            )
        };
        
        if status != EFI_SUCCESS {
            let _ = free_pool(buffer as *mut c_void);
            return Err(status);
        }
        
        let entry_count = map_size / descriptor_size;
        
        Ok(Self {
            buffer,
            buffer_size: map_size,
            map_key,
            descriptor_size,
            descriptor_version,
            entry_count,
        })
    }
    
    /// Get a memory region by index
    pub fn get_region(&self, index: usize) -> Option<MemoryRegion> {
        if index >= self.entry_count {
            return None;
        }
        
        let offset = index * self.descriptor_size;
        let desc = unsafe { &*(self.buffer.add(offset) as *const EFI_MEMORY_DESCRIPTOR) };
        
        // Convert u32 to EFI_MEMORY_TYPE
        let memory_type = unsafe { core::mem::transmute::<u32, EFI_MEMORY_TYPE>(desc.memory_type) };
        
        Some(MemoryRegion {
            memory_type,
            physical_start: desc.physical_start,
            virtual_start: desc.virtual_start,
            page_count: desc.number_of_pages,
            attributes: desc.attribute,
        })
    }
    
    /// Iterator over memory regions
    pub fn iter(&self) -> MemoryMapIter<'_> {
        MemoryMapIter {
            map: self,
            index: 0,
        }
    }
    
    /// Find total usable memory
    pub fn total_usable_memory(&self) -> u64 {
        self.iter()
            .filter(|r| r.is_usable())
            .map(|r| r.size())
            .sum()
    }
    
    /// Find largest contiguous usable region
    #[allow(dead_code)]
    pub fn largest_usable_region(&self) -> Option<MemoryRegion> {
        self.iter()
            .filter(|r| r.is_usable())
            .max_by_key(|r| r.size())
    }
    
    /// Print memory map summary
    pub fn print_summary(&self) {
        println!("=== Memory Map ===");
        println!("Entries: {}", self.entry_count);
        println!("Total Usable: {} MB", self.total_usable_memory() / (1024 * 1024));
        println!();
        
        for (i, region) in self.iter().enumerate() {
            let type_str = match region.memory_type {
                EFI_MEMORY_TYPE::ReservedMemoryType => "Reserved",
                EFI_MEMORY_TYPE::LoaderCode => "LoaderCode",
                EFI_MEMORY_TYPE::LoaderData => "LoaderData",
                EFI_MEMORY_TYPE::BootServicesCode => "BootSvcCode",
                EFI_MEMORY_TYPE::BootServicesData => "BootSvcData",
                EFI_MEMORY_TYPE::RuntimeServicesCode => "RuntimeCode",
                EFI_MEMORY_TYPE::RuntimeServicesData => "RuntimeData",
                EFI_MEMORY_TYPE::ConventionalMemory => "Conventional",
                EFI_MEMORY_TYPE::UnusableMemory => "Unusable",
                EFI_MEMORY_TYPE::ACPIReclaimMemory => "ACPIReclaim",
                EFI_MEMORY_TYPE::ACPIMemoryNVS => "ACPI NVS",
                EFI_MEMORY_TYPE::MemoryMappedIO => "MMIO",
                EFI_MEMORY_TYPE::MemoryMappedIOPortSpace => "MMIO Port",
                EFI_MEMORY_TYPE::PalCode => "PAL Code",
                EFI_MEMORY_TYPE::PersistentMemory => "Persistent",
                _ => "Unknown",
            };
            
            println!(
                "{:3}: {:016X}-{:016X} {:12} ({} pages)",
                i,
                region.physical_start,
                region.end() - 1,
                type_str,
                region.page_count
            );
        }
        println!();
    }
}

impl Drop for MemoryMap {
    fn drop(&mut self) {
        let _ = free_pool(self.buffer as *mut c_void);
    }
}

/// Iterator over memory map entries
pub struct MemoryMapIter<'a> {
    map: &'a MemoryMap,
    index: usize,
}

impl<'a> Iterator for MemoryMapIter<'a> {
    type Item = MemoryRegion;
    
    fn next(&mut self) -> Option<Self::Item> {
        let region = self.map.get_region(self.index)?;
        self.index += 1;
        Some(region)
    }
}

// =============================================================================
// Utility Functions
// =============================================================================

/// Copy memory (uses UEFI boot services)
#[allow(dead_code)]
pub fn copy_mem(dest: *mut c_void, src: *const c_void, size: usize) {
    unsafe {
        ((*bs()).copy_mem)(dest, src, size);
    }
}

/// Set memory (uses UEFI boot services)
#[allow(dead_code)]
pub fn set_mem(dest: *mut c_void, size: usize, value: u8) {
    unsafe {
        ((*bs()).set_mem)(dest, size, value);
    }
}

/// Zero memory
#[allow(dead_code)]
pub fn zero_mem(dest: *mut c_void, size: usize) {
    set_mem(dest, size, 0);
}
