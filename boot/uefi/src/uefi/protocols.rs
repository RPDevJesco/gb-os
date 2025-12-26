//! UEFI Protocol Definitions
//! 
//! Console I/O, Graphics, and File System protocols.

#![allow(non_camel_case_types)]
#![allow(dead_code)]

use core::ffi::c_void;
use super::types::*;

// =============================================================================
// Simple Text Input Protocol
// =============================================================================

#[repr(C)]
pub struct EFI_INPUT_KEY {
    pub scan_code: u16,
    pub unicode_char: CHAR16,
}

pub type EFI_INPUT_RESET = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_INPUT_PROTOCOL,
    extended_verification: BOOLEAN,
) -> EFI_STATUS;

pub type EFI_INPUT_READ_KEY = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_INPUT_PROTOCOL,
    key: *mut EFI_INPUT_KEY,
) -> EFI_STATUS;

#[repr(C)]
pub struct EFI_SIMPLE_TEXT_INPUT_PROTOCOL {
    pub reset: EFI_INPUT_RESET,
    pub read_key_stroke: EFI_INPUT_READ_KEY,
    pub wait_for_key: EFI_EVENT,
}

// Scan codes
pub const SCAN_NULL: u16 = 0x0000;
pub const SCAN_UP: u16 = 0x0001;
pub const SCAN_DOWN: u16 = 0x0002;
pub const SCAN_RIGHT: u16 = 0x0003;
pub const SCAN_LEFT: u16 = 0x0004;
pub const SCAN_HOME: u16 = 0x0005;
pub const SCAN_END: u16 = 0x0006;
pub const SCAN_INSERT: u16 = 0x0007;
pub const SCAN_DELETE: u16 = 0x0008;
pub const SCAN_PAGE_UP: u16 = 0x0009;
pub const SCAN_PAGE_DOWN: u16 = 0x000A;
pub const SCAN_F1: u16 = 0x000B;
pub const SCAN_F2: u16 = 0x000C;
pub const SCAN_F10: u16 = 0x0014;
pub const SCAN_ESC: u16 = 0x0017;

// =============================================================================
// Simple Text Output Protocol
// =============================================================================

pub type EFI_TEXT_RESET = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    extended_verification: BOOLEAN,
) -> EFI_STATUS;

pub type EFI_TEXT_STRING = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    string: *const CHAR16,
) -> EFI_STATUS;

pub type EFI_TEXT_TEST_STRING = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    string: *const CHAR16,
) -> EFI_STATUS;

pub type EFI_TEXT_QUERY_MODE = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    mode_number: UINTN,
    columns: *mut UINTN,
    rows: *mut UINTN,
) -> EFI_STATUS;

pub type EFI_TEXT_SET_MODE = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    mode_number: UINTN,
) -> EFI_STATUS;

pub type EFI_TEXT_SET_ATTRIBUTE = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    attribute: UINTN,
) -> EFI_STATUS;

pub type EFI_TEXT_CLEAR_SCREEN = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
) -> EFI_STATUS;

pub type EFI_TEXT_SET_CURSOR_POSITION = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    column: UINTN,
    row: UINTN,
) -> EFI_STATUS;

pub type EFI_TEXT_ENABLE_CURSOR = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    visible: BOOLEAN,
) -> EFI_STATUS;

#[repr(C)]
pub struct SIMPLE_TEXT_OUTPUT_MODE {
    pub max_mode: i32,
    pub mode: i32,
    pub attribute: i32,
    pub cursor_column: i32,
    pub cursor_row: i32,
    pub cursor_visible: BOOLEAN,
}

#[repr(C)]
pub struct EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL {
    pub reset: EFI_TEXT_RESET,
    pub output_string: EFI_TEXT_STRING,
    pub test_string: EFI_TEXT_TEST_STRING,
    pub query_mode: EFI_TEXT_QUERY_MODE,
    pub set_mode: EFI_TEXT_SET_MODE,
    pub set_attribute: EFI_TEXT_SET_ATTRIBUTE,
    pub clear_screen: EFI_TEXT_CLEAR_SCREEN,
    pub set_cursor_position: EFI_TEXT_SET_CURSOR_POSITION,
    pub enable_cursor: EFI_TEXT_ENABLE_CURSOR,
    pub mode: *mut SIMPLE_TEXT_OUTPUT_MODE,
}

// Text colors
pub const EFI_BLACK: UINTN = 0x00;
pub const EFI_BLUE: UINTN = 0x01;
pub const EFI_GREEN: UINTN = 0x02;
pub const EFI_CYAN: UINTN = 0x03;
pub const EFI_RED: UINTN = 0x04;
pub const EFI_MAGENTA: UINTN = 0x05;
pub const EFI_BROWN: UINTN = 0x06;
pub const EFI_LIGHTGRAY: UINTN = 0x07;
pub const EFI_DARKGRAY: UINTN = 0x08;
pub const EFI_LIGHTBLUE: UINTN = 0x09;
pub const EFI_LIGHTGREEN: UINTN = 0x0A;
pub const EFI_LIGHTCYAN: UINTN = 0x0B;
pub const EFI_LIGHTRED: UINTN = 0x0C;
pub const EFI_LIGHTMAGENTA: UINTN = 0x0D;
pub const EFI_YELLOW: UINTN = 0x0E;
pub const EFI_WHITE: UINTN = 0x0F;

// Background colors (shift left by 4)
pub const EFI_BACKGROUND_BLACK: UINTN = 0x00;
pub const EFI_BACKGROUND_BLUE: UINTN = 0x10;
pub const EFI_BACKGROUND_GREEN: UINTN = 0x20;
pub const EFI_BACKGROUND_CYAN: UINTN = 0x30;
pub const EFI_BACKGROUND_RED: UINTN = 0x40;
pub const EFI_BACKGROUND_MAGENTA: UINTN = 0x50;
pub const EFI_BACKGROUND_BROWN: UINTN = 0x60;
pub const EFI_BACKGROUND_LIGHTGRAY: UINTN = 0x70;

#[inline]
pub const fn efi_text_attr(foreground: UINTN, background: UINTN) -> UINTN {
    foreground | background
}

// =============================================================================
// Graphics Output Protocol (GOP)
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EFI_GRAPHICS_PIXEL_FORMAT {
    PixelRedGreenBlueReserved8BitPerColor = 0,
    PixelBlueGreenRedReserved8BitPerColor = 1,
    PixelBitMask = 2,
    PixelBltOnly = 3,
    PixelFormatMax = 4,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EFI_PIXEL_BITMASK {
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub reserved_mask: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EFI_GRAPHICS_OUTPUT_MODE_INFORMATION {
    pub version: u32,
    pub horizontal_resolution: u32,
    pub vertical_resolution: u32,
    pub pixel_format: EFI_GRAPHICS_PIXEL_FORMAT,
    pub pixel_information: EFI_PIXEL_BITMASK,
    pub pixels_per_scan_line: u32,
}

#[repr(C)]
pub struct EFI_GRAPHICS_OUTPUT_PROTOCOL_MODE {
    pub max_mode: u32,
    pub mode: u32,
    pub info: *mut EFI_GRAPHICS_OUTPUT_MODE_INFORMATION,
    pub size_of_info: UINTN,
    pub frame_buffer_base: EFI_PHYSICAL_ADDRESS,
    pub frame_buffer_size: UINTN,
}

pub type EFI_GRAPHICS_OUTPUT_PROTOCOL_QUERY_MODE = unsafe extern "efiapi" fn(
    this: *mut EFI_GRAPHICS_OUTPUT_PROTOCOL,
    mode_number: u32,
    size_of_info: *mut UINTN,
    info: *mut *mut EFI_GRAPHICS_OUTPUT_MODE_INFORMATION,
) -> EFI_STATUS;

pub type EFI_GRAPHICS_OUTPUT_PROTOCOL_SET_MODE = unsafe extern "efiapi" fn(
    this: *mut EFI_GRAPHICS_OUTPUT_PROTOCOL,
    mode_number: u32,
) -> EFI_STATUS;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct EFI_GRAPHICS_OUTPUT_BLT_PIXEL {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub reserved: u8,
}

pub type EFI_GRAPHICS_OUTPUT_PROTOCOL_BLT = unsafe extern "efiapi" fn(
    this: *mut EFI_GRAPHICS_OUTPUT_PROTOCOL,
    blt_buffer: *mut EFI_GRAPHICS_OUTPUT_BLT_PIXEL,
    blt_operation: u32,
    source_x: UINTN,
    source_y: UINTN,
    dest_x: UINTN,
    dest_y: UINTN,
    width: UINTN,
    height: UINTN,
    delta: UINTN,
) -> EFI_STATUS;

#[repr(C)]
pub struct EFI_GRAPHICS_OUTPUT_PROTOCOL {
    pub query_mode: EFI_GRAPHICS_OUTPUT_PROTOCOL_QUERY_MODE,
    pub set_mode: EFI_GRAPHICS_OUTPUT_PROTOCOL_SET_MODE,
    pub blt: EFI_GRAPHICS_OUTPUT_PROTOCOL_BLT,
    pub mode: *mut EFI_GRAPHICS_OUTPUT_PROTOCOL_MODE,
}

// BLT operations
pub const EFI_BLT_VIDEO_FILL: u32 = 0;
pub const EFI_BLT_VIDEO_TO_BLT_BUFFER: u32 = 1;
pub const EFI_BLT_BUFFER_TO_VIDEO: u32 = 2;
pub const EFI_BLT_VIDEO_TO_VIDEO: u32 = 3;

// =============================================================================
// Simple File System Protocol
// =============================================================================

pub type EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_OPEN_VOLUME = unsafe extern "efiapi" fn(
    this: *mut EFI_SIMPLE_FILE_SYSTEM_PROTOCOL,
    root: *mut *mut EFI_FILE_PROTOCOL,
) -> EFI_STATUS;

#[repr(C)]
pub struct EFI_SIMPLE_FILE_SYSTEM_PROTOCOL {
    pub revision: u64,
    pub open_volume: EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_OPEN_VOLUME,
}

pub type EFI_FILE_OPEN = unsafe extern "efiapi" fn(
    this: *mut EFI_FILE_PROTOCOL,
    new_handle: *mut *mut EFI_FILE_PROTOCOL,
    file_name: *const CHAR16,
    open_mode: u64,
    attributes: u64,
) -> EFI_STATUS;

pub type EFI_FILE_CLOSE = unsafe extern "efiapi" fn(
    this: *mut EFI_FILE_PROTOCOL,
) -> EFI_STATUS;

pub type EFI_FILE_READ = unsafe extern "efiapi" fn(
    this: *mut EFI_FILE_PROTOCOL,
    buffer_size: *mut UINTN,
    buffer: *mut c_void,
) -> EFI_STATUS;

pub type EFI_FILE_WRITE = unsafe extern "efiapi" fn(
    this: *mut EFI_FILE_PROTOCOL,
    buffer_size: *mut UINTN,
    buffer: *const c_void,
) -> EFI_STATUS;

pub type EFI_FILE_GET_POSITION = unsafe extern "efiapi" fn(
    this: *mut EFI_FILE_PROTOCOL,
    position: *mut u64,
) -> EFI_STATUS;

pub type EFI_FILE_SET_POSITION = unsafe extern "efiapi" fn(
    this: *mut EFI_FILE_PROTOCOL,
    position: u64,
) -> EFI_STATUS;

pub type EFI_FILE_GET_INFO = unsafe extern "efiapi" fn(
    this: *mut EFI_FILE_PROTOCOL,
    information_type: *const EFI_GUID,
    buffer_size: *mut UINTN,
    buffer: *mut c_void,
) -> EFI_STATUS;

pub type EFI_FILE_SET_INFO = unsafe extern "efiapi" fn(
    this: *mut EFI_FILE_PROTOCOL,
    information_type: *const EFI_GUID,
    buffer_size: UINTN,
    buffer: *const c_void,
) -> EFI_STATUS;

pub type EFI_FILE_FLUSH = unsafe extern "efiapi" fn(
    this: *mut EFI_FILE_PROTOCOL,
) -> EFI_STATUS;

#[repr(C)]
pub struct EFI_FILE_PROTOCOL {
    pub revision: u64,
    pub open: EFI_FILE_OPEN,
    pub close: EFI_FILE_CLOSE,
    pub delete: *const c_void,
    pub read: EFI_FILE_READ,
    pub write: EFI_FILE_WRITE,
    pub get_position: EFI_FILE_GET_POSITION,
    pub set_position: EFI_FILE_SET_POSITION,
    pub get_info: EFI_FILE_GET_INFO,
    pub set_info: EFI_FILE_SET_INFO,
    pub flush: EFI_FILE_FLUSH,
    // UEFI 2.0+
    pub open_ex: *const c_void,
    pub read_ex: *const c_void,
    pub write_ex: *const c_void,
    pub flush_ex: *const c_void,
}

// File open modes
pub const EFI_FILE_MODE_READ: u64 = 0x0000000000000001;
pub const EFI_FILE_MODE_WRITE: u64 = 0x0000000000000002;
pub const EFI_FILE_MODE_CREATE: u64 = 0x8000000000000000;

// File attributes
pub const EFI_FILE_READ_ONLY: u64 = 0x0000000000000001;
pub const EFI_FILE_HIDDEN: u64 = 0x0000000000000002;
pub const EFI_FILE_SYSTEM: u64 = 0x0000000000000004;
pub const EFI_FILE_RESERVED: u64 = 0x0000000000000008;
pub const EFI_FILE_DIRECTORY: u64 = 0x0000000000000010;
pub const EFI_FILE_ARCHIVE: u64 = 0x0000000000000020;

#[repr(C)]
pub struct EFI_FILE_INFO {
    pub size: u64,
    pub file_size: u64,
    pub physical_size: u64,
    pub create_time: EFI_TIME,
    pub last_access_time: EFI_TIME,
    pub modification_time: EFI_TIME,
    pub attribute: u64,
    // file_name follows as variable-length CHAR16 array
}

// =============================================================================
// Loaded Image Protocol
// =============================================================================

#[repr(C)]
pub struct EFI_LOADED_IMAGE_PROTOCOL {
    pub revision: u32,
    pub parent_handle: EFI_HANDLE,
    pub system_table: *mut c_void,
    
    // Source location
    pub device_handle: EFI_HANDLE,
    pub file_path: *mut c_void,
    pub reserved: *mut c_void,
    
    // Image load options
    pub load_options_size: u32,
    pub load_options: *mut c_void,
    
    // Image location in memory
    pub image_base: *mut c_void,
    pub image_size: u64,
    pub image_code_type: EFI_MEMORY_TYPE,
    pub image_data_type: EFI_MEMORY_TYPE,
    pub unload: *const c_void,
}
