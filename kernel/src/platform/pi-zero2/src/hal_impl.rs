//! Pi Zero 2W HAL Implementations
//!
//! Platform-specific implementations of the HAL traits for GPi Case 2W hardware.

use crate::hal::display::{Display, DisplayInfo, PixelFormat, GB_WIDTH, GB_HEIGHT};
use crate::hal::input::{InputDevice, ButtonState};
use crate::drivers::gpio;

// ============================================================================
// Display Implementation (DPI Framebuffer)
// ============================================================================

const DISPLAY_WIDTH: usize = 640;
const DISPLAY_HEIGHT: usize = 480;
const DISPLAY_BPP: usize = 4;  // 32-bit ARGB
const DISPLAY_PITCH: usize = DISPLAY_WIDTH * DISPLAY_BPP;
const FRAMEBUFFER_SIZE: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT * DISPLAY_BPP;

/// Pi Zero 2W Display with double buffering
pub struct PiDisplay {
    info: DisplayInfo,
    front_buffer: usize,  // Physical address from GPU
    back_buffer_data: [u8; FRAMEBUFFER_SIZE],
    current_buffer: usize,
}

impl PiDisplay {
    pub fn new(width: usize, height: usize) -> Self {
        // Allocate framebuffer via mailbox
        let fb_addr = allocate_framebuffer(width, height);

        Self {
            info: DisplayInfo {
                width,
                height,
                pitch: width * DISPLAY_BPP,
                format: PixelFormat::Argb8888,
            },
            front_buffer: fb_addr,
            back_buffer_data: [0; FRAMEBUFFER_SIZE],
            current_buffer: 0,
        }
    }

    /// Get pointer to front buffer (GPU framebuffer)
    fn front_ptr(&mut self) -> *mut u8 {
        self.front_buffer as *mut u8
    }
}

impl Display for PiDisplay {
    fn info(&self) -> DisplayInfo {
        self.info
    }

    fn framebuffer(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(self.front_ptr(), FRAMEBUFFER_SIZE)
        }
    }

    fn back_buffer(&mut self) -> &mut [u8] {
        &mut self.back_buffer_data
    }

    fn flip(&mut self) {
        // Copy back buffer to front buffer
        unsafe {
            let src = self.back_buffer_data.as_ptr();
            let dst = self.front_ptr();
            core::ptr::copy_nonoverlapping(src, dst, FRAMEBUFFER_SIZE);
        }
    }

    fn vsync(&self) {
        // Wait for vsync via mailbox or by polling vsync counter
        // For now, just a small delay
        crate::drivers::timer::delay_us(100);
    }

    fn clear(&mut self, color: u32) {
        let bytes = color.to_le_bytes();
        for chunk in self.back_buffer_data.chunks_exact_mut(4) {
            chunk.copy_from_slice(&bytes);
        }
    }

    fn draw_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }

        let offset = (y * self.info.pitch) + (x * DISPLAY_BPP);
        let bytes = color.to_le_bytes();
        self.back_buffer_data[offset..offset+4].copy_from_slice(&bytes);
    }

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let bytes = color.to_le_bytes();

        for row in y..(y + h).min(self.info.height) {
            let row_start = row * self.info.pitch + x * DISPLAY_BPP;
            let row_end = row_start + w.min(self.info.width - x) * DISPLAY_BPP;

            for chunk in self.back_buffer_data[row_start..row_end].chunks_exact_mut(4) {
                chunk.copy_from_slice(&bytes);
            }
        }
    }

    fn blit_gb_frame(&mut self, gb_pixels: &[u8], scale: usize) {
        // Calculate centered position
        let scaled_w = GB_WIDTH * scale;
        let scaled_h = GB_HEIGHT * scale;
        let start_x = (self.info.width - scaled_w) / 2;
        let start_y = (self.info.height - scaled_h) / 2;

        // GB pixels are in RGB888 format (3 bytes per pixel)
        // Or palette indexed (1 byte per pixel) depending on source

        // Assuming RGB888 input (160*144*3 = 69120 bytes)
        if gb_pixels.len() >= GB_WIDTH * GB_HEIGHT * 3 {
            for gy in 0..GB_HEIGHT {
                for gx in 0..GB_WIDTH {
                    let src_idx = (gy * GB_WIDTH + gx) * 3;
                    let r = gb_pixels[src_idx];
                    let g = gb_pixels[src_idx + 1];
                    let b = gb_pixels[src_idx + 2];
                    let color = crate::hal::display::colors::rgb_to_argb(r, g, b);

                    // Draw scaled pixel
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let dx = start_x + gx * scale + sx;
                            let dy = start_y + gy * scale + sy;
                            self.draw_pixel(dx, dy, color);
                        }
                    }
                }
            }
        }
    }

    fn set_palette(&mut self, _index: u8, _r: u8, _g: u8, _b: u8) {
        // Not used for 32-bit display
    }
}

/// Allocate framebuffer via VideoCore mailbox
fn allocate_framebuffer(width: usize, height: usize) -> usize {
    // Mailbox buffer (must be 16-byte aligned)
    #[repr(C, align(16))]
    struct MboxBuffer {
        data: [u32; 36],
    }

    let mut mbox = MboxBuffer { data: [0; 36] };

    // Build framebuffer allocation request
    mbox.data[0] = 35 * 4;  // Buffer size
    mbox.data[1] = 0;       // Request code

    // Set physical size
    mbox.data[2] = 0x48003;  // Tag: Set physical size
    mbox.data[3] = 8;        // Value buffer size
    mbox.data[4] = 8;        // Request size
    mbox.data[5] = width as u32;
    mbox.data[6] = height as u32;

    // Set virtual size
    mbox.data[7] = 0x48004;  // Tag: Set virtual size
    mbox.data[8] = 8;
    mbox.data[9] = 8;
    mbox.data[10] = width as u32;
    mbox.data[11] = height as u32;

    // Set depth
    mbox.data[12] = 0x48005;  // Tag: Set depth
    mbox.data[13] = 4;
    mbox.data[14] = 4;
    mbox.data[15] = 32;  // 32 bits per pixel

    // Set pixel order (RGB)
    mbox.data[16] = 0x48006;  // Tag: Set pixel order
    mbox.data[17] = 4;
    mbox.data[18] = 4;
    mbox.data[19] = 0;  // RGB

    // Allocate buffer
    mbox.data[20] = 0x40001;  // Tag: Allocate buffer
    mbox.data[21] = 8;
    mbox.data[22] = 8;
    mbox.data[23] = 4096;  // Alignment
    mbox.data[24] = 0;     // Will be filled with size

    // Get pitch
    mbox.data[25] = 0x40008;  // Tag: Get pitch
    mbox.data[26] = 4;
    mbox.data[27] = 4;
    mbox.data[28] = 0;  // Will be filled

    // End tag
    mbox.data[29] = 0;

    // Send to mailbox
    let mbox_addr = &mbox.data as *const _ as usize;
    mailbox_call(8, mbox_addr);  // Channel 8 = ARM to VC

    // Get framebuffer address (convert from bus address to ARM address)
    let fb_addr = (mbox.data[23] & 0x3FFFFFFF) as usize;

    fb_addr
}

/// Send message to VideoCore mailbox
fn mailbox_call(channel: u8, data: usize) {
    const MBOX_BASE: usize = 0x3F00_B880;
    const MBOX_READ: usize = MBOX_BASE + 0x00;
    const MBOX_STATUS: usize = MBOX_BASE + 0x18;
    const MBOX_WRITE: usize = MBOX_BASE + 0x20;
    const MBOX_FULL: u32 = 0x8000_0000;
    const MBOX_EMPTY: u32 = 0x4000_0000;

    // Wait for mailbox not full
    loop {
        let status: u32 = unsafe { core::ptr::read_volatile(MBOX_STATUS as *const u32) };
        if (status & MBOX_FULL) == 0 { break; }
    }

    // Write message
    let msg = ((data as u32) & !0xF) | (channel as u32);
    unsafe { core::ptr::write_volatile(MBOX_WRITE as *mut u32, msg); }

    // Wait for response
    loop {
        loop {
            let status: u32 = unsafe { core::ptr::read_volatile(MBOX_STATUS as *const u32) };
            if (status & MBOX_EMPTY) == 0 { break; }
        }

        let response: u32 = unsafe { core::ptr::read_volatile(MBOX_READ as *const u32) };
        if (response & 0xF) == channel as u32 {
            break;
        }
    }
}

// ============================================================================
// Input Implementation (GPIO Buttons)
// ============================================================================

/// GPi Case 2W GPIO button configuration
pub struct GpiPinConfig {
    pub up: u8,
    pub down: u8,
    pub left: u8,
    pub right: u8,
    pub a: u8,
    pub b: u8,
    pub x: u8,
    pub y: u8,
    pub l: u8,
    pub r: u8,
    pub start: u8,
    pub select: u8,
    pub home: u8,
    pub turbo: u8,
}

impl GpiPinConfig {
    /// Default configuration for GPi Case 2W
    /// Note: These may need adjustment based on your specific hardware revision
    pub const fn default_gpi_case_2w() -> Self {
        Self {
            up:     5,
            down:   6,
            left:   13,
            right:  19,
            a:      26,
            b:      12,
            x:      20,
            y:      16,
            l:      4,
            r:      17,
            start:  27,
            select: 23,
            home:   24,
            turbo:  25,
        }
    }
}

/// Pi Zero 2W input device using GPIO buttons
pub struct PiInput {
    config: GpiPinConfig,
    current: ButtonState,
    previous: u16,
    debounce: [u8; 14],
}

impl PiInput {
    pub fn new() -> Self {
        let config = GpiPinConfig::default_gpi_case_2w();

        // Configure GPIO pins as inputs with pull-ups
        Self::configure_pin(config.up);
        Self::configure_pin(config.down);
        Self::configure_pin(config.left);
        Self::configure_pin(config.right);
        Self::configure_pin(config.a);
        Self::configure_pin(config.b);
        Self::configure_pin(config.x);
        Self::configure_pin(config.y);
        Self::configure_pin(config.l);
        Self::configure_pin(config.r);
        Self::configure_pin(config.start);
        Self::configure_pin(config.select);
        Self::configure_pin(config.home);
        Self::configure_pin(config.turbo);

        Self {
            config,
            current: ButtonState::default(),
            previous: 0,
            debounce: [0; 14],
        }
    }

    fn configure_pin(pin: u8) {
        gpio::set_function(pin, gpio::GpioFunction::Input);
        gpio::set_pull(pin, gpio::GpioPull::Up);
    }

    fn read_buttons(&self) -> u16 {
        let mut state = 0u16;

        // Read each button (active low)
        if !gpio::read_pin(self.config.right)  { state |= ButtonState::RIGHT; }
        if !gpio::read_pin(self.config.left)   { state |= ButtonState::LEFT; }
        if !gpio::read_pin(self.config.up)     { state |= ButtonState::UP; }
        if !gpio::read_pin(self.config.down)   { state |= ButtonState::DOWN; }
        if !gpio::read_pin(self.config.a)      { state |= ButtonState::A; }
        if !gpio::read_pin(self.config.b)      { state |= ButtonState::B; }
        if !gpio::read_pin(self.config.select) { state |= ButtonState::SELECT; }
        if !gpio::read_pin(self.config.start)  { state |= ButtonState::START; }
        if !gpio::read_pin(self.config.x)      { state |= ButtonState::X; }
        if !gpio::read_pin(self.config.y)      { state |= ButtonState::Y; }
        if !gpio::read_pin(self.config.l)      { state |= ButtonState::L; }
        if !gpio::read_pin(self.config.r)      { state |= ButtonState::R; }
        if !gpio::read_pin(self.config.home)   { state |= ButtonState::HOME; }
        if !gpio::read_pin(self.config.turbo)  { state |= ButtonState::TURBO; }

        state
    }
}

impl InputDevice for PiInput {
    fn poll(&mut self) -> ButtonState {
        let raw = self.read_buttons();

        // Simple debouncing: require 2 consecutive reads
        let mut debounced = 0u16;
        for i in 0..14 {
            let bit = 1u16 << i;
            if (raw & bit) != 0 {
                if self.debounce[i] < 2 {
                    self.debounce[i] += 1;
                }
                if self.debounce[i] >= 2 {
                    debounced |= bit;
                }
            } else {
                self.debounce[i] = 0;
            }
        }

        // Calculate edges
        let just_pressed = debounced & !self.previous;
        let just_released = !debounced & self.previous;

        self.previous = debounced;
        self.current = ButtonState {
            pressed: debounced,
            just_pressed,
            just_released,
        };

        self.current
    }

    fn state(&self) -> ButtonState {
        self.current
    }

    fn menu_requested(&self) -> bool {
        self.current.was_just_pressed(ButtonState::HOME)
    }
}

// Re-export ButtonState for convenience
pub use crate::hal::input::ButtonState;
