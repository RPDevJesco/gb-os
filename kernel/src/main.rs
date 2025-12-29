//! RetroFutureGB - Bare Metal Game Boy Emulator
//!
//! A Game Boy emulator that runs directly on x86 hardware without an OS.
//! Boots into VGA mode 13h (320x200x256) - perfect for 160x144 GB scaled 2x.

#![no_std]
#![no_main]
#![allow(dead_code)]

extern crate alloc;

// Core kernel modules
mod defensive;
mod boot_info;
mod arch;
mod mm;
mod drivers;
mod event_chains;

// GUI module (needed by drivers/init.rs even if not used)
mod graphics;
mod gui;
mod storage;
mod rom_browser;

// GameBoy emulator
mod gameboy;

pub mod overlay;

use boot_info::BootInfo;
use arch::x86::{gdt, idt};
use core::arch::global_asm;
use crate::graphics::vga_palette;
use crate::gameboy::gbmode::GbMode;

// Import defensive module for hardening
use defensive::{OperationId, set_last_operation};

// Import layout constants for Game Boy screen positioning
use gui::layout::{GB_X, GB_Y, GB_WIDTH, GB_HEIGHT, GB_BORDER, GB_BORDER_COLOR};

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    defensive::diagnostic_panic(info)
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
// Kernel Main
// ============================================================================

#[no_mangle]
extern "C" fn kernel_main(_boot_info_ptr: u32) -> ! {
    // ========================================================================
    // Early init - no stack guard yet (need to know memory map first)
    // ========================================================================
    set_last_operation(OperationId::BootStart);

    // Parse boot info from fixed address 0x500
    let boot_info = unsafe { BootInfo::from_ptr(0x500 as *const u8) };

    // Initialize GDT
    set_last_operation(OperationId::GdtInit);
    gdt::init();

    // Initialize IDT
    set_last_operation(OperationId::IdtInit);
    idt::init();

    // Initialize memory manager
    set_last_operation(OperationId::HeapInit);
    mm::init(boot_info.e820_map_addr);

    // ========================================================================
    // NOW it's safe to init stack guard (memory manager initialized)
    // ========================================================================
    unsafe { defensive::init_stack_guard(); };

    // Initialize storage subsystem
    set_last_operation(OperationId::AtaInit);
    let storage_result = storage::init();

    // Enable interrupts
    unsafe { core::arch::asm!("sti"); }

    // Test disk read if devices found
    if storage_result.ata_devices > 0 {
        storage::test_read();

        // Mount FAT32
        set_last_operation(OperationId::Fat32Mount);
        let mount_result = storage::fat32::mount(0);

        if mount_result.is_ok() {
            // Show ROM browser and get selection
            if let Some(rom_index) = rom_browser::select_rom() {
                // Load selected ROM
                set_last_operation(OperationId::RomLoad);
                if let Some((rom_ptr, rom_size)) = load_rom(rom_index) {
                    // Clear screen
                    clear_screen(0x00);

                    // Initialize VGA palette for GBC colors NOW
                    vga_palette::init_palette();

                    // Draw Game Boy border
                    draw_gb_border();

                    // Run emulator with selected ROM
                    set_last_operation(OperationId::EmulatorInit);
                    run_gameboy_emulator_with_rom(rom_ptr, rom_size);
                }
            }
        }
    }

    // Halt if we get here
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

// ============================================================================
// ROM Loading
// ============================================================================

// Static ROM buffer - must be outside the function to have stable address
static mut ROM_BUFFER: [u8; 2 * 1024 * 1024] = [0; 2 * 1024 * 1024]; // 2MB max

/// Load ROM at given index from FAT32
/// Returns (pointer to ROM data, size) if successful
fn load_rom(index: usize) -> Option<(*const u8, usize)> {
    // Find ROM at index
    let (cluster, size) = match storage::fat32::get_fs().find_rom(index) {
        Some(info) => info,
        None => {
            return None;
        }
    };


    // Load ROM data into static buffer
    let rom_buf = unsafe { &mut ROM_BUFFER };

    match storage::fat32::get_fs().read_file(cluster, size, rom_buf) {
        Ok(bytes_read) => {
            // Verify we got some data
            if bytes_read == 0 {
                return None;
            }

            // Wait 2 seconds so user can see debug output
            for _ in 0..2000 {
                for _ in 0..10000 {
                    unsafe { core::arch::asm!("nop"); }
                }
            }

            Some((rom_buf.as_ptr(), bytes_read))
        }
        Err(_) => {
            // Magenta bar = read error
            None
        }
    }
}

// ============================================================================
// VGA Mode 13h Display Functions
// ============================================================================

/// Clear the entire VGA screen
fn clear_screen(color: u8) {
    defensive::safe_fill_rect(0, 0, defensive::VGA_WIDTH, defensive::VGA_HEIGHT, color);
}

/// Draw the Game Boy screen border using layout constants
fn draw_gb_border() {
    let start_x = GB_X;
    let start_y = GB_Y;
    let gb_width = GB_WIDTH;
    let gb_height = GB_HEIGHT;
    let border = GB_BORDER;

    // Top border
    if start_y >= border {
        defensive::safe_fill_rect(
            start_x.saturating_sub(border),
            start_y - border,
            gb_width + border * 2,
            border,
            GB_BORDER_COLOR,
        );
    }

    // Bottom border
    defensive::safe_fill_rect(
        start_x.saturating_sub(border),
        start_y + gb_height,
        gb_width + border * 2,
        border,
        GB_BORDER_COLOR,
    );

    // Left border
    if start_x >= border {
        defensive::safe_fill_rect(
            start_x - border,
            start_y,
            border,
            gb_height,
            GB_BORDER_COLOR,
        );
    }

    // Right border
    defensive::safe_fill_rect(
        start_x + gb_width,
        start_y,
        border,
        gb_height,
        GB_BORDER_COLOR,
    );
}

/// Blit Game Boy framebuffer (160x144 RGB) to VGA mode 13h (320x200)
///
/// The GB GPU outputs RGB format (3 bytes per pixel).
/// We convert to VGA palette indices using grayscale approximation.
///
/// VGA mode 13h default palette has grayscale at indices 16-31.
/// Fast palette-indexed blit for VGA mode 13h
fn blit_gb_to_vga_fast(pal_data: &[u8]) {
    unsafe {
        let vga = 0xA0000 as *mut u8;
        for y in 0..GB_HEIGHT {
            let src = pal_data.as_ptr().add(y * GB_WIDTH);
            let dst = vga.add((GB_Y + y) * 320 + GB_X);
            core::ptr::copy_nonoverlapping(src, dst, GB_WIDTH);
        }
    }
}

// ============================================================================
// GameBoy Emulator Integration
// ============================================================================

/// Run emulator with ROM loaded from FAT32
fn run_gameboy_emulator_with_rom(rom_ptr: *const u8, rom_size: usize) -> ! {
    use alloc::vec::Vec;
    use crate::overlay::{Game, RamReader, render_overlay};

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

    // Detect game for overlay (do once at startup)
    let game = Game::detect(&device.romname());
    let overlay_enabled = false;

    // Create input handler
    let input_state = gameboy::input::InputState::new();

    // Frame timing: 59.7 fps = ~16.75ms per frame
    const TICKS_PER_FRAME: u32 = 17;
    let mut last_frame_ticks = arch::x86::pit::ticks();

    // VGA framebuffer for overlay rendering
    let vga_buffer: &mut [u8] = unsafe {
        core::slice::from_raw_parts_mut(0xA0000 as *mut u8, 320 * 200)
    };

    // Main emulation loop
    const CYCLES_PER_FRAME: u32 = 70224;

    // ========================================================================
    // MAIN EMULATION LOOP - with defensive instrumentation
    // ========================================================================
    loop {
        set_last_operation(OperationId::FrameStart);
        defensive::increment_frame_count();

        // ====================================================================
        // Periodic health check (once per frame is cheap)
        // ====================================================================
        if defensive::check_stack_overflow() {
            panic!("Stack overflow detected in emulation loop!");
        }

        // Run one frame of emulation
        set_last_operation(OperationId::CpuCycle);
        let mut cycles: u32 = 0;
        while cycles < CYCLES_PER_FRAME {
            cycles += device.do_cycle();
        }

        // Blit to screen if GPU updated
        if device.check_and_reset_gpu_updated() {
            set_last_operation(OperationId::GpuRender);
            let gpu_data = device.get_gpu_data();

            // Sync GBC palettes to VGA DAC
            if device.mode() == GbMode::Color {
                vga_palette::sync_gbc_bg_palettes(device.get_cbgpal());
                vga_palette::sync_gbc_sprite_palettes(device.get_csprit());
            } else {
                let (palb, pal0, pal1) = device.get_dmg_palettes();
                vga_palette::sync_dmg_palettes(palb, pal0, pal1);
            }

            set_last_operation(OperationId::VgaBlit);
            blit_gb_to_vga_fast(device.get_pal_data());

            // Render overlay (reads RAM via peek - no side effects)
            if overlay_enabled {
                let reader = RamReader::new(device.mmu(), game);
                render_overlay(vga_buffer, &reader, game);
            }
        }

        // Process keyboard input
        set_last_operation(OperationId::KeyboardPoll);
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
        set_last_operation(OperationId::FrameEnd);
        let target_ticks = last_frame_ticks.wrapping_add(TICKS_PER_FRAME);
        while arch::x86::pit::ticks().wrapping_sub(target_ticks) > 0x8000_0000 {
            unsafe { core::arch::asm!("hlt"); }
        }
        last_frame_ticks = target_ticks;
    }
}

/// Show "NO ROM" error on screen
fn show_no_rom_error() {
    const START_Y: usize = 80;
    const START_X: usize = 120;

    // Draw red "X" pattern to indicate error
    for i in 0..40usize {
        defensive::safe_put_pixel(START_X + i, START_Y + i, 0x04);  // Red
        defensive::safe_put_pixel(START_X + 40 - i, START_Y + i, 0x04);
    }
}

/// Show emulator initialization error
fn show_emulator_error() {
    const START_Y: usize = 80;

    // Draw yellow bar to indicate emulator error
    defensive::safe_fill_rect(100, START_Y, 120, 1, 0x0E);  // Yellow
}
