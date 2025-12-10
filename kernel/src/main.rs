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

// Optional modules - comment out for minimal boot testing
// mod sched;
// mod syscall;
// mod fs;
 mod gui;

// GameBoy emulator - uncomment once basic boot works
// mod gameboy;

use boot_info::BootInfo;
use arch::x86::{gdt, idt};
use core::arch::global_asm;

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Try to write panic info to VGA
    unsafe {
        // Draw red bar at top of screen
        let vga = 0xA0000 as *mut u8;
        for i in 0..320 {
            core::ptr::write_volatile(vga.add(i), 0x04); // Red
        }

        // If we have a VGA writer, use it
        if let Some(writer) = drivers::vga::WRITER.as_mut() {
            use core::fmt::Write;
            let _ = writeln!(writer, "\n!!! KERNEL PANIC !!!");
            let _ = writeln!(writer, "{}", info);
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

/// Draw debug value as colored blocks
#[inline(always)]
unsafe fn draw_hex(value: u32, row: isize) {
    let vga = 0xA0000 as *mut u8;
    for i in 0..8 {
        let nibble = ((value >> (28 - i * 4)) & 0xF) as u8;
        let color = 0x10 + nibble;
        for j in 0..8 {
            core::ptr::write_volatile(vga.offset(row + (i * 10 + j) as isize), color);
        }
    }
}

// ============================================================================
// Kernel Main
// ============================================================================

#[no_mangle]
extern "C" fn kernel_main(boot_info_ptr: u32) -> ! {
    const ROW: isize = 320 * 10;  // Row 10 for progress

    // Stage M1: In kernel_main - MAGENTA
    unsafe { draw_bar(0, ROW, 0x05, 20); }

    // Parse boot info
    let boot_info = unsafe { BootInfo::from_ptr(boot_info_ptr as *const u8) };

    // Stage M2: Parsed - GREEN
    unsafe { draw_bar(20, ROW, 0x02, 20); }

    // Debug: Show magic value on row 14
    unsafe { draw_hex(boot_info.magic, 320 * 14); }

    // Verify magic ('GBOY' = 0x594F4247)
    if !boot_info.verify_magic() {
        // Bad magic - RED bar on row 12
        unsafe { draw_bar(0, 320 * 12, 0x04, 200); }
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    // Stage M3: Magic OK - YELLOW
    unsafe { draw_bar(40, ROW, 0x0E, 20); }

    // Initialize GDT
    gdt::init();

    // Stage M4: GDT OK - LIGHT CYAN
    unsafe { draw_bar(60, ROW, 0x0B, 20); }

    // Initialize IDT
    idt::init();

    // Stage M5: IDT OK - CYAN
    unsafe { draw_bar(80, ROW, 0x03, 20); }

    // Initialize memory manager
    mm::init(boot_info.e820_map_addr);

    // Stage M6: Memory OK - LIGHT GREEN
    unsafe { draw_bar(100, ROW, 0x0A, 20); }

    // Enable interrupts
    unsafe { core::arch::asm!("sti"); }

    // Stage M7: All init done - WHITE
    unsafe { draw_bar(120, ROW, 0x0F, 40); }

    // =========================================================================
    // Draw Game Boy screen placeholder
    // =========================================================================
    // GB is 160x144, we have 320x200
    // Center it: x = (320-160)/2 = 80, y = (200-144)/2 = 28

    unsafe {
        let vga = 0xA0000 as *mut u8;
        let start_x: isize = 80;
        let start_y: isize = 28;

        // Draw border around GB screen (dark gray)
        let border_color: u8 = 0x08;

        // Top border
        for x in (start_x - 4)..(start_x + 164) {
            for y_off in 0..4 {
                let offset = (start_y - 4 + y_off) * 320 + x;
                if offset >= 0 && offset < 64000 {
                    core::ptr::write_volatile(vga.offset(offset), border_color);
                }
            }
        }

        // Bottom border
        for x in (start_x - 4)..(start_x + 164) {
            for y_off in 0..4 {
                let offset = (start_y + 144 + y_off) * 320 + x;
                if offset >= 0 && offset < 64000 {
                    core::ptr::write_volatile(vga.offset(offset), border_color);
                }
            }
        }

        // Left border
        for y in (start_y - 4)..(start_y + 148) {
            for x_off in 0..4 {
                let offset = y * 320 + start_x - 4 + x_off;
                if offset >= 0 && offset < 64000 {
                    core::ptr::write_volatile(vga.offset(offset), border_color);
                }
            }
        }

        // Right border
        for y in (start_y - 4)..(start_y + 148) {
            for x_off in 0..4 {
                let offset = y * 320 + start_x + 160 + x_off;
                if offset >= 0 && offset < 64000 {
                    core::ptr::write_volatile(vga.offset(offset), border_color);
                }
            }
        }

        // Draw GB LCD area - classic Game Boy green gradient
        for y in 0..144 {
            for x in 0..160 {
                let offset = (start_y + y as isize) * 320 + start_x + x as isize;
                // Classic GB greenish color (palette ~0x32-0x3A)
                let color = 0x32 + ((x + y) % 8) as u8;
                core::ptr::write_volatile(vga.offset(offset), color);
            }
        }
    }

    // Stage M8: Display ready - BRIGHT WHITE bar
    unsafe { draw_bar(160, ROW, 0x0F, 40); }

    // =========================================================================
    // Main loop - wait for emulator implementation
    // =========================================================================
    // TODO:
    // 1. Load ROM from floppy or embedded data
    // 2. Initialize Game Boy CPU, PPU, APU
    // 3. Run emulation loop
    // 4. Blit GB framebuffer to VGA
    // 5. Handle keyboard input

    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}
