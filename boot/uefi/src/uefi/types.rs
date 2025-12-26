//! Core UEFI type definitions
//! 
//! These match the UEFI specification exactly. No external dependencies.

#![allow(non_camel_case_types)]
#![allow(dead_code)]

use core::ffi::c_void;

// =============================================================================
// Fundamental Types
// =============================================================================

pub type EFI_HANDLE = *mut c_void;
pub type EFI_EVENT = *mut c_void;
pub type EFI_STATUS = usize;
pub type EFI_TPL = usize;
pub type EFI_LBA = u64;
pub type EFI_PHYSICAL_ADDRESS = u64;
pub type EFI_VIRTUAL_ADDRESS = u64;

pub type BOOLEAN = u8;
pub type CHAR16 = u16;
pub type UINTN = usize;
pub type INTN = isize;

// =============================================================================
// Status Codes
// =============================================================================

pub const EFI_SUCCESS: EFI_STATUS = 0;
pub const EFI_LOAD_ERROR: EFI_STATUS = 1;
pub const EFI_INVALID_PARAMETER: EFI_STATUS = 2;
pub const EFI_UNSUPPORTED: EFI_STATUS = 3;
pub const EFI_BAD_BUFFER_SIZE: EFI_STATUS = 4;
pub const EFI_BUFFER_TOO_SMALL: EFI_STATUS = 5;
pub const EFI_NOT_READY: EFI_STATUS = 6;
pub const EFI_DEVICE_ERROR: EFI_STATUS = 7;
pub const EFI_WRITE_PROTECTED: EFI_STATUS = 8;
pub const EFI_OUT_OF_RESOURCES: EFI_STATUS = 9;
pub const EFI_NOT_FOUND: EFI_STATUS = 14;

// High bit set = error
pub const EFI_ERROR_BIT: EFI_STATUS = 1 << (core::mem::size_of::<EFI_STATUS>() * 8 - 1);

#[inline]
pub fn efi_error(status: EFI_STATUS) -> bool {
    (status & EFI_ERROR_BIT) != 0
}

// =============================================================================
// GUIDs
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct EFI_GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl EFI_GUID {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self { data1, data2, data3, data4 }
    }
}

// Standard Protocol GUIDs
pub const EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL_GUID: EFI_GUID = EFI_GUID::new(
    0x387477c2, 0x69c7, 0x11d2,
    [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]
);

pub const EFI_SIMPLE_TEXT_INPUT_PROTOCOL_GUID: EFI_GUID = EFI_GUID::new(
    0x387477c1, 0x69c7, 0x11d2,
    [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]
);

pub const EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID: EFI_GUID = EFI_GUID::new(
    0x9042a9de, 0x23dc, 0x4a38,
    [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a]
);

pub const EFI_LOADED_IMAGE_PROTOCOL_GUID: EFI_GUID = EFI_GUID::new(
    0x5B1B31A1, 0x9562, 0x11d2,
    [0x8E, 0x3F, 0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B]
);

pub const EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID: EFI_GUID = EFI_GUID::new(
    0x964e5b22, 0x6459, 0x11d2,
    [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]
);

pub const EFI_FILE_INFO_GUID: EFI_GUID = EFI_GUID::new(
    0x09576e92, 0x6d3f, 0x11d2,
    [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]
);

// =============================================================================
// Memory Types
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EFI_MEMORY_TYPE {
    ReservedMemoryType = 0,
    LoaderCode = 1,
    LoaderData = 2,
    BootServicesCode = 3,
    BootServicesData = 4,
    RuntimeServicesCode = 5,
    RuntimeServicesData = 6,
    ConventionalMemory = 7,
    UnusableMemory = 8,
    ACPIReclaimMemory = 9,
    ACPIMemoryNVS = 10,
    MemoryMappedIO = 11,
    MemoryMappedIOPortSpace = 12,
    PalCode = 13,
    PersistentMemory = 14,
    MaxMemoryType = 15,
}

// Memory attribute bits
pub const EFI_MEMORY_UC: u64 = 0x0000000000000001;
pub const EFI_MEMORY_WC: u64 = 0x0000000000000002;
pub const EFI_MEMORY_WT: u64 = 0x0000000000000004;
pub const EFI_MEMORY_WB: u64 = 0x0000000000000008;
pub const EFI_MEMORY_UCE: u64 = 0x0000000000000010;
pub const EFI_MEMORY_WP: u64 = 0x0000000000001000;
pub const EFI_MEMORY_RP: u64 = 0x0000000000002000;
pub const EFI_MEMORY_XP: u64 = 0x0000000000004000;
pub const EFI_MEMORY_RUNTIME: u64 = 0x8000000000000000;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EFI_MEMORY_DESCRIPTOR {
    pub memory_type: u32,
    pub physical_start: EFI_PHYSICAL_ADDRESS,
    pub virtual_start: EFI_VIRTUAL_ADDRESS,
    pub number_of_pages: u64,
    pub attribute: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EFI_ALLOCATE_TYPE {
    AllocateAnyPages = 0,
    AllocateMaxAddress = 1,
    AllocateAddress = 2,
    MaxAllocateType = 3,
}

// =============================================================================
// Table Headers
// =============================================================================

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EFI_TABLE_HEADER {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    pub reserved: u32,
}

// Table signatures
pub const EFI_SYSTEM_TABLE_SIGNATURE: u64 = 0x5453595320494249; // "IBI SYST"
pub const EFI_BOOT_SERVICES_SIGNATURE: u64 = 0x56524553544f4f42; // "BOOTSERV"
pub const EFI_RUNTIME_SERVICES_SIGNATURE: u64 = 0x56524553544e5552; // "RUNTSERV"

// =============================================================================
// Time
// =============================================================================

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct EFI_TIME {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub pad1: u8,
    pub nanosecond: u32,
    pub time_zone: i16,
    pub daylight: u8,
    pub pad2: u8,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EFI_TIME_CAPABILITIES {
    pub resolution: u32,
    pub accuracy: u32,
    pub sets_to_zero: BOOLEAN,
}
