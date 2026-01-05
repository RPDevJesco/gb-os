//! Game Boy Emulator Integration
//!
//! This module provides the interface between the bare-metal platform
//! and the Game Boy emulator core. In the full implementation, this
//! would import and use the gameboy/ modules (cpu.rs, gpu.rs, mmu.rs, etc.).
//!
//! For now, this is a stub that provides a placeholder implementation.

use crate::framebuffer::{Framebuffer, GB_WIDTH, GB_HEIGHT, colors};
use crate::input::{InputState, GbKey};
use crate::timer::FrameTimer;

// ============================================================================
// Constants
// ============================================================================

/// Cycles per Game Boy frame (70,224 T-cycles @ ~59.7 Hz)
pub const CYCLES_PER_FRAME: u32 = 70_224;

/// Game Boy CPU frequency (4.194304 MHz)
pub const CPU_FREQ_HZ: u32 = 4_194_304;

// ============================================================================
// Emulator Device
// ============================================================================

/// Game Boy emulator device.
///
/// In the full implementation, this would wrap the CPU struct from
/// `gameboy/cpu.rs`, which internally contains the MMU, GPU, etc.
pub struct Device {
    /// Frame buffer for GPU output (160x144 palette indices).
    frame_buffer: [u8; GB_WIDTH * GB_HEIGHT],
    /// Frame counter for placeholder animation.
    frame_count: u32,
    /// Current key state.
    keys: u8,
    /// ROM name.
    rom_name: [u8; 16],
    /// ROM loaded flag.
    rom_loaded: bool,
}

impl Device {
    /// Create a new emulator device with ROM data.
    ///
    /// # Arguments
    /// * `rom_data` - Game Boy ROM data
    ///
    /// # Returns
    /// `Ok(Device)` on success, `Err` with error message on failure.
    pub fn new(rom_data: &[u8]) -> Result<Self, &'static str> {
        if rom_data.len() < 0x150 {
            return Err("ROM too small");
        }

        // Extract ROM name from header (0x134-0x143)
        let mut rom_name = [0u8; 16];
        for (i, &b) in rom_data[0x134..0x144].iter().enumerate() {
            rom_name[i] = if b >= 0x20 && b < 0x7F { b } else { 0 };
        }

        // In full implementation:
        // let cart = mbc::get_mbc(rom_data.to_vec(), false)?;
        // let cpu = CPU::new_cgb(cart)?;

        Ok(Self {
            frame_buffer: [0; GB_WIDTH * GB_HEIGHT],
            frame_count: 0,
            keys: 0,
            rom_name,
            rom_loaded: true,
        })
    }

    /// Run one frame worth of CPU cycles.
    ///
    /// In the full implementation, this would execute cycles and update
    /// the GPU frame buffer.
    pub fn do_frame(&mut self) {
        // In full implementation:
        // let mut cycles = 0;
        // while cycles < CYCLES_PER_FRAME {
        //     cycles += self.cpu.do_cycle();
        // }

        // Placeholder: generate test pattern
        self.frame_count = self.frame_count.wrapping_add(1);
        self.generate_test_pattern();
    }

    /// Generate a test pattern for the placeholder implementation.
    fn generate_test_pattern(&mut self) {
        let offset = (self.frame_count / 10) as usize;

        for y in 0..GB_HEIGHT {
            for x in 0..GB_WIDTH {
                // Scrolling pattern to show it's running
                let pattern = ((x + y + offset) / 20) % 4;
                self.frame_buffer[y * GB_WIDTH + x] = pattern as u8;
            }
        }
    }

    /// Get the GPU frame buffer (palette indices).
    pub fn get_frame_buffer(&self) -> &[u8] {
        &self.frame_buffer
    }

    /// Handle a key press.
    pub fn keydown(&mut self, key: GbKey) {
        self.keys |= key.mask();

        // In full implementation:
        // self.cpu.mmu.keypad.keydown(key);
    }

    /// Handle a key release.
    pub fn keyup(&mut self, key: GbKey) {
        self.keys &= !key.mask();

        // In full implementation:
        // self.cpu.mmu.keypad.keyup(key);
    }

    /// Update key state from input state.
    pub fn update_keys(&mut self, input: &InputState) {
        let pressed = input.gb_keys_just_pressed();
        let released = input.gb_keys_just_released();

        // Process pressed keys
        if pressed & GbKey::Right.mask() != 0 { self.keydown(GbKey::Right); }
        if pressed & GbKey::Left.mask() != 0 { self.keydown(GbKey::Left); }
        if pressed & GbKey::Up.mask() != 0 { self.keydown(GbKey::Up); }
        if pressed & GbKey::Down.mask() != 0 { self.keydown(GbKey::Down); }
        if pressed & GbKey::A.mask() != 0 { self.keydown(GbKey::A); }
        if pressed & GbKey::B.mask() != 0 { self.keydown(GbKey::B); }
        if pressed & GbKey::Select.mask() != 0 { self.keydown(GbKey::Select); }
        if pressed & GbKey::Start.mask() != 0 { self.keydown(GbKey::Start); }

        // Process released keys
        if released & GbKey::Right.mask() != 0 { self.keyup(GbKey::Right); }
        if released & GbKey::Left.mask() != 0 { self.keyup(GbKey::Left); }
        if released & GbKey::Up.mask() != 0 { self.keyup(GbKey::Up); }
        if released & GbKey::Down.mask() != 0 { self.keyup(GbKey::Down); }
        if released & GbKey::A.mask() != 0 { self.keyup(GbKey::A); }
        if released & GbKey::B.mask() != 0 { self.keyup(GbKey::B); }
        if released & GbKey::Select.mask() != 0 { self.keyup(GbKey::Select); }
        if released & GbKey::Start.mask() != 0 { self.keyup(GbKey::Start); }
    }

    /// Get the ROM name.
    pub fn rom_name(&self) -> &str {
        let len = self.rom_name.iter().position(|&b| b == 0).unwrap_or(16);
        core::str::from_utf8(&self.rom_name[..len]).unwrap_or("Unknown")
    }

    /// Check if ROM is loaded.
    pub fn is_loaded(&self) -> bool {
        self.rom_loaded
    }

    /// Check if cartridge has battery-backed RAM (for saves).
    pub fn has_battery(&self) -> bool {
        // In full implementation:
        // self.cpu.mmu.mbc.is_battery_backed()
        false
    }

    /// Dump SRAM for saving.
    pub fn dump_sram(&self) -> &[u8] {
        // In full implementation:
        // self.cpu.mmu.mbc.dumpram()
        &[]
    }

    /// Load SRAM from save data.
    pub fn load_sram(&mut self, _data: &[u8]) -> Result<(), &'static str> {
        // In full implementation:
        // self.cpu.mmu.mbc.loadram(data)
        Ok(())
    }
}

// ============================================================================
// Emulator Runner
// ============================================================================

/// Run the emulator with the given ROM data.
///
/// This is the main emulator loop that handles:
/// - Frame timing
/// - Input processing
/// - GPU rendering
/// - Display output
pub fn run(fb: &mut Framebuffer, rom_data: &[u8]) -> Result<(), &'static str> {
    // Initialize emulator
    let mut device = Device::new(rom_data)?;

    // Draw initial screen
    fb.clear(colors::BLACK);
    fb.draw_gb_border(colors::GRAY);

    // Show ROM name
    let name = device.rom_name();
    fb.draw_string(10, 10, name, colors::WHITE, colors::BLACK);

    // Frame timer
    let mut timer = FrameTimer::gameboy();

    // Main loop
    loop {
        // Update input
        crate::input::update();
        let input = crate::input::get();

        // Check for Home button (exit)
        if input.just_pressed(crate::input::Button::Home) {
            break;
        }

        // Process input
        device.update_keys(input.state());

        // Run one frame
        device.do_frame();

        // Render to display
        fb.blit_gb_screen_dmg(device.get_frame_buffer());

        // Wait for frame timing
        timer.wait_for_frame();
    }

    Ok(())
}

// ============================================================================
// ROM Loading
// ============================================================================

/// Maximum ROM size (2MB)
pub const MAX_ROM_SIZE: usize = 2 * 1024 * 1024;

/// Static ROM buffer.
static mut ROM_BUFFER: [u8; MAX_ROM_SIZE] = [0; MAX_ROM_SIZE];

/// Get the ROM buffer for loading.
///
/// # Safety
/// Caller must ensure exclusive access during ROM loading.
pub unsafe fn get_rom_buffer() -> &'static mut [u8] {
    &mut ROM_BUFFER
}
