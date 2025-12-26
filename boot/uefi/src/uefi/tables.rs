//! UEFI System Table and Services
//! 
//! The System Table is the root of all UEFI functionality.

#![allow(non_camel_case_types)]
#![allow(dead_code)]

use core::ffi::c_void;
use super::types::*;
use super::protocols::*;

// =============================================================================
// System Table
// =============================================================================

#[repr(C)]
pub struct EFI_SYSTEM_TABLE {
    pub hdr: EFI_TABLE_HEADER,
    pub firmware_vendor: *const CHAR16,
    pub firmware_revision: u32,
    pub console_in_handle: EFI_HANDLE,
    pub con_in: *mut EFI_SIMPLE_TEXT_INPUT_PROTOCOL,
    pub console_out_handle: EFI_HANDLE,
    pub con_out: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    pub standard_error_handle: EFI_HANDLE,
    pub std_err: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    pub runtime_services: *mut EFI_RUNTIME_SERVICES,
    pub boot_services: *mut EFI_BOOT_SERVICES,
    pub number_of_table_entries: UINTN,
    pub configuration_table: *mut EFI_CONFIGURATION_TABLE,
}

#[repr(C)]
pub struct EFI_CONFIGURATION_TABLE {
    pub vendor_guid: EFI_GUID,
    pub vendor_table: *mut c_void,
}

// =============================================================================
// Boot Services
// =============================================================================

/// Function pointer types for Boot Services
pub type EFI_RAISE_TPL = unsafe extern "efiapi" fn(new_tpl: EFI_TPL) -> EFI_TPL;
pub type EFI_RESTORE_TPL = unsafe extern "efiapi" fn(old_tpl: EFI_TPL);

pub type EFI_ALLOCATE_PAGES = unsafe extern "efiapi" fn(
    alloc_type: EFI_ALLOCATE_TYPE,
    memory_type: EFI_MEMORY_TYPE,
    pages: UINTN,
    memory: *mut EFI_PHYSICAL_ADDRESS,
) -> EFI_STATUS;

pub type EFI_FREE_PAGES = unsafe extern "efiapi" fn(
    memory: EFI_PHYSICAL_ADDRESS,
    pages: UINTN,
) -> EFI_STATUS;

pub type EFI_GET_MEMORY_MAP = unsafe extern "efiapi" fn(
    memory_map_size: *mut UINTN,
    memory_map: *mut EFI_MEMORY_DESCRIPTOR,
    map_key: *mut UINTN,
    descriptor_size: *mut UINTN,
    descriptor_version: *mut u32,
) -> EFI_STATUS;

pub type EFI_ALLOCATE_POOL = unsafe extern "efiapi" fn(
    pool_type: EFI_MEMORY_TYPE,
    size: UINTN,
    buffer: *mut *mut c_void,
) -> EFI_STATUS;

pub type EFI_FREE_POOL = unsafe extern "efiapi" fn(buffer: *mut c_void) -> EFI_STATUS;

pub type EFI_CREATE_EVENT = unsafe extern "efiapi" fn(
    event_type: u32,
    notify_tpl: EFI_TPL,
    notify_function: Option<unsafe extern "efiapi" fn(EFI_EVENT, *mut c_void)>,
    notify_context: *mut c_void,
    event: *mut EFI_EVENT,
) -> EFI_STATUS;

pub type EFI_SET_TIMER = unsafe extern "efiapi" fn(
    event: EFI_EVENT,
    timer_type: u32,
    trigger_time: u64,
) -> EFI_STATUS;

pub type EFI_WAIT_FOR_EVENT = unsafe extern "efiapi" fn(
    number_of_events: UINTN,
    event: *const EFI_EVENT,
    index: *mut UINTN,
) -> EFI_STATUS;

pub type EFI_SIGNAL_EVENT = unsafe extern "efiapi" fn(event: EFI_EVENT) -> EFI_STATUS;
pub type EFI_CLOSE_EVENT = unsafe extern "efiapi" fn(event: EFI_EVENT) -> EFI_STATUS;
pub type EFI_CHECK_EVENT = unsafe extern "efiapi" fn(event: EFI_EVENT) -> EFI_STATUS;

pub type EFI_HANDLE_PROTOCOL = unsafe extern "efiapi" fn(
    handle: EFI_HANDLE,
    protocol: *const EFI_GUID,
    interface: *mut *mut c_void,
) -> EFI_STATUS;

pub type EFI_LOCATE_HANDLE = unsafe extern "efiapi" fn(
    search_type: u32,
    protocol: *const EFI_GUID,
    search_key: *mut c_void,
    buffer_size: *mut UINTN,
    buffer: *mut EFI_HANDLE,
) -> EFI_STATUS;

pub type EFI_LOCATE_PROTOCOL = unsafe extern "efiapi" fn(
    protocol: *const EFI_GUID,
    registration: *mut c_void,
    interface: *mut *mut c_void,
) -> EFI_STATUS;

pub type EFI_IMAGE_LOAD = unsafe extern "efiapi" fn(
    boot_policy: BOOLEAN,
    parent_image_handle: EFI_HANDLE,
    device_path: *mut c_void,
    source_buffer: *mut c_void,
    source_size: UINTN,
    image_handle: *mut EFI_HANDLE,
) -> EFI_STATUS;

pub type EFI_IMAGE_START = unsafe extern "efiapi" fn(
    image_handle: EFI_HANDLE,
    exit_data_size: *mut UINTN,
    exit_data: *mut *mut CHAR16,
) -> EFI_STATUS;

pub type EFI_EXIT = unsafe extern "efiapi" fn(
    image_handle: EFI_HANDLE,
    exit_status: EFI_STATUS,
    exit_data_size: UINTN,
    exit_data: *mut CHAR16,
) -> EFI_STATUS;

pub type EFI_EXIT_BOOT_SERVICES = unsafe extern "efiapi" fn(
    image_handle: EFI_HANDLE,
    map_key: UINTN,
) -> EFI_STATUS;

pub type EFI_SET_WATCHDOG_TIMER = unsafe extern "efiapi" fn(
    timeout: UINTN,
    watchdog_code: u64,
    data_size: UINTN,
    watchdog_data: *mut CHAR16,
) -> EFI_STATUS;

pub type EFI_STALL = unsafe extern "efiapi" fn(microseconds: UINTN) -> EFI_STATUS;

pub type EFI_OPEN_PROTOCOL = unsafe extern "efiapi" fn(
    handle: EFI_HANDLE,
    protocol: *const EFI_GUID,
    interface: *mut *mut c_void,
    agent_handle: EFI_HANDLE,
    controller_handle: EFI_HANDLE,
    attributes: u32,
) -> EFI_STATUS;

pub type EFI_CLOSE_PROTOCOL = unsafe extern "efiapi" fn(
    handle: EFI_HANDLE,
    protocol: *const EFI_GUID,
    agent_handle: EFI_HANDLE,
    controller_handle: EFI_HANDLE,
) -> EFI_STATUS;

pub type EFI_LOCATE_HANDLE_BUFFER = unsafe extern "efiapi" fn(
    search_type: u32,
    protocol: *const EFI_GUID,
    search_key: *mut c_void,
    no_handles: *mut UINTN,
    buffer: *mut *mut EFI_HANDLE,
) -> EFI_STATUS;

#[repr(C)]
pub struct EFI_BOOT_SERVICES {
    pub hdr: EFI_TABLE_HEADER,
    
    // Task Priority Services
    pub raise_tpl: EFI_RAISE_TPL,
    pub restore_tpl: EFI_RESTORE_TPL,
    
    // Memory Services
    pub allocate_pages: EFI_ALLOCATE_PAGES,
    pub free_pages: EFI_FREE_PAGES,
    pub get_memory_map: EFI_GET_MEMORY_MAP,
    pub allocate_pool: EFI_ALLOCATE_POOL,
    pub free_pool: EFI_FREE_POOL,
    
    // Event & Timer Services
    pub create_event: EFI_CREATE_EVENT,
    pub set_timer: EFI_SET_TIMER,
    pub wait_for_event: EFI_WAIT_FOR_EVENT,
    pub signal_event: EFI_SIGNAL_EVENT,
    pub close_event: EFI_CLOSE_EVENT,
    pub check_event: EFI_CHECK_EVENT,
    
    // Protocol Handler Services
    pub install_protocol_interface: *const c_void,
    pub reinstall_protocol_interface: *const c_void,
    pub uninstall_protocol_interface: *const c_void,
    pub handle_protocol: EFI_HANDLE_PROTOCOL,
    pub reserved: *const c_void,
    pub register_protocol_notify: *const c_void,
    pub locate_handle: EFI_LOCATE_HANDLE,
    pub locate_device_path: *const c_void,
    pub install_configuration_table: *const c_void,
    
    // Image Services
    pub load_image: EFI_IMAGE_LOAD,
    pub start_image: EFI_IMAGE_START,
    pub exit: EFI_EXIT,
    pub unload_image: *const c_void,
    pub exit_boot_services: EFI_EXIT_BOOT_SERVICES,
    
    // Misc Services
    pub get_next_monotonic_count: *const c_void,
    pub stall: EFI_STALL,
    pub set_watchdog_timer: EFI_SET_WATCHDOG_TIMER,
    
    // Driver Support Services
    pub connect_controller: *const c_void,
    pub disconnect_controller: *const c_void,
    
    // Open/Close Protocol Services
    pub open_protocol: EFI_OPEN_PROTOCOL,
    pub close_protocol: EFI_CLOSE_PROTOCOL,
    pub open_protocol_information: *const c_void,
    
    // Library Services
    pub protocols_per_handle: *const c_void,
    pub locate_handle_buffer: EFI_LOCATE_HANDLE_BUFFER,
    pub locate_protocol: EFI_LOCATE_PROTOCOL,
    pub install_multiple_protocol_interfaces: *const c_void,
    pub uninstall_multiple_protocol_interfaces: *const c_void,
    
    // CRC32 Services
    pub calculate_crc32: *const c_void,
    
    // Misc
    pub copy_mem: unsafe extern "efiapi" fn(*mut c_void, *const c_void, UINTN),
    pub set_mem: unsafe extern "efiapi" fn(*mut c_void, UINTN, u8),
    pub create_event_ex: *const c_void,
}

// =============================================================================
// Runtime Services
// =============================================================================

pub type EFI_GET_TIME = unsafe extern "efiapi" fn(
    time: *mut EFI_TIME,
    capabilities: *mut EFI_TIME_CAPABILITIES,
) -> EFI_STATUS;

pub type EFI_SET_TIME = unsafe extern "efiapi" fn(time: *const EFI_TIME) -> EFI_STATUS;

pub type EFI_GET_VARIABLE = unsafe extern "efiapi" fn(
    variable_name: *const CHAR16,
    vendor_guid: *const EFI_GUID,
    attributes: *mut u32,
    data_size: *mut UINTN,
    data: *mut c_void,
) -> EFI_STATUS;

pub type EFI_SET_VARIABLE = unsafe extern "efiapi" fn(
    variable_name: *const CHAR16,
    vendor_guid: *const EFI_GUID,
    attributes: u32,
    data_size: UINTN,
    data: *const c_void,
) -> EFI_STATUS;

pub type EFI_RESET_SYSTEM = unsafe extern "efiapi" fn(
    reset_type: u32,
    reset_status: EFI_STATUS,
    data_size: UINTN,
    reset_data: *const c_void,
) -> !;

#[repr(C)]
pub struct EFI_RUNTIME_SERVICES {
    pub hdr: EFI_TABLE_HEADER,
    
    // Time Services
    pub get_time: EFI_GET_TIME,
    pub set_time: EFI_SET_TIME,
    pub get_wakeup_time: *const c_void,
    pub set_wakeup_time: *const c_void,
    
    // Virtual Memory Services
    pub set_virtual_address_map: *const c_void,
    pub convert_pointer: *const c_void,
    
    // Variable Services
    pub get_variable: EFI_GET_VARIABLE,
    pub get_next_variable_name: *const c_void,
    pub set_variable: EFI_SET_VARIABLE,
    
    // Misc Services
    pub get_next_high_monotonic_count: *const c_void,
    pub reset_system: EFI_RESET_SYSTEM,
    
    // UEFI 2.0 Capsule Services
    pub update_capsule: *const c_void,
    pub query_capsule_capabilities: *const c_void,
    
    // UEFI 2.0 Query Variable Info
    pub query_variable_info: *const c_void,
}

// Reset types for reset_system
pub const EFI_RESET_COLD: u32 = 0;
pub const EFI_RESET_WARM: u32 = 1;
pub const EFI_RESET_SHUTDOWN: u32 = 2;
pub const EFI_RESET_PLATFORM_SPECIFIC: u32 = 3;
