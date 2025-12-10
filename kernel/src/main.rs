//! Rustacean OS Kernel Entry Point (with GameBoy Mode)
//!
//! This is where the bootloader hands off control to the kernel.
//! We're in 32-bit protected mode with a flat memory model.
//!
//! # Dual Mode Operation
//!
//! The kernel now supports two modes:
//! 1. **Normal Mode** (magic='RUST') - Full Rustacean OS with windowing GUI
//! 2. **GameBoy Mode** (magic='GBOY') - Boots directly into GameBoy emulator
//!
//! Mode is determined by bootloader based on whether a game ROM was loaded.
//!
//! # EventChains Architecture (Normal Mode)
//!
//! 1. **Driver EventChain** - Fault-tolerant driver initialization
//! 2. **Kernel EventChain** - Syscall processing with middleware
//! 3. **Window Manager EventChain** - Window lifecycle events

#![no_std]
#![no_main]
#![allow(dead_code)]

extern crate alloc;

// Core kernel modules
mod boot_info;
mod arch;
mod mm;
mod sched;
mod event_chains;
mod syscall;
mod drivers;
mod fs;
mod gui;

// GameBoy emulator module (new!)
mod gameboy;

use boot_info::BootInfo;
use arch::x86::{gdt, idt};

use core::fmt::Write;
use core::arch::global_asm;

// Panic handler
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(writer) = unsafe { drivers::vga::WRITER.as_mut() } {
        let _ = writeln!(writer, "\n!!! KERNEL PANIC !!!");
        let _ = writeln!(writer, "{}", info);
    }
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}

// Assembly entry point with debug
global_asm!(
    ".section .text.boot",
    ".global _start",
    "_start:",
    // Write "K=" at positions 2-3 to confirm _start runs
    "    mov byte ptr [0xB8002], 0x4B",   // 'K' - kernel reached
    "    mov byte ptr [0xB8003], 0x2F",   // green
    "    mov byte ptr [0xB8004], 0x3D",   // '='
    "    mov byte ptr [0xB8005], 0x2F",   // green

    // Push boot_info and call kernel_main
    "    push eax",
    "    call kernel_main",

    // If kernel_main returns (shouldn't happen)
    "    mov byte ptr [0xB8006], 0x58",   // 'X' = returned
    "    mov byte ptr [0xB8007], 0x4F",   // red
    "2:",
    "    cli",
    "    hlt",
    "    jmp 2b",
);

/// Main kernel initialization
#[no_mangle]
extern "C" fn kernel_main(boot_info_ptr: u32) -> ! {
    // FIRST THING: Write "M!" to VGA to prove we're in kernel_main
    unsafe {
        core::ptr::write_volatile(0xB8008 as *mut u8, b'M');  // 'M' = in Main
        core::ptr::write_volatile(0xB8009 as *mut u8, 0x2F); // green
        core::ptr::write_volatile(0xB800A as *mut u8, b'!');
        core::ptr::write_volatile(0xB800B as *mut u8, 0x2F);
    }

    // Write boot_info_ptr value as hex (positions 6-13)
    unsafe {
        let ptr = boot_info_ptr;
        for i in 0..8 {
            let nibble = ((ptr >> (28 - i * 4)) & 0xF) as u8;
            let c = if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 };
            core::ptr::write_volatile((0xB800C + i * 2) as *mut u8, c);
            core::ptr::write_volatile((0xB800D + i * 2) as *mut u8, 0x2F);
        }
    }

    // Write "1" - about to parse boot_info
    unsafe {
        core::ptr::write_volatile(0xB801C as *mut u8, b'1');
        core::ptr::write_volatile(0xB801D as *mut u8, 0x2F);
    }

    // Check pointer validity
    if boot_info_ptr == 0 || boot_info_ptr > 0x100000 {
        unsafe {
            core::ptr::write_volatile(0xB801E as *mut u8, b'!');
            core::ptr::write_volatile(0xB801F as *mut u8, 0x4F); // red
        }
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    // Write "2" - pointer valid
    unsafe {
        core::ptr::write_volatile(0xB801E as *mut u8, b'2');
        core::ptr::write_volatile(0xB801F as *mut u8, 0x2F);
    }

    // Parse boot info
    let boot_info = unsafe {
        BootInfo::from_ptr(boot_info_ptr as *const u8)
    };

    // Write "3" - parsed
    unsafe {
        core::ptr::write_volatile(0xB8020 as *mut u8, b'3');
        core::ptr::write_volatile(0xB8021 as *mut u8, 0x2F);
    }

    // Verify boot info
    if !boot_info.verify_magic() {
        unsafe {
            let vga = 0xB8000 as *mut u8;
            let msg = b"BAD MAGIC";
            for (i, &c) in msg.iter().enumerate() {
                core::ptr::write_volatile(vga.offset(160 + i as isize * 2), c);
                core::ptr::write_volatile(vga.offset(161 + i as isize * 2), 0x4F);
            }
        }
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    // Write "4" - magic OK
    unsafe {
        core::ptr::write_volatile(0xB8022 as *mut u8, b'4');
        core::ptr::write_volatile(0xB8023 as *mut u8, 0x2F);
    }

    // Initialize GDT
    gdt::init();

    // Write "5" - GDT OK
    unsafe {
        core::ptr::write_volatile(0xB8024 as *mut u8, b'5');
        core::ptr::write_volatile(0xB8025 as *mut u8, 0x2F);
    }

    // Initialize VGA text mode for boot messages (VESA may not be available)
    unsafe {
        if boot_info.vesa_enabled && boot_info.framebuffer_addr != 0xB8000 {
            drivers::vga::init_framebuffer(
                boot_info.framebuffer_addr,
                boot_info.screen_width,
                boot_info.screen_height,
                boot_info.bits_per_pixel,
                boot_info.pitch,
            );
        } else {
            drivers::vga::init_text_mode();
        }
    }

    // Write "6" - VGA OK
    unsafe {
        core::ptr::write_volatile(0xB8026 as *mut u8, b'6');
        core::ptr::write_volatile(0xB8027 as *mut u8, 0x2F);
    }

    let writer = unsafe { drivers::vga::WRITER.as_mut().unwrap() };
    let _ = writeln!(writer, "");
    let _ = writeln!(writer, "[KERNEL] Entered protected mode!");

    // Initialize IDT
    let _ = write!(writer, "[INIT] Loading IDT...");
    idt::init();
    let _ = writeln!(writer, " OK");

    // Initialize memory manager
    let _ = write!(writer, "[INIT] Memory...");
    let mem_info = mm::init(boot_info.e820_map_addr);
    let _ = writeln!(writer, " OK ({} KB)", mem_info.usable_kb);

    // Enable interrupts
    unsafe { core::arch::asm!("sti"); }

    // =========================================================================
    // Check for GameBoy Mode
    // =========================================================================

    if boot_info.is_gameboy_mode() {
        let _ = writeln!(writer, "");
        let _ = writeln!(writer, "[MODE] GameBoy Mode Detected!");
        let _ = writeln!(writer, "[ROM ] Address: 0x{:08X}", boot_info.rom_addr);
        let _ = writeln!(writer, "[ROM ] Size: {} bytes", boot_info.rom_size);

        let title = unsafe { boot_info.rom_title() };
        let _ = writeln!(writer, "[ROM ] Title: {}", title);
        let _ = writeln!(writer, "");

        // Initialize drivers (ATI Rage will give us a framebuffer)
        let _ = writeln!(writer, "[DRV ] Initializing drivers...");

        // Use VESA info if available, otherwise use defaults for driver init
        let (fb_addr, width, height, bpp, pitch) = if boot_info.vesa_enabled {
            (boot_info.framebuffer_addr, boot_info.screen_width,
             boot_info.screen_height, boot_info.bits_per_pixel / 8, boot_info.pitch)
        } else {
            // Defaults - driver will override with actual values
            (0, 800, 600, 4, 800 * 4)
        };

        let drv_result = drivers::init_all_drivers(fb_addr, width, height, bpp, pitch);

        let _ = writeln!(writer, "[DRV ] GPU: {}", drv_result.gpu_type_str());
        let _ = writeln!(writer, "[DRV ] Framebuffer: 0x{:08X}", drv_result.fb_addr);
        let _ = writeln!(writer, "[DRV ] Resolution: {}x{}x{}",
                         drv_result.width, drv_result.height, drv_result.bpp * 8);

        // Run GameBoy mode with the driver-provided framebuffer
        run_gameboy_mode(boot_info, drv_result, writer);
    }

    // =========================================================================
    // Normal Rustacean OS Mode
    // =========================================================================
    let _ = writeln!(writer, "[MODE] Normal Rustacean OS Mode");
    let _ = writeln!(writer, "[BOOT] Magic: 0x{:08X}", boot_info.magic);

    // Start GUI using drivers (ATI Rage or VESA fallback)
    let _ = writeln!(writer, "[DRV ] Initializing drivers...");

    let (fb_addr, width, height, bpp, pitch) = if boot_info.vesa_enabled {
        (boot_info.framebuffer_addr, boot_info.screen_width,
         boot_info.screen_height, boot_info.bits_per_pixel / 8, boot_info.pitch)
    } else {
        // Defaults - ATI Rage driver will set up its own mode
        (0, 800, 600, 4, 800 * 4)
    };

    let drv_result = drivers::init_all_drivers(fb_addr, width, height, bpp, pitch);

    let _ = writeln!(writer, "[DRV ] GPU: {}", drv_result.gpu_type_str());
    let _ = writeln!(writer, "[DRV ] Input: {}", drv_result.input_type_str());
    let _ = writeln!(writer, "[DRV ] Framebuffer: 0x{:08X}", drv_result.fb_addr);
    let _ = writeln!(writer, "[DRV ] Resolution: {}x{}x{}",
                     drv_result.width, drv_result.height, drv_result.bpp * 8);
    let _ = writeln!(writer, "");
    let _ = writeln!(writer, "[READY] Starting GUI...");

    // Delay to show messages
    for _ in 0..50_000_000u32 {
        unsafe { core::arch::asm!("nop"); }
    }

    run_gui(drv_result);
}

// =============================================================================
// GameBoy Mode
// =============================================================================

/// Run GameBoy emulator mode
///
/// Uses ATI Rage driver (or VESA fallback) for framebuffer graphics.
fn run_gameboy_mode(boot_info: BootInfo, drv: drivers::DriverInitResult, writer: &mut drivers::vga::Writer) -> ! {
    use alloc::vec::Vec;

    let _ = writeln!(writer, "[EMU ] Initializing GameBoy emulator...");

    // Get ROM data from bootloader
    let rom_slice = unsafe {
        boot_info.rom_slice().expect("ROM should be loaded")
    };
    let rom_data: Vec<u8> = rom_slice.to_vec();

    let _ = writeln!(writer, "[EMU ] ROM loaded: {} bytes", rom_data.len());

    // Create emulator device
    let mut device = match gameboy::Device::new(rom_data, false) {
        Ok(d) => d,
        Err(e) => {
            let _ = writeln!(writer, "[FAIL] Emulator init failed: {}", e);
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    };

    let _ = writeln!(writer, "[EMU ] Game: {}", device.romname());
    let _ = writeln!(writer, "[EMU ] Starting emulation...");

    // Small delay to show messages
    for _ in 0..30_000_000u32 {
        unsafe { core::arch::asm!("nop"); }
    }

    // Use driver-provided framebuffer (ATI Rage or VESA)
    let fb_addr = drv.fb_addr as *mut u8;
    let fb_pitch = drv.pitch as usize;
    let fb_width = drv.width as usize;
    let fb_height = drv.height as usize;
    let fb_bpp = drv.bpp * 8; // Driver stores bytes, display expects bits

    // Clear screen with dark border color
    unsafe {
        gameboy::display::clear_borders(fb_addr, fb_pitch, fb_width, fb_height, fb_bpp);
    }

    // Create input mapper
    let mut input_state = gameboy::input::InputState::new();

    // Main emulator loop
    run_emulator_loop(&mut device, &mut input_state, fb_addr, fb_pitch, fb_bpp);
}

/// GameBoy emulator main loop
fn run_emulator_loop(
    device: &mut gameboy::Device,
    input: &mut gameboy::input::InputState,
    fb: *mut u8,
    pitch: usize,
    bpp: u32,
) -> ! {
    const CYCLES_PER_FRAME: u32 = 70224;

    loop {
        // Run one frame of emulation
        let mut cycles: u32 = 0;
        while cycles < CYCLES_PER_FRAME {
            cycles += device.do_cycle();
        }

        // Blit to screen if GPU updated
        if device.check_and_reset_gpu_updated() {
            unsafe {
                gameboy::display::blit_scaled(
                    device.get_gpu_data(),
                    fb,
                    pitch,
                    bpp,
                );
            }
        }

        // Process keyboard input (poll from existing driver)
        while let Some(key) = drivers::keyboard::get_key() {
            // Map Rustacean OS KeyCode to GameBoy KeypadKey
            if let Some(gb_key) = input.map_keycode(key.keycode) {
                if key.pressed {
                    device.keydown(gb_key);
                } else {
                    device.keyup(gb_key);
                }
            }
        }

        // Wait for next frame (HLT until timer interrupt)
        unsafe { core::arch::asm!("hlt"); }
    }
}

// =============================================================================
// Normal GUI Mode (existing code)
// =============================================================================
static mut BACK_BUFFER_DATA: [u8; 800 * 600 * 4] = [0u8; 800 * 600 * 4];

/// Run the graphical user interface
///
/// Uses:
/// - Driver EventChain results for display/input configuration
/// - Window Manager EventChain for discrete window events
/// - Direct calls for hot path (mouse tracking, rendering)
fn run_gui(drv: drivers::DriverInitResult) -> ! {
    // Create back buffer for double buffering
    let mut back_buffer = unsafe {
        gui::Framebuffer::new(
            BACK_BUFFER_DATA.as_mut_ptr(),
            drv.width,
            drv.height,
            drv.bpp,
            drv.pitch,
        )
    };

    // Initialize desktop window manager with hardware cursor support
    gui::desktop::init_with_hw_cursor(drv.width, drv.height, drv.hw_cursor);

    let desktop = gui::desktop::get().expect("Desktop not initialized");
    let fb = gui::framebuffer::get().expect("Framebuffer not initialized");

    // Create demo windows (goes through WM EventChain)
    desktop.create_window("Welcome to Rustacean OS!", 50, 50, 450, 220);
    desktop.create_terminal_window(100, 280, 400, 180);  // Heap-allocated terminal!
    desktop.create_window("Files", 470, 50, 300, 220);

    desktop.mark_dirty();

    // =========================================================================
    // Main GUI event loop (Polling Mode)
    // =========================================================================

    // Disable keyboard (IRQ1) and mouse (IRQ12) interrupts - we'll poll instead
    // This avoids race conditions between IRQ handlers and our polling loop
    unsafe {
        // Disable IRQ1 (keyboard) on master PIC
        let mask = crate::arch::x86::io::inb(0x21);
        crate::arch::x86::io::outb(0x21, mask | 0x02);  // Set bit 1

        // Disable IRQ12 (mouse) on slave PIC
        let mask = crate::arch::x86::io::inb(0xA1);
        crate::arch::x86::io::outb(0xA1, mask | 0x10);  // Set bit 4
    }

    let mut last_mouse_x = (drv.width / 2) as i32;
    let mut last_mouse_y = (drv.height / 2) as i32;
    let mut last_buttons = 0u8;

    // Keyboard-controlled cursor (fallback)
    let mut kb_cursor_x = last_mouse_x;
    let mut kb_cursor_y = last_mouse_y;
    let cursor_speed = 8i32;

    let using_synaptics = drv.is_synaptics();
    let using_ati_rage = drv.is_ati_rage();

    loop {
        // =====================================================================
        // Poll PS/2 controller - route keyboard and mouse data to drivers
        // =====================================================================
        unsafe {
            let status = crate::arch::x86::io::inb(0x64);

            // Check if output buffer has data (bit 0)
            if status & 0x01 != 0 {
                let data = crate::arch::x86::io::inb(0x60);

                // Bit 5 tells us if it's from auxiliary device (mouse/touchpad)
                if status & 0x20 == 0 {
                    // Keyboard data - process through keyboard driver
                    drivers::keyboard::KEYBOARD.process_scancode(data);
                } else {
                    // Mouse/touchpad data - route to appropriate driver
                    if using_synaptics {
                        drivers::synaptics::handle_irq_byte(data);
                    } else {
                        drivers::mouse::MOUSE.process_byte(data);
                    }
                }
            }
        }

        // =====================================================================
        // Handle keyboard input - poll driver buffer
        // =====================================================================
        while let Some(key) = drivers::keyboard::get_key() {
            use drivers::keyboard::KeyCode;

            if desktop.is_terminal_focused() {
                // Terminal input mode
                match key.keycode {
                    KeyCode::Enter => desktop.term_enter(),
                    KeyCode::Backspace => desktop.term_backspace(),
                    KeyCode::Up => {
                        kb_cursor_y = (kb_cursor_y - cursor_speed).max(0);
                        desktop.handle_mouse_move(kb_cursor_x, kb_cursor_y);
                    }
                    KeyCode::Down => {
                        kb_cursor_y = (kb_cursor_y + cursor_speed).min(drv.height as i32 - 1);
                        desktop.handle_mouse_move(kb_cursor_x, kb_cursor_y);
                    }
                    KeyCode::Left => {
                        kb_cursor_x = (kb_cursor_x - cursor_speed).max(0);
                        desktop.handle_mouse_move(kb_cursor_x, kb_cursor_y);
                    }
                    KeyCode::Right => {
                        kb_cursor_x = (kb_cursor_x + cursor_speed).min(drv.width as i32 - 1);
                        desktop.handle_mouse_move(kb_cursor_x, kb_cursor_y);
                    }
                    _ => {
                        // Send printable characters to terminal
                        if let Some(c) = key.ascii {
                            desktop.term_key_input(c);
                        }
                    }
                }
            } else {
                // Window navigation mode
                match key.keycode {
                    KeyCode::Up | KeyCode::W => {
                        kb_cursor_y = (kb_cursor_y - cursor_speed).max(0);
                        desktop.handle_mouse_move(kb_cursor_x, kb_cursor_y);
                    }
                    KeyCode::Down | KeyCode::S => {
                        kb_cursor_y = (kb_cursor_y + cursor_speed).min(drv.height as i32 - 1);
                        desktop.handle_mouse_move(kb_cursor_x, kb_cursor_y);
                    }
                    KeyCode::Left | KeyCode::A => {
                        kb_cursor_x = (kb_cursor_x - cursor_speed).max(0);
                        desktop.handle_mouse_move(kb_cursor_x, kb_cursor_y);
                    }
                    KeyCode::Right | KeyCode::D => {
                        kb_cursor_x = (kb_cursor_x + cursor_speed).min(drv.width as i32 - 1);
                        desktop.handle_mouse_move(kb_cursor_x, kb_cursor_y);
                    }
                    KeyCode::Enter => unsafe {
                        desktop.handle_mouse_button(gui::MouseButton::Left, true);
                        for _ in 0..100000u32 { core::arch::asm!("nop"); }
                        desktop.handle_mouse_button(gui::MouseButton::Left, false);
                    }
                    KeyCode::Space => {
                        desktop.handle_mouse_button(gui::MouseButton::Left, true);
                    }
                    _ => {}
                }
            }
        }

        // =====================================================================
        // Handle pointing device input (direct - hot path)
        // =====================================================================
        let (mouse_x, mouse_y, buttons) = if using_synaptics {
            let (x, y) = drivers::synaptics::get_position();
            let btns = drivers::synaptics::get_buttons();
            (x, y, btns)
        } else {
            let (x, y) = drivers::mouse::get_position();
            let btns = drivers::mouse::get_buttons();
            (x, y, btns)
        };

        if mouse_x != last_mouse_x || mouse_y != last_mouse_y {
            desktop.handle_mouse_move(mouse_x, mouse_y);
            kb_cursor_x = mouse_x;
            kb_cursor_y = mouse_y;

            if using_ati_rage {
                if let Some(gpu) = drivers::ati_rage::get() {
                    gpu.set_cursor_pos(mouse_x, mouse_y);
                }
            }

            last_mouse_x = mouse_x;
            last_mouse_y = mouse_y;
        }

        if buttons != last_buttons {
            if (buttons & 0x01) != (last_buttons & 0x01) {
                desktop.handle_mouse_button(gui::MouseButton::Left, buttons & 0x01 != 0);
            }
            if (buttons & 0x02) != (last_buttons & 0x02) {
                desktop.handle_mouse_button(gui::MouseButton::Right, buttons & 0x02 != 0);
            }
            if (buttons & 0x04) != (last_buttons & 0x04) {
                desktop.handle_mouse_button(gui::MouseButton::Middle, buttons & 0x04 != 0);
            }
            last_buttons = buttons;
        }

        // =====================================================================
        // Draw the desktop (direct - hot path, double buffered)
        // =====================================================================
        desktop.draw(&mut back_buffer, fb);

        // Small yield
        for _ in 0..10000u32 {
            unsafe { core::arch::asm!("nop"); }
        }
    }
}
