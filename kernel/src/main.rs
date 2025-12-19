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

// GameBoy emulator
mod gameboy;

mod storage;

use boot_info::BootInfo;
use arch::x86::{gdt, idt};
use core::arch::global_asm;

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
unsafe fn draw_bar(x: isize, row: isize, color: u8, width: isize) {
    let vga = 0xA0000 as *mut u8;
    for i in 0..width {
        core::ptr::write_volatile(vga.offset(row + x + i), color);
    }
}

// ============================================================================
// Kernel Main
// ============================================================================
#[no_mangle]
extern "C" fn kernel_main(_boot_info_ptr: u32) -> ! {
    // Parse boot info from fixed address 0x500
    let boot_info = unsafe { BootInfo::from_ptr(0x500 as *const u8) };

    // Initialize GDT
    gdt::init();

    // Initialize IDT
    idt::init();

    // Initialize memory manager
    mm::init(boot_info.e820_map_addr);

    // Initialize storage subsystem
    let storage_result = storage::init();

    // Enable interrupts
    unsafe { core::arch::asm!("sti"); }

    // Test disk read if devices found
    if storage_result.ata_devices > 0 {
        storage::test_read();

        // Try to mount FAT32 and load ROM
        if let Some((rom_ptr, rom_size)) = try_load_rom() {
            // Clear screen first (dark green/gray like original GB)
            clear_screen(0x00);

            // Draw Game Boy border
            draw_gb_border();

            // Run the emulator with loaded ROM
            run_gameboy_emulator_with_rom(rom_ptr, rom_size);
        }
    }

    // Halt if we get here
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

// Static ROM buffer - must be outside the function to have stable address
static mut ROM_BUFFER: [u8; 2 * 1024 * 1024] = [0; 2 * 1024 * 1024]; // 2MB max

/// Try to load first ROM from FAT32
fn try_load_rom() -> Option<(*const u8, usize)> {
    // Mount FAT32
    if storage::fat32::mount(0).is_err() {
        return None;
    }

    // Find first ROM
    let (cluster, size) = storage::fat32::get_fs().find_rom(0)?;

    // Load ROM data into static buffer
    let rom_buf = unsafe { &mut ROM_BUFFER };

    match storage::fat32::get_fs().read_file(cluster, size, rom_buf) {
        Ok(bytes_read) => {
            // Return pointer and size
            Some((rom_buf.as_ptr(), bytes_read))
        }
        Err(_) => None,
    }
}

// ============================================================================
// VGA Mode 13h Display Functions
// ============================================================================

/// Clear the entire VGA screen
fn clear_screen(color: u8) {
    unsafe {
        let vga = 0xA0000 as *mut u8;
        for i in 0..(320 * 200) {
            core::ptr::write_volatile(vga.add(i), color);
        }
    }
}

/// Draw the Game Boy screen border
/// GB is 160x144, centered in 320x200: x=80, y=28
fn draw_gb_border() {
    const GB_WIDTH: isize = 160;
    const GB_HEIGHT: isize = 144;
    const START_X: isize = 80;
    const START_Y: isize = 28;
    const BORDER: isize = 4;
    const BORDER_COLOR: u8 = 0x08;  // Dark gray

    unsafe {
        let vga = 0xA0000 as *mut u8;

        // Top border
        for x in (START_X - BORDER)..(START_X + GB_WIDTH + BORDER) {
            for y_off in 0..BORDER {
                let offset = (START_Y - BORDER + y_off) * 320 + x;
                if offset >= 0 && offset < 64000 {
                    core::ptr::write_volatile(vga.offset(offset), BORDER_COLOR);
                }
            }
        }

        // Bottom border
        for x in (START_X - BORDER)..(START_X + GB_WIDTH + BORDER) {
            for y_off in 0..BORDER {
                let offset = (START_Y + GB_HEIGHT + y_off) * 320 + x;
                if offset >= 0 && offset < 64000 {
                    core::ptr::write_volatile(vga.offset(offset), BORDER_COLOR);
                }
            }
        }

        // Left border
        for y in (START_Y - BORDER)..(START_Y + GB_HEIGHT + BORDER) {
            for x_off in 0..BORDER {
                let offset = y * 320 + START_X - BORDER + x_off;
                if offset >= 0 && offset < 64000 {
                    core::ptr::write_volatile(vga.offset(offset), BORDER_COLOR);
                }
            }
        }

        // Right border
        for y in (START_Y - BORDER)..(START_Y + GB_HEIGHT + BORDER) {
            for x_off in 0..BORDER {
                let offset = y * 320 + START_X + GB_WIDTH + x_off;
                if offset >= 0 && offset < 64000 {
                    core::ptr::write_volatile(vga.offset(offset), BORDER_COLOR);
                }
            }
        }
    }
}

/// Blit Game Boy framebuffer (160x144 RGB) to VGA mode 13h (320x200)
///
/// The GB GPU outputs RGB format (3 bytes per pixel).
/// We convert to VGA palette indices using grayscale approximation.
///
/// VGA mode 13h default palette has grayscale at indices 16-31.
fn blit_gb_to_vga(gb_data: &[u8]) {
    const START_X: isize = 80;
    const START_Y: isize = 28;
    const SCREEN_W: usize = 160;
    const SCREEN_H: usize = 144;

    unsafe {
        let vga = 0xA0000 as *mut u8;

        for y in 0..SCREEN_H {
            for x in 0..SCREEN_W {
                // RGB format: 3 bytes per pixel
                let src_idx = (y * SCREEN_W + x) * 3;

                if src_idx + 2 < gb_data.len() {
                    let r = gb_data[src_idx] as u16;
                    let g = gb_data[src_idx + 1] as u16;
                    let b = gb_data[src_idx + 2] as u16;

                    // Convert RGB to grayscale using luminance formula
                    // Y = 0.299*R + 0.587*G + 0.114*B (approximated with integers)
                    let gray = ((r * 77 + g * 150 + b * 29) >> 8) as u8;

                    // Map to VGA grayscale palette (indices 16-31)
                    // gray is 0-255, we want 16-31 (16 levels)
                    let vga_color = 16 + (gray >> 4);

                    let offset = (START_Y as usize + y) * 320 + START_X as usize + x;
                    core::ptr::write_volatile(vga.add(offset), vga_color);
                }
            }
        }
    }
}

// ============================================================================
// GameBoy Emulator Integration
// ============================================================================
fn run_gameboy_emulator_with_rom(rom_ptr: *const u8, rom_size: usize) -> ! {
    use alloc::vec::Vec;

    // Initialize PIT for accurate timing (1000 Hz = 1ms per tick)
    arch::x86::pit::set_frequency(1000);

    // Create ROM vec from loaded data
    let rom_data: Vec<u8> = unsafe {
        let rom_slice = core::slice::from_raw_parts(rom_ptr, rom_size);
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

fn run_gameboy_emulator() -> ! {
    use alloc::vec::Vec;

    // Initialize PIT for accurate timing (1000 Hz = 1ms per tick)
    arch::x86::pit::set_frequency(1000);

    // Check if ROM was loaded by bootloader
    let boot_info = unsafe { BootInfo::from_ptr(0x500 as *const u8) };

    let rom_data: Vec<u8> = if boot_info.rom_addr != 0 && boot_info.rom_size > 0 {
        // ROM loaded by bootloader
        let rom_slice = unsafe {
            core::slice::from_raw_parts(
                boot_info.rom_addr as *const u8,
                boot_info.rom_size as usize
            )
        };
        rom_slice.to_vec()
    } else {
        // No ROM loaded - show error and halt
        show_no_rom_error();
        loop { unsafe { core::arch::asm!("hlt"); } }
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
