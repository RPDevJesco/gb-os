//! UEFI Bootloader
//! 
//! A minimal UEFI bootloader written in pure Rust with no external dependencies.
//! 
//! # Features
//! - Console text output and input
//! - Memory map enumeration
//! - File system access (load files from boot partition)
//! - Graphics output (GOP framebuffer)
//! - Exit boot services and transfer to kernel
//! 
//! # Usage
//! Build with: cargo build --target x86_64-unknown-uefi
//! 
//! Copy the resulting .efi file to /EFI/BOOT/BOOTX64.EFI on a FAT32 partition.

#![no_std]
#![no_main]

mod uefi;
mod console;
mod memory;
mod fs;
mod graphics;

use core::panic::PanicInfo;
use uefi::*;

/// Bootloader version
const VERSION: &str = "0.1.0";

/// Kernel path to load (configurable)
const KERNEL_PATH: &str = "\\kernel.elf";

// =============================================================================
// UEFI Entry Point
// =============================================================================

/// UEFI application entry point
/// 
/// This is called by the UEFI firmware when the bootloader is executed.
#[unsafe(no_mangle)]
pub extern "efiapi" fn efi_main(
    image_handle: EFI_HANDLE,
    system_table: *mut EFI_SYSTEM_TABLE,
) -> EFI_STATUS {
    // Safety: We trust the firmware to pass valid pointers
    unsafe {
        // Initialize console
        console::Console::init(system_table);
        
        // Initialize memory services
        let boot_services = (*system_table).boot_services;
        memory::init(boot_services);
        
        // Disable watchdog timer (firmware may reboot us otherwise)
        ((*boot_services).set_watchdog_timer)(0, 0, 0, core::ptr::null_mut());
        
        // Run main bootloader logic
        match bootloader_main(image_handle, system_table) {
            Ok(_) => EFI_SUCCESS,
            Err(status) => {
                println!("\nBootloader failed with status: 0x{:X}", status);
                wait_for_key();
                status
            }
        }
    }
}

/// Main bootloader logic
/// 
/// # Safety
/// Caller must ensure image_handle and system_table are valid
unsafe fn bootloader_main(
    image_handle: EFI_HANDLE,
    system_table: *mut EFI_SYSTEM_TABLE,
) -> Result<(), EFI_STATUS> {
    unsafe {
        let console = console::Console::get().ok_or(EFI_DEVICE_ERROR)?;
        let boot_services = (*system_table).boot_services;
        
        // Clear screen and show banner
        console.clear();
        console.set_color(EFI_LIGHTCYAN, EFI_BACKGROUND_BLACK);
        
        println!("╔════════════════════════════════════════════════════════════╗");
        println!("║          UEFI Bootloader v{}                            ║", VERSION);
        println!("║          Written in Pure Rust - No Dependencies            ║");
        println!("╚════════════════════════════════════════════════════════════╝");
        println!();
        
        console.set_color(EFI_WHITE, EFI_BACKGROUND_BLACK);
        
        // Print firmware info
        let vendor = (*system_table).firmware_vendor;
        print!("Firmware: ");
        print_wstr(vendor);
        println!(" (rev 0x{:X})", (*system_table).firmware_revision);
        println!();
        
        // Interactive menu
        loop {
            console.set_color(EFI_YELLOW, EFI_BACKGROUND_BLACK);
            println!("=== Boot Menu ===");
            console.set_color(EFI_WHITE, EFI_BACKGROUND_BLACK);
            println!("1. Boot kernel ({})", KERNEL_PATH);
            println!("2. Show memory map");
            println!("3. Show graphics modes");
            println!("4. Test graphics output");
            println!("5. Show system info");
            println!("6. Reboot");
            println!("7. Shutdown");
            println!();
            print!("Select option: ");
            
            let key = console.wait_key().map_err(|_| EFI_DEVICE_ERROR)?;
            println!();
            
            match key.unicode_char {
                0x0031 => { // '1'
                    boot_kernel(image_handle, system_table)?;
                }
                0x0032 => { // '2'
                    show_memory_map()?;
                }
                0x0033 => { // '3'
                    show_graphics_modes(boot_services)?;
                }
                0x0034 => { // '4'
                    test_graphics(boot_services)?;
                }
                0x0035 => { // '5'
                    show_system_info(system_table)?;
                }
                0x0036 => { // '6'
                    reboot(system_table);
                }
                0x0037 => { // '7'
                    shutdown(system_table);
                }
                _ => {
                    println!("Invalid option");
                }
            }
            
            println!();
        }
    }
}

// =============================================================================
// Menu Functions
// =============================================================================

/// Boot the kernel
/// 
/// # Safety
/// Caller must ensure image_handle and system_table are valid
unsafe fn boot_kernel(
    image_handle: EFI_HANDLE,
    system_table: *mut EFI_SYSTEM_TABLE,
) -> Result<(), EFI_STATUS> {
    unsafe {
        let boot_services = (*system_table).boot_services;
        
        println!("Attempting to boot kernel...");
        
        // Open file system
        let fs = fs::FileSystem::from_loaded_image(boot_services, image_handle)?;
        
        // Check if kernel exists
        if !fs.exists(KERNEL_PATH) {
            println!("Kernel not found at {}", KERNEL_PATH);
            println!("Place your kernel at this path on the boot partition.");
            wait_for_key();
            return Ok(());
        }
        
        // Load kernel
        let (entry_point, kernel_buffer, kernel_size) = fs::load_kernel(&fs, KERNEL_PATH)?;
        
        println!("Kernel loaded: {} bytes at 0x{:X}", kernel_size, kernel_buffer as u64);
        println!("Entry point: 0x{:X}", entry_point);
        
        // Get final memory map
        println!("Getting final memory map...");
        let memory_map = memory::MemoryMap::get()?;
        let map_key = memory_map.map_key;
        
        println!("Exiting boot services (map_key: 0x{:X})...", map_key);
        
        // Exit boot services - point of no return!
        let status = ((*boot_services).exit_boot_services)(image_handle, map_key);
        
        if status != EFI_SUCCESS {
            // Memory map may have changed, try again
            let memory_map = memory::MemoryMap::get()?;
            let status = ((*boot_services).exit_boot_services)(image_handle, memory_map.map_key);
            
            if status != EFI_SUCCESS {
                // This is a problem - we're in a broken state
                // Just hang since we can't print anymore
                loop {
                    core::hint::spin_loop();
                }
            }
        }
        
        // Boot services are now gone - no more UEFI calls except runtime services!
        // We would jump to the kernel here
        
        // For demonstration, we'll just halt since we don't have a real kernel
        // In a real bootloader, you'd parse the ELF/PE header and jump to entry
        
        // Example: Jump to kernel
        // let kernel_entry: extern "C" fn(*mut BootInfo) -> ! = 
        //     core::mem::transmute(entry_point);
        // kernel_entry(boot_info);
        
        // Halt (no kernel to actually run)
        loop {
            core::hint::spin_loop();
        }
    }
}

/// Display memory map
fn show_memory_map() -> Result<(), EFI_STATUS> {
    let map = memory::MemoryMap::get()?;
    map.print_summary();
    
    println!("Press any key to continue...");
    wait_for_key();
    
    Ok(())
}

/// Show available graphics modes
/// 
/// # Safety
/// Caller must ensure boot_services is valid
unsafe fn show_graphics_modes(boot_services: *mut EFI_BOOT_SERVICES) -> Result<(), EFI_STATUS> {
    unsafe {
        match graphics::Graphics::new(boot_services) {
            Ok(gfx) => {
                gfx.list_modes();
                println!("\nCurrent: {}x{}", gfx.info.width, gfx.info.height);
                println!("Framebuffer: 0x{:X} ({} bytes)", gfx.info.base, gfx.info.size);
            }
            Err(_) => {
                println!("Graphics output not available");
            }
        }
        
        println!("\nPress any key to continue...");
        wait_for_key();
        
        Ok(())
    }
}

/// Test graphics output
/// 
/// # Safety
/// Caller must ensure boot_services is valid
unsafe fn test_graphics(boot_services: *mut EFI_BOOT_SERVICES) -> Result<(), EFI_STATUS> {
    unsafe {
        let gfx = match graphics::Graphics::new(boot_services) {
            Ok(g) => g,
            Err(_) => {
                println!("Graphics not available");
                return Ok(());
            }
        };
        
        // Clear to dark blue
        gfx.clear(0, 0, 64);
        
        // Draw some shapes
        gfx.fill_rect(50, 50, 200, 150, 255, 0, 0);    // Red rectangle
        gfx.fill_rect(100, 100, 200, 150, 0, 255, 0);  // Green rectangle
        gfx.fill_rect(150, 150, 200, 150, 0, 0, 255);  // Blue rectangle
        
        // Draw text
        gfx.draw_string(50, 20, "UEFI GRAPHICS TEST", 255, 255, 255);
        gfx.draw_string(50, 350, "Press any key to return...", 255, 255, 0);
        
        // Draw border
        gfx.rect(10, 10, gfx.info.width - 20, gfx.info.height - 20, 255, 255, 255);
        
        wait_for_key();
        
        // Restore text mode by clearing console
        if let Some(console) = console::Console::get() {
            console.clear();
        }
        
        Ok(())
    }
}

/// Show system information
/// 
/// # Safety
/// Caller must ensure system_table is valid
unsafe fn show_system_info(system_table: *mut EFI_SYSTEM_TABLE) -> Result<(), EFI_STATUS> {
    unsafe {
        println!("=== System Information ===");
        println!();
        
        // Table revision
        let rev = (*system_table).hdr.revision;
        println!("UEFI Revision: {}.{}", rev >> 16, rev & 0xFFFF);
        
        // Firmware
        print!("Firmware Vendor: ");
        print_wstr((*system_table).firmware_vendor);
        println!();
        println!("Firmware Revision: 0x{:08X}", (*system_table).firmware_revision);
        
        // Configuration tables
        println!("\nConfiguration Tables: {}", (*system_table).number_of_table_entries);
        
        let config_tables = (*system_table).configuration_table;
        for i in 0..(*system_table).number_of_table_entries {
            let table = &*config_tables.add(i);
            let guid = &table.vendor_guid;
            
            // Check for known GUIDs
            let name = if guid.data1 == 0x8868e871 { // ACPI 2.0
                "ACPI 2.0 RSDP"
            } else if guid.data1 == 0xeb9d2d30 { // ACPI 1.0
                "ACPI 1.0 RSDP"
            } else if guid.data1 == 0x7739f24c { // SMBIOS 3.0
                "SMBIOS 3.0"
            } else if guid.data1 == 0xeb9d2d31 { // SMBIOS
                "SMBIOS"
            } else {
                "Unknown"
            };
            
            println!(
                "  {:08X}-{:04X}-{:04X} = {} (0x{:X})",
                guid.data1, guid.data2, guid.data3,
                name,
                table.vendor_table as u64
            );
        }
        
        println!("\nPress any key to continue...");
        wait_for_key();
        
        Ok(())
    }
}

/// Reboot the system
/// 
/// # Safety
/// Caller must ensure system_table is valid
unsafe fn reboot(system_table: *mut EFI_SYSTEM_TABLE) -> ! {
    unsafe {
        println!("Rebooting...");
        let runtime = (*system_table).runtime_services;
        ((*runtime).reset_system)(EFI_RESET_COLD, EFI_SUCCESS, 0, core::ptr::null());
    }
}

/// Shutdown the system
/// 
/// # Safety
/// Caller must ensure system_table is valid
unsafe fn shutdown(system_table: *mut EFI_SYSTEM_TABLE) -> ! {
    unsafe {
        println!("Shutting down...");
        let runtime = (*system_table).runtime_services;
        ((*runtime).reset_system)(EFI_RESET_SHUTDOWN, EFI_SUCCESS, 0, core::ptr::null());
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Print a UTF-16 string
/// 
/// # Safety
/// Caller must ensure s is a valid null-terminated UTF-16 string
unsafe fn print_wstr(s: *const CHAR16) {
    unsafe {
        let mut ptr = s;
        while *ptr != 0 {
            let c = char::from_u32(*ptr as u32).unwrap_or('?');
            print!("{}", c);
            ptr = ptr.add(1);
        }
    }
}

/// Wait for any key press
fn wait_for_key() {
    if let Some(console) = console::Console::get() {
        let _ = console.wait_key();
    }
}

// =============================================================================
// Panic Handler
// =============================================================================

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n!!! PANIC !!!");
    
    if let Some(location) = info.location() {
        println!("At {}:{}:{}", location.file(), location.line(), location.column());
    }
    
    // PanicMessage now implements Display directly
    println!("Message: {}", info.message());
    
    println!("\nSystem halted.");
    
    loop {
        core::hint::spin_loop();
    }
}
