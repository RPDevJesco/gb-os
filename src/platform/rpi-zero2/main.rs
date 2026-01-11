//! # Raspberry Pi Zero 2W Bare-Metal Kernel Entry Point

#![no_std]
#![no_main]

extern crate alloc;

mod platform_core;
mod hal;
mod display;
mod drivers;
mod subsystems;

use alloc::vec;
use core::arch::global_asm;
use core::fmt::Write;
use core::panic::PanicInfo;

use crate::platform_core::allocator;
use crate::display::console::{Console, StringWriter};
use crate::display::framebuffer::{color, Framebuffer};
use crate::display::splash_screen;
use crate::drivers::usb::{UsbHost, Xbox360InputReport};
use crate::hal::gpio;
use crate::platform_core::mmio::{micros, delay_ms};
use crate::subsystems::fat32::{Fat32, Fat32FileSystem};
use crate::subsystems::input::{GpiButtonState, RomSelectorInput, button};

use gameboy::{Emulator, EmulatorConfig, GbMode, KeypadKey};

// =============================================================================
// Assembly Boot Code
// =============================================================================

global_asm!(include_str!("boot.S"));

unsafe extern "C" {
    fn enable_icache();
    fn enable_dcache();
    fn clean_dcache_range(start: usize, size: usize);
}

// =============================================================================
// Constants
// =============================================================================

const FRAME_TIME_US: u32 = 16742;
const FPS_UPDATE_INTERVAL_US: u32 = 1_000_000;
const MAX_ROM_SIZE: usize = 8 * 1024 * 1024;

// =============================================================================
// Exception Context
// =============================================================================

#[repr(C)]
pub struct ExceptionContext {
    pub gpr: [u64; 31],
    pub elr: u64,
    pub spsr: u64,
    pub lr: u64,
}

// =============================================================================
// FPS Counter
// =============================================================================

struct FpsCounter {
    frame_count: u32,
    last_update: u32,
    current_fps: u32,
}

impl FpsCounter {
    const fn new() -> Self {
        Self {
            frame_count: 0,
            last_update: 0,
            current_fps: 0,
        }
    }

    fn init(&mut self) {
        self.last_update = micros();
    }

    fn tick(&mut self) -> bool {
        self.frame_count += 1;
        let now = micros();
        let elapsed = now.wrapping_sub(self.last_update);

        if elapsed >= FPS_UPDATE_INTERVAL_US {
            self.current_fps = (self.frame_count as u64 * 1_000_000 / elapsed as u64) as u32;
            self.frame_count = 0;
            self.last_update = now;
            true
        } else {
            false
        }
    }

    fn fps(&self) -> u32 {
        self.current_fps
    }
}

// =============================================================================
// Input Helpers
// =============================================================================

fn apply_input_changes(
    emu: &mut Emulator,
    old: &GpiButtonState,
    new: &GpiButtonState,
) {
    check_button(emu, old, new, button::UP, KeypadKey::Up);
    check_button(emu, old, new, button::DOWN, KeypadKey::Down);
    check_button(emu, old, new, button::LEFT, KeypadKey::Left);
    check_button(emu, old, new, button::RIGHT, KeypadKey::Right);
    check_button(emu, old, new, button::A, KeypadKey::A);
    check_button(emu, old, new, button::B, KeypadKey::B);
    check_button(emu, old, new, button::START, KeypadKey::Start);
    check_button(emu, old, new, button::SELECT, KeypadKey::Select);
}

#[inline]
fn check_button(
    emu: &mut Emulator,
    old: &GpiButtonState,
    new: &GpiButtonState,
    btn: u16,
    key: KeypadKey,
) {
    let was_pressed = (old.current & btn) != 0;
    let is_pressed = (new.current & btn) != 0;

    if is_pressed && !was_pressed {
        emu.key_down(key);
    } else if !is_pressed && was_pressed {
        emu.key_up(key);
    }
}

// =============================================================================
// Kernel Entry Point
// =============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_main(_dtb_ptr: *const u8) -> ! {
    // Initialize allocator
    unsafe { allocator::init(); }
    gpio::configure_for_dpi();
    delay_ms(100);

    let mut fb = match Framebuffer::new() {
        Some(fb) => fb,
        None => halt_with_message("Failed to init framebuffer"),
    };

    // Show splash while initializing
    splash_screen::show(&fb);

    unsafe { init_exception_vectors(); }

    // =========================================================================
    // USB INITIALIZATION
    // =========================================================================

    let mut usb = UsbHost::new();

    match usb.init() {
        Ok(()) => {
            if usb.wait_for_connection(3000) {
                delay_ms(150);
                if let Ok(()) = usb.reset_port() {
                    let _ = usb.enumerate();
                }
            }
        }
        Err(_) => {}
    }

    // =========================================================================
    // SD CARD AND FILESYSTEM
    // =========================================================================

    let mut fs = Fat32::new();

    if let Err(_e) = fs.mount() {
        fb.clear(color::RED);
        fb.draw_str(100, 200, "SD Card mount failed!", color::WHITE, color::RED);
        fb.present();
        halt();
    }

    // =========================================================================
    // ROM SELECTION
    // =========================================================================

    let selection = {
        let mut fs_adapter = Fat32FileSystem::new(&mut fs);
        let mut input = RomSelectorInput::new(&mut usb);

        crate::subsystems::rom_selector::run_selector(&mut fs_adapter, &mut fb, &mut input)
    };
    // Adapters dropped here, memory freed by TLSF allocator

    let selection = match selection {
        Some(sel) => sel,
        None => {
            fb.clear(color::RED);
            fb.draw_str(100, 200, "No ROM selected!", color::WHITE, color::RED);
            fb.present();
            halt();
        }
    };

    // =========================================================================
    // LOAD ROM
    // =========================================================================

    fb.clear(color::BLACK);
    fb.draw_str(200, 230, "Loading ROM...", color::WHITE, color::BLACK);
    fb.present();

    if selection.size as usize > MAX_ROM_SIZE {
        fb.clear(color::RED);
        fb.draw_str(100, 200, "ROM too large!", color::WHITE, color::RED);
        fb.present();
        halt();
    }

    let mut rom_data = vec![0u8; selection.size as usize];

    if let Err(_e) = fs.read_file(selection.cluster, selection.size, &mut rom_data) {
        fb.clear(color::RED);
        fb.draw_str(100, 200, "Failed to read ROM!", color::WHITE, color::RED);
        fb.present();
        halt();
    }

    // =========================================================================
    // CREATE EMULATOR
    // =========================================================================

    let config = EmulatorConfig {
        force_classic: false,
        skip_checksum: true,
        enable_audio: false,
        sample_rate: 0,
    };

    let mut emulator = match Emulator::new(&rom_data, config) {
        Ok(emu) => emu,
        Err(_) => {
            fb.clear(color::RED);
            fb.draw_str(100, 200, "Failed to create emulator!", color::WHITE, color::RED);
            fb.present();
            halt();
        }
    };

    // =========================================================================
    // ENABLE CACHES
    // =========================================================================

    unsafe { enable_icache(); }
    unsafe { enable_dcache(); }

    // =========================================================================
    // MAIN EMULATOR LOOP
    // =========================================================================

    let mut button_state = GpiButtonState::new();
    let mut input_report = Xbox360InputReport::default();
    let mut speed_multiplier: u32 = 1;
    let mut fps_counter = FpsCounter::new();
    fps_counter.init();

    fb.clear(color::BLACK);

    loop {
        let frame_start = micros();

        // Poll USB input
        if usb.is_enumerated() {
            if let Ok(true) = usb.read_input(&mut input_report) {
                let old_state = button_state;
                button_state.update_from_xbox(&input_report);

                // Cycle speed with L or R
                if button_state.just_pressed(button::L) || button_state.just_pressed(button::R) {
                    speed_multiplier = match speed_multiplier {
                        1 => 2,
                        2 => 4,
                        _ => 1,
                    };
                }

                apply_input_changes(&mut emulator, &old_state, &button_state);
            }
        }

        // Run emulator
        emulator.step_frame_fast(speed_multiplier);

        // Render frame
        if emulator.frame_ready() {
            let gb_fb = emulator.framebuffer();
            fb.blit_gb_screen_gbc(gb_fb);

            unsafe {
                clean_dcache_range(fb.addr as usize, (fb.pitch * fb.height) as usize);
            }

            // FPS display
            if fps_counter.tick() {
                fb.fill_rect(4, 4, 120, 10, color::BLACK);
                let mut writer = StringWriter::new(&mut fb, 4, 4, color::GREEN, color::BLACK);
                match speed_multiplier {
                    1 => { let _ = write!(writer, "{} FPS", fps_counter.fps()); }
                    n => { let _ = write!(writer, "{} FPS [{}x]", fps_counter.fps(), n); }
                }
            }
        }

        // Frame timing
        let frame_elapsed = micros().wrapping_sub(frame_start);
        if frame_elapsed < FRAME_TIME_US {
            let wait_time = FRAME_TIME_US - frame_elapsed;
            let wait_start = micros();
            while micros().wrapping_sub(wait_start) < wait_time {
                core::hint::spin_loop();
            }
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

fn halt() -> ! {
    loop {
        unsafe { core::arch::asm!("wfe"); }
    }
}

fn halt_with_message(_msg: &str) -> ! {
    halt()
}

// =============================================================================
// Exception Handling
// =============================================================================

unsafe fn init_exception_vectors() {
    unsafe extern "C" {
        static exception_vectors: u8;
    }

    let vectors = unsafe { &exception_vectors as *const u8 as u64 };

    unsafe {
        core::arch::asm!(
        "msr vbar_el1, {0}",
        in(reg) vectors,
        options(nostack)
        );
        core::arch::asm!("isb", options(nostack));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn handle_sync_exception(_ctx: &ExceptionContext, _esr: u64, _far: u64) {
    halt();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn handle_irq(_ctx: &ExceptionContext) {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn handle_unhandled_exception(_ctx: &ExceptionContext, _esr: u64) {
    halt();
}

// =============================================================================
// Panic Handler
// =============================================================================

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(mut fb) = Framebuffer::new() {
        fb.clear(color::RED);
        let mut con = Console::new(&mut fb, color::WHITE, color::RED);
        con.println("!!! PANIC !!!");
        if let Some(loc) = info.location() {
            let _ = write!(con, "{}:{}\n", loc.file(), loc.line());
        }
        if let Some(msg) = info.message().as_str() {
            let _ = write!(con, "{}\n", msg);
        }
    }
    halt()
}
