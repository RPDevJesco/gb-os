//! GB-OS for Raspberry Pi Zero 2 W
//!
//! Bare-metal Game Boy emulator for the GPi Case 2W.
//!
//! # Module Organization
//!
//! - `mmio` - Low-level memory-mapped I/O
//! - `gpio` - GPIO pin control
//! - `timer` - System timer for frame pacing
//! - `mailbox` - VideoCore GPU communication
//! - `framebuffer` - Display and drawing operations
//! - `input` - GPi Case button handling
//! - `emulator` - Game Boy emulator integration
//! - `entry` - Assembly boot code

#![no_std]
#![no_main]
#![feature(naked_functions)]

// ============================================================================
// Modules
// ============================================================================

mod mmio;
mod gpio;
mod timer;
mod mailbox;
mod framebuffer;
mod input;
mod emulator;
mod entry;

// ============================================================================
// Imports
// ============================================================================

use framebuffer::{Framebuffer, colors};
use input::{Button, MenuAction};

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Try to display panic message on screen
    // (May not work if framebuffer isn't initialized)

    // For debugging, blink the ACT LED in a distinctive pattern
    gpio::init_act_led();

    loop {
        // SOS pattern: ... --- ...
        for _ in 0..3 {
            gpio::act_led_on();
            timer::delay_ms(100);
            gpio::act_led_off();
            timer::delay_ms(100);
        }
        timer::delay_ms(200);
        for _ in 0..3 {
            gpio::act_led_on();
            timer::delay_ms(300);
            gpio::act_led_off();
            timer::delay_ms(100);
        }
        timer::delay_ms(200);
        for _ in 0..3 {
            gpio::act_led_on();
            timer::delay_ms(100);
            gpio::act_led_off();
            timer::delay_ms(100);
        }
        timer::delay_ms(1000);
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Display configuration
mod config {
    /// Display width (GPi Case 2W native is 640x480)
    pub const DISPLAY_WIDTH: u32 = 640;
    /// Display height
    pub const DISPLAY_HEIGHT: u32 = 480;
    /// Color depth (bits per pixel)
    pub const DISPLAY_DEPTH: u32 = 32;
}

// ============================================================================
// Kernel Entry Point
// ============================================================================

/// Main kernel entry point (called from assembly).
#[no_mangle]
pub extern "C" fn kernel_main(_core_id: u64) -> ! {
    // ========================================================================
    // Phase 1: Early Hardware Init
    // ========================================================================

    // Initialize ACT LED for status feedback
    gpio::init_act_led();
    gpio::act_led_on();

    // Initialize GPIO for DPI display
    gpio::configure_for_dpi();

    // ========================================================================
    // Phase 2: Initialize Framebuffer
    // ========================================================================

    let mut fb = match Framebuffer::new(
        config::DISPLAY_WIDTH,
        config::DISPLAY_HEIGHT,
        config::DISPLAY_DEPTH,
    ) {
        Some(fb) => fb,
        None => {
            // Framebuffer allocation failed - blink LED rapidly
            blink_error_pattern();
        }
    };

    // Clear screen to show we're alive
    fb.clear(colors::BLACK);

    // Draw boot message
    fb.draw_string(10, 10, "GB-OS Pi Zero 2 W", colors::GREEN, colors::BLACK);
    fb.draw_string(10, 26, "Initializing...", colors::WHITE, colors::BLACK);

    // ========================================================================
    // Phase 3: Initialize Input
    // ========================================================================

    input::init();
    fb.draw_string(10, 42, "Input OK", colors::GREEN, colors::BLACK);

    // LED off to indicate successful init
    gpio::act_led_off();

    // ========================================================================
    // Phase 4: Main Menu / ROM Browser
    // ========================================================================

    // For now, show a test screen since we don't have storage yet
    fb.clear(colors::BLACK);
    show_test_screen(&mut fb);

    // ========================================================================
    // Phase 5: Main Loop
    // ========================================================================

    main_loop(&mut fb);
}

// ============================================================================
// Test Screen
// ============================================================================

/// Display a test screen to verify everything works.
fn show_test_screen(fb: &mut Framebuffer) {
    // Title
    fb.draw_string(200, 20, "GB-OS Test Screen", colors::CYAN, colors::BLACK);
    fb.draw_string(180, 40, "Pi Zero 2 W / GPi Case 2W", colors::WHITE, colors::BLACK);

    // Draw Game Boy border
    fb.draw_gb_border(colors::GRAY);

    // Instructions
    fb.draw_string(200, 420, "Press buttons to test", colors::WHITE, colors::BLACK);
    fb.draw_string(200, 436, "HOME to exit", colors::YELLOW, colors::BLACK);

    // Draw button status area
    fb.draw_string(50, 380, "D-Pad:", colors::LIGHT_GRAY, colors::BLACK);
    fb.draw_string(50, 396, "Face:", colors::LIGHT_GRAY, colors::BLACK);
    fb.draw_string(50, 412, "Menu:", colors::LIGHT_GRAY, colors::BLACK);
}

// ============================================================================
// Main Loop
// ============================================================================

/// Main application loop.
fn main_loop(fb: &mut Framebuffer) -> ! {
    let mut frame_timer = timer::FrameTimer::fps_60();
    let mut last_buttons = 0u16;

    loop {
        // Update input
        input::update();
        let inp = input::get();
        let buttons = inp.state().current;

        // Only update display when buttons change
        if buttons != last_buttons {
            update_button_display(fb, buttons);
            last_buttons = buttons;
        }

        // Check for test emulator
        if inp.just_pressed(Button::Start) && inp.is_pressed(Button::Select) {
            run_test_emulator(fb);
            // Redraw test screen after returning
            fb.clear(colors::BLACK);
            show_test_screen(fb);
        }

        // Frame timing
        frame_timer.wait_for_frame();

        // Toggle LED to show we're running
        static mut LED_COUNTER: u32 = 0;
        unsafe {
            LED_COUNTER += 1;
            if LED_COUNTER >= 30 {
                LED_COUNTER = 0;
                gpio::act_led_toggle();
            }
        }
    }
}

/// Update the button status display.
fn update_button_display(fb: &mut Framebuffer, buttons: u16) {
    // Clear button status area
    fb.fill_rect(110, 380, 200, 48, colors::BLACK);

    // D-Pad
    let mut dpad_str = [b' '; 16];
    let mut idx = 0;
    if buttons & Button::Up as u16 != 0 { dpad_str[idx] = b'U'; idx += 1; }
    if buttons & Button::Down as u16 != 0 { dpad_str[idx] = b'D'; idx += 1; }
    if buttons & Button::Left as u16 != 0 { dpad_str[idx] = b'L'; idx += 1; }
    if buttons & Button::Right as u16 != 0 { dpad_str[idx] = b'R'; idx += 1; }
    let dpad_s = core::str::from_utf8(&dpad_str[..idx.max(1)]).unwrap_or("-");
    fb.draw_string(110, 380, dpad_s, colors::GREEN, colors::BLACK);

    // Face buttons
    let mut face_str = [b' '; 16];
    idx = 0;
    if buttons & Button::A as u16 != 0 { face_str[idx] = b'A'; idx += 1; }
    if buttons & Button::B as u16 != 0 { face_str[idx] = b'B'; idx += 1; }
    if buttons & Button::X as u16 != 0 { face_str[idx] = b'X'; idx += 1; }
    if buttons & Button::Y as u16 != 0 { face_str[idx] = b'Y'; idx += 1; }
    let face_s = core::str::from_utf8(&face_str[..idx.max(1)]).unwrap_or("-");
    fb.draw_string(110, 396, face_s, colors::GREEN, colors::BLACK);

    // Menu/Shoulder
    let mut menu_str = [b' '; 16];
    idx = 0;
    if buttons & Button::Start as u16 != 0 { menu_str[idx..idx+2].copy_from_slice(b"St"); idx += 2; }
    if buttons & Button::Select as u16 != 0 { menu_str[idx..idx+2].copy_from_slice(b"Se"); idx += 2; }
    if buttons & Button::L as u16 != 0 { menu_str[idx] = b'L'; idx += 1; }
    if buttons & Button::R as u16 != 0 { menu_str[idx] = b'R'; idx += 1; }
    if buttons & Button::Home as u16 != 0 { menu_str[idx] = b'H'; idx += 1; }
    let menu_s = core::str::from_utf8(&menu_str[..idx.max(1)]).unwrap_or("-");
    fb.draw_string(110, 412, menu_s, colors::GREEN, colors::BLACK);
}

/// Run the test emulator (placeholder).
fn run_test_emulator(fb: &mut Framebuffer) {
    fb.clear(colors::BLACK);
    fb.draw_string(200, 200, "Test Emulator", colors::CYAN, colors::BLACK);
    fb.draw_string(200, 220, "No ROM loaded", colors::YELLOW, colors::BLACK);
    fb.draw_string(200, 260, "Press B to return", colors::WHITE, colors::BLACK);

    // Create a dummy ROM header for testing
    let dummy_rom = create_dummy_rom();

    // Try to run the emulator with the dummy ROM
    match emulator::Device::new(&dummy_rom) {
        Ok(mut device) => {
            fb.clear(colors::BLACK);
            fb.draw_gb_border(colors::GRAY);

            let mut timer = timer::FrameTimer::gameboy();

            loop {
                input::update();
                let inp = input::get();

                // Exit on B button
                if inp.just_pressed(Button::B) || inp.just_pressed(Button::Home) {
                    break;
                }

                // Update emulator
                device.update_keys(inp.state());
                device.do_frame();

                // Render
                fb.blit_gb_screen_dmg(device.get_frame_buffer());

                timer.wait_for_frame();
            }
        }
        Err(e) => {
            fb.draw_string(200, 280, e, colors::RED, colors::BLACK);

            // Wait for button press
            loop {
                input::update();
                if input::get().just_pressed(Button::B) {
                    break;
                }
                timer::delay_ms(16);
            }
        }
    }
}

/// Create a minimal valid GB ROM header for testing.
fn create_dummy_rom() -> [u8; 0x150] {
    let mut rom = [0u8; 0x150];

    // Nintendo logo (required for boot)
    let logo: [u8; 48] = [
        0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B,
        0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
        0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E,
        0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
        0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC,
        0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
    ];
    rom[0x104..0x134].copy_from_slice(&logo);

    // Title
    rom[0x134..0x140].copy_from_slice(b"TEST ROM\0\0\0\0");

    // CGB flag (supports both DMG and CGB)
    rom[0x143] = 0x80;

    // Cartridge type (ROM only)
    rom[0x147] = 0x00;

    // ROM size (32KB)
    rom[0x148] = 0x00;

    // RAM size (none)
    rom[0x149] = 0x00;

    // Destination (non-Japanese)
    rom[0x14A] = 0x01;

    // Calculate header checksum
    let mut checksum: u8 = 0;
    for i in 0x134..0x14D {
        checksum = checksum.wrapping_sub(rom[i]).wrapping_sub(1);
    }
    rom[0x14D] = checksum;

    rom
}

// ============================================================================
// Error Handling
// ============================================================================

/// Blink LED in error pattern (never returns).
fn blink_error_pattern() -> ! {
    gpio::init_act_led();

    loop {
        // Fast blink indicates error
        gpio::act_led_on();
        timer::delay_ms(100);
        gpio::act_led_off();
        timer::delay_ms(100);
    }
}
