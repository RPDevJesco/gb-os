//! RetroFutureGB - Bare Metal Game Boy Emulator
//!
//! A Game Boy emulator that runs directly on x86 hardware without an OS.
//! Boots into VGA mode 13h (320x200x256) - perfect for 160x144 GB scaled 2x.

#![no_std]
#![no_main]
#![allow(dead_code)]

extern crate alloc;

// Core kernel modules
mod boot_info;
mod arch;
mod mm;
mod drivers;
mod event_chains;

// GUI module (needed by drivers/init.rs even if not used)
mod gui;

// Filesystem support (for ROM partition)
mod fs;

// User interface (ROM menu)
mod ui;

// GameBoy emulator
mod gameboy;

use boot_info::{BootInfo, BootType};
use arch::x86::{gdt, idt};
use core::arch::global_asm;
use alloc::vec::Vec;

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Draw red bar at top of screen to indicate panic
    unsafe {
        let vga = 0xA0000 as *mut u8;
        for i in 0..320 {
            core::ptr::write_volatile(vga.add(i), 0x04); // Red
        }
    }
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}

// ============================================================================
// Assembly Entry Point
// ============================================================================

global_asm!(
    ".section .text.boot",
    ".global _start",
    "_start:",

    // VGA Mode 13h: framebuffer at 0xA0000, 320x200, 8bpp
    // Draw progress pixels on row 5

    // Stage 1: Kernel entry reached - WHITE pixels
    "    mov edi, 0xA0640",
    "    mov al, 0x0F",
    "    mov ecx, 10",
    "1:  stosb",
    "    loop 1b",

    // Set up stack
    "    mov esp, 0x90000",

    // Stage 2: Stack set - GREEN pixels
    "    mov edi, 0xA064A",
    "    mov al, 0x02",
    "    mov ecx, 10",
    "2:  stosb",
    "    loop 2b",

    // Push boot_info pointer (in EAX from bootloader)
    "    push eax",

    // Stage 3: About to call kernel_main - CYAN pixels
    "    mov edi, 0xA0654",
    "    mov al, 0x03",
    "    mov ecx, 10",
    "3:  stosb",
    "    loop 3b",

    // Call kernel_main
    "    call kernel_main",

    // If kernel_main returns - RED pixels (shouldn't happen)
    "    mov edi, 0xA065E",
    "    mov al, 0x04",
    "    mov ecx, 10",
    "4:  stosb",
    "    loop 4b",

    "5:",
    "    cli",
    "    hlt",
    "    jmp 5b",
);

// ============================================================================
// Helper Functions
// ============================================================================

/// Draw a colored bar for debug progress
#[inline(always)]
#[allow(dead_code)]
unsafe fn draw_bar(x: isize, row: isize, color: u8, width: isize) {
    let vga = 0xA0000 as *mut u8;
    for i in 0..width {
        core::ptr::write_volatile(vga.offset(row + x + i), color);
    }
}

// ============================================================================
// ROM Loading
// ============================================================================

/// ROM address for dynamic loading (3MB mark)
const ROM_LOAD_ADDRESS: u32 = 0x300000;

/// Try to load ROM from ROMS partition
fn load_rom_from_partition(boot_info: &BootInfo) -> Option<(u32, u32)> {
    // Only works for partition boot
    if boot_info.boot_type != BootType::Partition {
        return None;
    }

    // Get ATA drive from BIOS drive number
    let ata_drive = drivers::bios_drive_to_ata(boot_info.boot_drive as u8)?;

    // Create ATA driver
    let mut ata = drivers::Ata::new(ata_drive);

    // Find ROMS partition (partition 2)
    let roms_partition_start = match fs::find_roms_partition(&mut ata) {
        Ok(lba) => lba,
        Err(_) => return None,
    };

    // Open FAT16 filesystem on ROMS partition
    let mut fat = match fs::Fat16::new(ata_drive, roms_partition_start) {
        Ok(f) => f,
        Err(_) => return None,
    };

    // List ROM files
    let roms = match fat.list_roms() {
        Ok(r) => r,
        Err(_) => return None,
    };

    if roms.is_empty() {
        // No ROMs - show message and return None
        ui::show_rom_menu(roms);
        return None;
    }

    // Show ROM selection menu
    let selected_idx = ui::show_rom_menu(roms.clone())?;

    // Get selected ROM
    let selected_rom = &roms[selected_idx];

    // Load ROM to memory
    unsafe {
        let dest = ROM_LOAD_ADDRESS as *mut u8;
        match fat.load_rom(selected_rom, dest) {
            Ok(size) => Some((ROM_LOAD_ADDRESS, size)),
            Err(_) => None,
        }
    }
}

/// Clear the screen to black
fn clear_screen() {
    unsafe {
        let vga = 0xA0000 as *mut u8;
        for i in 0..(320 * 200) {
            core::ptr::write_volatile(vga.add(i), 0x00);
        }
    }
}

// ============================================================================
// VGA Blitting
// ============================================================================

/// Convert RGB to VGA palette index (grayscale approximation)
/// VGA palette: 16-31 are grayscale
#[inline(always)]
fn rgb_to_vga_gray(r: u8, g: u8, b: u8) -> u8 {
    // Luminance formula: Y = 0.299R + 0.587G + 0.114B
    // Simplified: Y = (R + 2*G + B) / 4
    let lum = ((r as u16 + 2 * g as u16 + b as u16) / 4) as u8;
    // Map to 16 grayscale levels (palette 16-31)
    16 + (lum >> 4)
}

/// Blit Game Boy framebuffer to VGA
/// GB: 160x144 RGB -> VGA: 320x200 centered with grayscale conversion
fn blit_gb_to_vga(gpu_data: &[u8]) {
    const GB_WIDTH: usize = 160;
    const GB_HEIGHT: usize = 144;
    const VGA_WIDTH: usize = 320;

    // Center the GB screen on VGA (320-160)/2 = 80, (200-144)/2 = 28
    const OFFSET_X: usize = 80;
    const OFFSET_Y: usize = 28;

    let vga = 0xA0000 as *mut u8;

    for y in 0..GB_HEIGHT {
        for x in 0..GB_WIDTH {
            let src_idx = (y * GB_WIDTH + x) * 3;
            let r = gpu_data[src_idx];
            let g = gpu_data[src_idx + 1];
            let b = gpu_data[src_idx + 2];

            let color = rgb_to_vga_gray(r, g, b);

            let dst_idx = (OFFSET_Y + y) * VGA_WIDTH + OFFSET_X + x;
            unsafe {
                core::ptr::write_volatile(vga.add(dst_idx), color);
            }
        }
    }
}

// ============================================================================
// Kernel Main
// ============================================================================

#[no_mangle]
extern "C" fn kernel_main(_boot_info_ptr: u32) -> ! {
    // Initialize GDT and IDT
    gdt::init();
    idt::init();

    // Initialize heap
    unsafe { mm::heap::init(); }

    // Initialize PIT for accurate timing (1000 Hz = 1ms per tick)
    arch::x86::pit::set_frequency(1000);

    // Get boot info
    let boot_info = unsafe {
        boot_info::get_boot_info().unwrap_or_else(|| {
            // Create a minimal boot info if parsing fails
            show_boot_error();
            loop { unsafe { core::arch::asm!("hlt"); } }
        })
    };

    // Determine ROM source
    let (rom_addr, rom_size): (u32, u32) = if boot_info.has_rom() {
        // ROM was embedded by bootloader
        (boot_info.rom_addr, boot_info.rom_size)
    } else if boot_info.boot_type == BootType::Partition {
        // Try to load from ROMS partition
        match load_rom_from_partition(&boot_info) {
            Some((addr, size)) => {
                // Clear screen after menu
                clear_screen();
                (addr, size)
            }
            None => {
                // No ROM available
                show_no_rom_error();
                loop { unsafe { core::arch::asm!("hlt"); } }
            }
        }
    } else {
        // Raw boot without ROM
        show_no_rom_error();
        loop { unsafe { core::arch::asm!("hlt"); } }
    };

    // Load ROM data into Vec
    let rom_data: Vec<u8> = unsafe {
        let rom_slice = core::slice::from_raw_parts(
            rom_addr as *const u8,
            rom_size as usize
        );
        rom_slice.to_vec()
    };

    // Create emulator
    let mut device = match gameboy::Device::new_cgb(rom_data, false) {
        Ok(d) => d,
        Err(_e) => {
            show_emulator_error();
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    };

    // Create input handler
    let mut input_state = gameboy::input::InputState::new();

    // Frame timing: 59.7 fps = ~16.75ms per frame
    // At 1000 Hz, that's ~17 ticks per frame
    const TICKS_PER_FRAME: u32 = 17;
    let mut last_frame_ticks = arch::x86::pit::ticks();

    // Main emulation loop
    const CYCLES_PER_FRAME: u32 = 70224;  // ~59.7 FPS

    loop {
        // Run one frame of emulation
        let mut cycles: u32 = 0;
        while cycles < CYCLES_PER_FRAME {
            cycles += device.do_cycle();
        }

        // Blit to screen if GPU updated
        if device.check_and_reset_gpu_updated() {
            let gpu_data = device.get_gpu_data();
            blit_gb_to_vga(gpu_data);
        }

        // Process keyboard input
        while let Some(key) = drivers::keyboard::get_key() {
            if let Some(gb_key) = input_state.map_keycode(key.keycode) {
                if key.pressed {
                    device.keydown(gb_key);
                } else {
                    device.keyup(gb_key);
                }
            }
        }

        // Frame timing - wait until next frame time
        let target_ticks = last_frame_ticks.wrapping_add(TICKS_PER_FRAME);
        while arch::x86::pit::ticks().wrapping_sub(target_ticks) > 0x8000_0000 {
            // Use HLT for power efficiency while waiting
            unsafe { core::arch::asm!("hlt"); }
        }
        last_frame_ticks = target_ticks;
    }
}

/// Show "NO ROM" error on screen
fn show_no_rom_error() {
    const START_Y: isize = 80;
    const START_X: isize = 120;

    unsafe {
        let vga = 0xA0000 as *mut u8;

        // Draw red "X" pattern to indicate error
        for i in 0..40isize {
            let offset1 = (START_Y + i) * 320 + START_X + i;
            let offset2 = (START_Y + i) * 320 + START_X + 40 - i;
            core::ptr::write_volatile(vga.offset(offset1), 0x04);  // Red
            core::ptr::write_volatile(vga.offset(offset2), 0x04);
        }
    }
}

/// Show emulator initialization error
fn show_emulator_error() {
    const START_Y: isize = 80;

    unsafe {
        let vga = 0xA0000 as *mut u8;

        // Draw yellow bar to indicate emulator error
        for x in 100..220isize {
            let offset = START_Y * 320 + x;
            core::ptr::write_volatile(vga.offset(offset), 0x0E);  // Yellow
        }
    }
}

/// Show boot info error
fn show_boot_error() {
    const START_Y: isize = 80;

    unsafe {
        let vga = 0xA0000 as *mut u8;

        // Draw magenta bar to indicate boot info error
        for x in 100..220isize {
            let offset = START_Y * 320 + x;
            core::ptr::write_volatile(vga.offset(offset), 0x05);  // Magenta
        }
    }
}
