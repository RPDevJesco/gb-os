//! GB-OS for Pi Zero 2W / GPi Case 2W
//!
//! A bare-metal GameBoy emulator that boots directly on Raspberry Pi Zero 2W.
//! This integrates:
//! - SD card reading via SDHOST controller
//! - FAT32 filesystem for ROM loading
//! - ROM browser UI
//! - GameBoy Color emulator
//! - DPI display output (640x480 ARGB)
//! - GPIO button input

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use core::fmt::Write;

// ============================================================================
// Hardware Constants
// ============================================================================

const PERIPHERAL_BASE: usize = 0x3F00_0000;

// GPIO
const GPIO_BASE: usize = PERIPHERAL_BASE + 0x0020_0000;
const GPFSEL0: usize = GPIO_BASE + 0x00;
const GPFSEL1: usize = GPIO_BASE + 0x04;
const GPFSEL2: usize = GPIO_BASE + 0x08;
const GPFSEL3: usize = GPIO_BASE + 0x0C;
const GPFSEL4: usize = GPIO_BASE + 0x10;
const GPFSEL5: usize = GPIO_BASE + 0x14;
const GPSET0: usize = GPIO_BASE + 0x1C;
const GPCLR0: usize = GPIO_BASE + 0x28;
const GPLEV0: usize = GPIO_BASE + 0x34;
const GPPUD: usize = GPIO_BASE + 0x94;
const GPPUDCLK0: usize = GPIO_BASE + 0x98;
const GPPUDCLK1: usize = GPIO_BASE + 0x9C;

// Mailbox
const MBOX_BASE: usize = PERIPHERAL_BASE + 0x0000_B880;
const MBOX_READ: usize = MBOX_BASE + 0x00;
const MBOX_STATUS: usize = MBOX_BASE + 0x18;
const MBOX_WRITE: usize = MBOX_BASE + 0x20;
const MBOX_FULL: u32 = 0x8000_0000;
const MBOX_EMPTY: u32 = 0x4000_0000;

// System Timer (for frame timing)
const SYSTIMER_BASE: usize = PERIPHERAL_BASE + 0x0000_3000;
const SYSTIMER_CLO: usize = SYSTIMER_BASE + 0x04;  // Lower 32 bits

// SDHOST Controller
const SDHOST_BASE: usize = PERIPHERAL_BASE + 0x0020_2000;
const SDHOST_CMD: usize = SDHOST_BASE + 0x00;
const SDHOST_ARG: usize = SDHOST_BASE + 0x04;
const SDHOST_TOUT: usize = SDHOST_BASE + 0x08;
const SDHOST_CDIV: usize = SDHOST_BASE + 0x0C;
const SDHOST_RSP0: usize = SDHOST_BASE + 0x10;
const SDHOST_HSTS: usize = SDHOST_BASE + 0x20;
const SDHOST_VDD: usize = SDHOST_BASE + 0x30;
const SDHOST_EDM: usize = SDHOST_BASE + 0x34;
const SDHOST_HCFG: usize = SDHOST_BASE + 0x38;
const SDHOST_HBCT: usize = SDHOST_BASE + 0x3C;
const SDHOST_DATA: usize = SDHOST_BASE + 0x40;
const SDHOST_HBLC: usize = SDHOST_BASE + 0x50;

// SDHOST flags
const SDHOST_CMD_NEW: u32 = 0x8000;
const SDHOST_CMD_FAIL: u32 = 0x4000;
const SDHOST_CMD_BUSY: u32 = 0x0800;
const SDHOST_CMD_NO_RSP: u32 = 0x0400;
const SDHOST_CMD_LONG_RSP: u32 = 0x0200;
const SDHOST_CMD_READ: u32 = 0x0040;
const SDHOST_HSTS_DATA_FLAG: u32 = 0x0001;
const SDHOST_HCFG_SLOW_CARD: u32 = 0x0002;
const SDHOST_HCFG_INTBUS: u32 = 0x0001;

// Display
const SCREEN_WIDTH: u32 = 640;
const SCREEN_HEIGHT: u32 = 480;

// GameBoy
const GB_WIDTH: usize = 160;
const GB_HEIGHT: usize = 144;
const GB_SCALE: usize = 2;  // 2x scale fits nicely in 640x480
const GB_SCALED_W: usize = GB_WIDTH * GB_SCALE;   // 320
const GB_SCALED_H: usize = GB_HEIGHT * GB_SCALE;  // 288
const GB_OFFSET_X: usize = (SCREEN_WIDTH as usize - GB_SCALED_W) / 2;  // 160
const GB_OFFSET_Y: usize = (SCREEN_HEIGHT as usize - GB_SCALED_H) / 2; // 96

// Emulator timing
const CYCLES_PER_FRAME: u32 = 70224;  // 4.19 MHz / 59.7 fps
const FRAME_TIME_US: u32 = 16742;     // ~59.7 fps in microseconds

// ============================================================================
// Colors (ARGB8888)
// ============================================================================

const BLACK: u32 = 0xFF000000;
const WHITE: u32 = 0xFFFFFFFF;
const GREEN: u32 = 0xFF00FF00;
const CYAN: u32 = 0xFF00FFFF;
const YELLOW: u32 = 0xFFFFFF00;
const RED: u32 = 0xFFFF0000;
const DARK_BLUE: u32 = 0xFF000040;
const GRAY: u32 = 0xFF808080;
const DARK_GRAY: u32 = 0xFF404040;

// GameBoy DMG palette (green shades)
const GB_PALETTE: [u32; 4] = [
    0xFFE0F8D0,  // Lightest (white)
    0xFF88C070,  // Light green
    0xFF346856,  // Dark green
    0xFF081820,  // Darkest (black)
];

// ============================================================================
// Entry Point
// ============================================================================

core::arch::global_asm!(
    r#"
.section .text.boot
.global _start

_start:
    mrs     x0, mpidr_el1
    and     x0, x0, #0xFF
    cbnz    x0, .Lpark

    mov     x1, #0x0010
    lsl     x1, x1, #16
    mov     sp, x1

    ldr     x0, =__bss_start
    ldr     x1, =__bss_end
.Lclear_bss:
    cmp     x0, x1
    b.ge    .Ldone_bss
    str     xzr, [x0], #8
    b       .Lclear_bss
.Ldone_bss:

    bl      boot_main

.Lhalt:
    wfe
    b       .Lhalt

.Lpark:
    wfe
    b       .Lpark
"#
);

extern "C" {
    static __bss_start: u8;
    static __bss_end: u8;
}

// ============================================================================
// MMIO and Timing Helpers
// ============================================================================

#[inline(always)]
fn mmio_read(addr: usize) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}

#[inline(always)]
fn mmio_write(addr: usize, val: u32) {
    unsafe { write_volatile(addr as *mut u32, val) }
}



/// Get current microsecond count from system timer
fn micros() -> u32 {
    mmio_read(SYSTIMER_CLO)
}

/// Delay for specified microseconds using system timer (accurate)
fn delay_us(us: u32) {
    let start = micros();
    while micros().wrapping_sub(start) < us {
        core::hint::spin_loop();
    }
}

/// Delay for specified milliseconds
fn delay_ms(ms: u32) {
    delay_us(ms * 1000);
}

/// Short delay for GPIO settling (~150 cycles)
fn delay_short() {
    for _ in 0..150 {
        unsafe { core::arch::asm!("nop") };
    }
}

// ============================================================================
// GPIO Configuration
// ============================================================================

fn gpio_set_function(pin: u8, function: u8) {
    let reg = match pin / 10 {
        0 => GPFSEL0,
        1 => GPFSEL1,
        2 => GPFSEL2,
        3 => GPFSEL3,
        4 => GPFSEL4,
        5 => GPFSEL5,
        _ => return,
    };
    let shift = (pin % 10) * 3;
    let mask = 0b111 << shift;
    let val = (function as u32) << shift;
    let current = mmio_read(reg);
    mmio_write(reg, (current & !mask) | val);
}

fn gpio_set_pull(pin: u8, pull: u8) {
    mmio_write(GPPUD, pull as u32);
    delay_short();
    let bit = 1u32 << pin;
    mmio_write(GPPUDCLK0, bit);
    delay_short();
    mmio_write(GPPUD, 0);
    mmio_write(GPPUDCLK0, 0);
}

fn gpio_read(pin: u8) -> bool {
    (mmio_read(GPLEV0) & (1 << pin)) != 0
}

/// Configure GPIO 0-21 for DPI display output
fn configure_gpio_for_dpi() {
    const ALT2: u32 = 0b110;

    let gpfsel0_val: u32 =
        (ALT2 << 0)  | (ALT2 << 3)  | (ALT2 << 6)  | (ALT2 << 9)  |
            (ALT2 << 12) | (ALT2 << 15) | (ALT2 << 18) | (ALT2 << 21) |
            (ALT2 << 24) | (ALT2 << 27);

    let gpfsel1_val: u32 =
        (ALT2 << 0)  | (ALT2 << 3)  | (ALT2 << 6)  | (ALT2 << 9)  |
            (ALT2 << 12) | (ALT2 << 15) | (ALT2 << 18) | (ALT2 << 21) |
            (ALT2 << 24) | (ALT2 << 27);

    let gpfsel2_current = mmio_read(GPFSEL2);
    let gpfsel2_val: u32 = (ALT2 << 0) | (ALT2 << 3);

    mmio_write(GPFSEL0, gpfsel0_val);
    mmio_write(GPFSEL1, gpfsel1_val);
    mmio_write(GPFSEL2, (gpfsel2_current & 0xFFFFFFC0) | gpfsel2_val);
    mmio_write(GPPUD, 0);
    mmio_write(GPPUDCLK0, 0x003F_FFFF);
    mmio_write(GPPUD, 0);
    mmio_write(GPPUDCLK0, 0);
}

/// Configure GPIO 48-53 for SD card
fn configure_gpio_for_sd() {
    const ALT0: u32 = 0b100;

    let gpfsel4 = mmio_read(GPFSEL4);
    let gpfsel4_new = (gpfsel4 & 0xC0FFFFFF) | (ALT0 << 24) | (ALT0 << 27);
    mmio_write(GPFSEL4, gpfsel4_new);

    let gpfsel5 = mmio_read(GPFSEL5);
    let gpfsel5_new = (gpfsel5 & 0xFFFFF000) | (ALT0 << 0) | (ALT0 << 3) | (ALT0 << 6) | (ALT0 << 9);
    mmio_write(GPFSEL5, gpfsel5_new);
    mmio_write(GPPUD, 2);  // Pull-up
    mmio_write(GPPUDCLK1, (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 21));
    mmio_write(GPPUD, 0);
    mmio_write(GPPUDCLK1, 0);
}

// ============================================================================
// Mailbox Interface
// ============================================================================

#[repr(C, align(16))]
struct MailboxBuffer {
    data: [u32; 64],
}

impl MailboxBuffer {
    const fn new() -> Self {
        Self { data: [0; 64] }
    }
}

fn mailbox_call(buffer: &mut MailboxBuffer, channel: u8) -> bool {
    let addr = buffer.data.as_ptr() as u32;

    while (mmio_read(MBOX_STATUS) & MBOX_FULL) != 0 {
        core::hint::spin_loop();
    }

    mmio_write(MBOX_WRITE, (addr & !0xF) | (channel as u32 & 0xF));

    loop {
        while (mmio_read(MBOX_STATUS) & MBOX_EMPTY) != 0 {
            core::hint::spin_loop();
        }
        let response = mmio_read(MBOX_READ);
        if (response & 0xF) == channel as u32 {
            return buffer.data[1] == 0x8000_0000;
        }
    }
}

fn set_power_state(device_id: u32, on: bool) -> bool {
    let mut mbox = MailboxBuffer::new();
    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = 0x00028001;
    mbox.data[3] = 8;
    mbox.data[4] = 8;
    mbox.data[5] = device_id;
    mbox.data[6] = if on { 3 } else { 0 };
    mbox.data[7] = 0;
    mailbox_call(&mut mbox, 8) && (mbox.data[6] & 1) != 0
}

// ============================================================================
// Framebuffer
// ============================================================================

struct Framebuffer {
    addr: u32,
    width: u32,
    height: u32,
    pitch: u32,
}

impl Framebuffer {
    fn new() -> Option<Self> {
        let mut mbox = MailboxBuffer::new();

        mbox.data[0] = 35 * 4;
        mbox.data[1] = 0;
        mbox.data[2] = 0x0004_8003; mbox.data[3] = 8; mbox.data[4] = 8;
        mbox.data[5] = SCREEN_WIDTH; mbox.data[6] = SCREEN_HEIGHT;
        mbox.data[7] = 0x0004_8004; mbox.data[8] = 8; mbox.data[9] = 8;
        mbox.data[10] = SCREEN_WIDTH; mbox.data[11] = SCREEN_HEIGHT;
        mbox.data[12] = 0x0004_8005; mbox.data[13] = 4; mbox.data[14] = 4;
        mbox.data[15] = 32;
        mbox.data[16] = 0x0004_8006; mbox.data[17] = 4; mbox.data[18] = 4;
        mbox.data[19] = 0;
        mbox.data[20] = 0x0004_0001; mbox.data[21] = 8; mbox.data[22] = 8;
        mbox.data[23] = 16; mbox.data[24] = 0;
        mbox.data[25] = 0x0004_0008; mbox.data[26] = 4; mbox.data[27] = 4;
        mbox.data[28] = 0;
        mbox.data[29] = 0;

        if mailbox_call(&mut mbox, 8) && mbox.data[23] != 0 {
            Some(Self {
                addr: mbox.data[23] & 0x3FFF_FFFF,
                width: mbox.data[5],
                height: mbox.data[6],
                pitch: mbox.data[28],
            })
        } else {
            None
        }
    }

    fn put_pixel(&self, x: u32, y: u32, color: u32) {
        if x >= self.width || y >= self.height { return; }
        let offset = y * self.pitch + x * 4;
        unsafe { write_volatile((self.addr + offset) as *mut u32, color); }
    }

    fn fill_rect(&self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                self.put_pixel(x + dx, y + dy, color);
            }
        }
    }

    fn clear(&self, color: u32) {
        self.fill_rect(0, 0, self.width, self.height, color);
    }

    /// Blit GameBoy screen (160x144) to framebuffer with 2x scaling
    /// gb_pixels is palette-indexed (0-3 for DMG, or RGB for GBC)
    fn blit_gb_screen_dmg(&self, pal_data: &[u8]) {
        for y in 0..GB_HEIGHT {
            for x in 0..GB_WIDTH {
                let pal_idx = pal_data[y * GB_WIDTH + x] as usize;
                let color = if pal_idx < 4 { GB_PALETTE[pal_idx] } else { BLACK };

                // 2x scaling
                let sx = GB_OFFSET_X + x * GB_SCALE;
                let sy = GB_OFFSET_Y + y * GB_SCALE;

                for dy in 0..GB_SCALE {
                    for dx in 0..GB_SCALE {
                        self.put_pixel((sx + dx) as u32, (sy + dy) as u32, color);
                    }
                }
            }
        }
    }

    /// Blit GameBoy Color screen (RGB data)
    fn blit_gb_screen_gbc(&self, rgb_data: &[u8]) {
        for y in 0..GB_HEIGHT {
            for x in 0..GB_WIDTH {
                let idx = (y * GB_WIDTH + x) * 3;
                let r = rgb_data[idx] as u32;
                let g = rgb_data[idx + 1] as u32;
                let b = rgb_data[idx + 2] as u32;
                let color = 0xFF000000 | (r << 16) | (g << 8) | b;

                let sx = GB_OFFSET_X + x * GB_SCALE;
                let sy = GB_OFFSET_Y + y * GB_SCALE;

                for dy in 0..GB_SCALE {
                    for dx in 0..GB_SCALE {
                        self.put_pixel((sx + dx) as u32, (sy + dy) as u32, color);
                    }
                }
            }
        }
    }

    /// Draw border around GameBoy screen area
    fn draw_gb_border(&self, color: u32) {
        let border = 4;
        let x = GB_OFFSET_X as u32 - border;
        let y = GB_OFFSET_Y as u32 - border;
        let w = GB_SCALED_W as u32 + border * 2;
        let h = GB_SCALED_H as u32 + border * 2;

        // Top
        self.fill_rect(x, y, w, border, color);
        // Bottom
        self.fill_rect(x, y + h - border, w, border, color);
        // Left
        self.fill_rect(x, y, border, h, color);
        // Right
        self.fill_rect(x + w - border, y, border, h, color);
    }
}

// ============================================================================
// Simple Font and Text Rendering
// ============================================================================

static FONT_8X8: [[u8; 8]; 96] = [
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // Space
    [0x18,0x18,0x18,0x18,0x18,0x00,0x18,0x00], // !
    [0x6C,0x6C,0x24,0x00,0x00,0x00,0x00,0x00], // "
    [0x6C,0x6C,0xFE,0x6C,0xFE,0x6C,0x6C,0x00], // #
    [0x18,0x3E,0x60,0x3C,0x06,0x7C,0x18,0x00], // $
    [0x00,0x66,0xAC,0xD8,0x36,0x6A,0xCC,0x00], // %
    [0x38,0x6C,0x68,0x76,0xDC,0xCC,0x76,0x00], // &
    [0x18,0x18,0x30,0x00,0x00,0x00,0x00,0x00], // '
    [0x0C,0x18,0x30,0x30,0x30,0x18,0x0C,0x00], // (
    [0x30,0x18,0x0C,0x0C,0x0C,0x18,0x30,0x00], // )
    [0x00,0x66,0x3C,0xFF,0x3C,0x66,0x00,0x00], // *
    [0x00,0x18,0x18,0x7E,0x18,0x18,0x00,0x00], // +
    [0x00,0x00,0x00,0x00,0x00,0x18,0x18,0x30], // ,
    [0x00,0x00,0x00,0x7E,0x00,0x00,0x00,0x00], // -
    [0x00,0x00,0x00,0x00,0x00,0x18,0x18,0x00], // .
    [0x06,0x0C,0x18,0x30,0x60,0xC0,0x80,0x00], // /
    [0x3C,0x66,0x6E,0x7E,0x76,0x66,0x3C,0x00], // 0
    [0x18,0x38,0x18,0x18,0x18,0x18,0x7E,0x00], // 1
    [0x3C,0x66,0x06,0x1C,0x30,0x66,0x7E,0x00], // 2
    [0x3C,0x66,0x06,0x1C,0x06,0x66,0x3C,0x00], // 3
    [0x1C,0x3C,0x6C,0xCC,0xFE,0x0C,0x1E,0x00], // 4
    [0x7E,0x60,0x7C,0x06,0x06,0x66,0x3C,0x00], // 5
    [0x1C,0x30,0x60,0x7C,0x66,0x66,0x3C,0x00], // 6
    [0x7E,0x66,0x06,0x0C,0x18,0x18,0x18,0x00], // 7
    [0x3C,0x66,0x66,0x3C,0x66,0x66,0x3C,0x00], // 8
    [0x3C,0x66,0x66,0x3E,0x06,0x0C,0x38,0x00], // 9
    [0x00,0x18,0x18,0x00,0x18,0x18,0x00,0x00], // :
    [0x00,0x18,0x18,0x00,0x18,0x18,0x30,0x00], // ;
    [0x0C,0x18,0x30,0x60,0x30,0x18,0x0C,0x00], // <
    [0x00,0x00,0x7E,0x00,0x7E,0x00,0x00,0x00], // =
    [0x30,0x18,0x0C,0x06,0x0C,0x18,0x30,0x00], // >
    [0x3C,0x66,0x0C,0x18,0x18,0x00,0x18,0x00], // ?
    [0x3C,0x66,0x6E,0x6A,0x6E,0x60,0x3C,0x00], // @
    [0x3C,0x66,0x66,0x7E,0x66,0x66,0x66,0x00], // A
    [0x7C,0x66,0x66,0x7C,0x66,0x66,0x7C,0x00], // B
    [0x3C,0x66,0x60,0x60,0x60,0x66,0x3C,0x00], // C
    [0x78,0x6C,0x66,0x66,0x66,0x6C,0x78,0x00], // D
    [0x7E,0x60,0x60,0x7C,0x60,0x60,0x7E,0x00], // E
    [0x7E,0x60,0x60,0x7C,0x60,0x60,0x60,0x00], // F
    [0x3C,0x66,0x60,0x6E,0x66,0x66,0x3E,0x00], // G
    [0x66,0x66,0x66,0x7E,0x66,0x66,0x66,0x00], // H
    [0x7E,0x18,0x18,0x18,0x18,0x18,0x7E,0x00], // I
    [0x3E,0x0C,0x0C,0x0C,0x0C,0x6C,0x38,0x00], // J
    [0x66,0x6C,0x78,0x70,0x78,0x6C,0x66,0x00], // K
    [0x60,0x60,0x60,0x60,0x60,0x60,0x7E,0x00], // L
    [0xC6,0xEE,0xFE,0xD6,0xC6,0xC6,0xC6,0x00], // M
    [0x66,0x76,0x7E,0x7E,0x6E,0x66,0x66,0x00], // N
    [0x3C,0x66,0x66,0x66,0x66,0x66,0x3C,0x00], // O
    [0x7C,0x66,0x66,0x7C,0x60,0x60,0x60,0x00], // P
    [0x3C,0x66,0x66,0x66,0x6A,0x6C,0x36,0x00], // Q
    [0x7C,0x66,0x66,0x7C,0x6C,0x66,0x66,0x00], // R
    [0x3C,0x66,0x60,0x3C,0x06,0x66,0x3C,0x00], // S
    [0x7E,0x18,0x18,0x18,0x18,0x18,0x18,0x00], // T
    [0x66,0x66,0x66,0x66,0x66,0x66,0x3C,0x00], // U
    [0x66,0x66,0x66,0x66,0x66,0x3C,0x18,0x00], // V
    [0xC6,0xC6,0xC6,0xD6,0xFE,0xEE,0xC6,0x00], // W
    [0x66,0x66,0x3C,0x18,0x3C,0x66,0x66,0x00], // X
    [0x66,0x66,0x66,0x3C,0x18,0x18,0x18,0x00], // Y
    [0x7E,0x06,0x0C,0x18,0x30,0x60,0x7E,0x00], // Z
    [0x3C,0x30,0x30,0x30,0x30,0x30,0x3C,0x00], // [
    [0xC0,0x60,0x30,0x18,0x0C,0x06,0x02,0x00], // backslash
    [0x3C,0x0C,0x0C,0x0C,0x0C,0x0C,0x3C,0x00], // ]
    [0x18,0x3C,0x66,0x00,0x00,0x00,0x00,0x00], // ^
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0xFF], // _
    [0x30,0x18,0x0C,0x00,0x00,0x00,0x00,0x00], // `
    [0x00,0x00,0x3C,0x06,0x3E,0x66,0x3E,0x00], // a
    [0x60,0x60,0x7C,0x66,0x66,0x66,0x7C,0x00], // b
    [0x00,0x00,0x3C,0x66,0x60,0x66,0x3C,0x00], // c
    [0x06,0x06,0x3E,0x66,0x66,0x66,0x3E,0x00], // d
    [0x00,0x00,0x3C,0x66,0x7E,0x60,0x3C,0x00], // e
    [0x1C,0x30,0x30,0x7C,0x30,0x30,0x30,0x00], // f
    [0x00,0x00,0x3E,0x66,0x66,0x3E,0x06,0x3C], // g
    [0x60,0x60,0x7C,0x66,0x66,0x66,0x66,0x00], // h
    [0x18,0x00,0x38,0x18,0x18,0x18,0x3C,0x00], // i
    [0x18,0x00,0x38,0x18,0x18,0x18,0x18,0x70], // j
    [0x60,0x60,0x66,0x6C,0x78,0x6C,0x66,0x00], // k
    [0x38,0x18,0x18,0x18,0x18,0x18,0x3C,0x00], // l
    [0x00,0x00,0x6C,0xFE,0xD6,0xC6,0xC6,0x00], // m
    [0x00,0x00,0x7C,0x66,0x66,0x66,0x66,0x00], // n
    [0x00,0x00,0x3C,0x66,0x66,0x66,0x3C,0x00], // o
    [0x00,0x00,0x7C,0x66,0x66,0x7C,0x60,0x60], // p
    [0x00,0x00,0x3E,0x66,0x66,0x3E,0x06,0x06], // q
    [0x00,0x00,0x7C,0x66,0x60,0x60,0x60,0x00], // r
    [0x00,0x00,0x3E,0x60,0x3C,0x06,0x7C,0x00], // s
    [0x30,0x30,0x7C,0x30,0x30,0x30,0x1C,0x00], // t
    [0x00,0x00,0x66,0x66,0x66,0x66,0x3E,0x00], // u
    [0x00,0x00,0x66,0x66,0x66,0x3C,0x18,0x00], // v
    [0x00,0x00,0xC6,0xC6,0xD6,0xFE,0x6C,0x00], // w
    [0x00,0x00,0x66,0x3C,0x18,0x3C,0x66,0x00], // x
    [0x00,0x00,0x66,0x66,0x66,0x3E,0x06,0x3C], // y
    [0x00,0x00,0x7E,0x0C,0x18,0x30,0x7E,0x00], // z
    [0x0C,0x18,0x18,0x30,0x18,0x18,0x0C,0x00], // {
    [0x18,0x18,0x18,0x18,0x18,0x18,0x18,0x00], // |
    [0x30,0x18,0x18,0x0C,0x18,0x18,0x30,0x00], // }
    [0x00,0x00,0x76,0xDC,0x00,0x00,0x00,0x00], // ~
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // DEL
];

fn draw_char(fb: &Framebuffer, x: u32, y: u32, c: char, fg: u32, bg: u32) {
    let code = c as u8;
    let idx = if code >= 32 && code < 128 { (code - 32) as usize } else { 0 };
    let glyph = &FONT_8X8[idx];

    for row in 0..8u32 {
        let bits = glyph[row as usize];
        for col in 0..8u32 {
            let bit = (bits >> (7 - col)) & 1;
            let color = if bit != 0 { fg } else { bg };
            fb.put_pixel(x + col, y + row, color);
        }
    }
}

fn draw_string(fb: &Framebuffer, x: u32, y: u32, s: &str, fg: u32, bg: u32) {
    let mut cx = x;
    for c in s.chars() {
        draw_char(fb, cx, y, c, fg, bg);
        cx += 8;
    }
}

// ============================================================================
// Console for Debug Output
// ============================================================================

struct Console<'a> {
    fb: &'a Framebuffer,
    x: u32,
    y: u32,
    fg: u32,
    bg: u32,
}

impl<'a> Console<'a> {
    fn new(fb: &'a Framebuffer, fg: u32, bg: u32) -> Self {
        Self { fb, x: 8, y: 8, fg, bg }
    }

    fn newline(&mut self) {
        self.x = 8;
        self.y += 10;
        if self.y + 10 > self.fb.height { self.y = 8; }
    }

    fn print(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\n' { self.newline(); continue; }
            draw_char(self.fb, self.x, self.y, c, self.fg, self.bg);
            self.x += 8;
            if self.x + 8 > self.fb.width - 8 { self.newline(); }
        }
    }

    fn println(&mut self, s: &str) {
        self.print(s);
        self.newline();
    }

    fn set_color(&mut self, fg: u32, bg: u32) {
        self.fg = fg;
        self.bg = bg;
    }
}

impl<'a> Write for Console<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.print(s);
        Ok(())
    }
}

// ============================================================================
// SD Card via SDHOST
// ============================================================================

struct SdCard {
    initialized: bool,
    is_sdhc: bool,
    rca: u32,
}

impl SdCard {
    const fn new() -> Self {
        Self { initialized: false, is_sdhc: true, rca: 0 }
    }

    fn clear_status(&self) {
        mmio_write(SDHOST_HSTS, 0x7F8);
    }

    fn reset(&self) {
        mmio_write(SDHOST_CMD, 0);
        mmio_write(SDHOST_ARG, 0);
        mmio_write(SDHOST_TOUT, 0xF00000);
        mmio_write(SDHOST_CDIV, 0);
        mmio_write(SDHOST_HSTS, 0x7F8);
        mmio_write(SDHOST_HCFG, 0);
        mmio_write(SDHOST_HBCT, 0);
        mmio_write(SDHOST_HBLC, 0);
        mmio_write(SDHOST_VDD, 1);
        delay_ms(10);  // Power stabilization
        mmio_write(SDHOST_HCFG, SDHOST_HCFG_SLOW_CARD | SDHOST_HCFG_INTBUS);
        mmio_write(SDHOST_CDIV, 0x148);
        delay_ms(10);  // Clock stabilization
    }

    fn wait_cmd(&self) -> Result<(), &'static str> {
        for _ in 0..50000 {
            let cmd = mmio_read(SDHOST_CMD);
            if (cmd & SDHOST_CMD_NEW) == 0 {
                let hsts = mmio_read(SDHOST_HSTS);
                if (hsts & 0x40) != 0 { self.clear_status(); return Err("Timeout"); }
                if (hsts & 0x10) != 0 { self.clear_status(); return Err("CRC"); }
                return Ok(());
            }
            if (cmd & SDHOST_CMD_FAIL) != 0 {
                self.clear_status();
                return Err("Fail");
            }
        }
        Err("Wait timeout")
    }

    fn send_cmd(&mut self, cmd_idx: u32, arg: u32, flags: u32) -> Result<u32, &'static str> {
        self.clear_status();
        mmio_write(SDHOST_ARG, arg);
        mmio_write(SDHOST_CMD, (cmd_idx & 0x3F) | flags | SDHOST_CMD_NEW);
        self.wait_cmd()?;
        Ok(mmio_read(SDHOST_RSP0))
    }

    fn init(&mut self) -> Result<(), &'static str> {
        configure_gpio_for_sd();
        set_power_state(0, true);
        self.reset();

        // CMD0
        mmio_write(SDHOST_ARG, 0);
        mmio_write(SDHOST_CMD, 0 | SDHOST_CMD_NO_RSP | SDHOST_CMD_NEW);
        delay_ms(50);  // Card needs time to reset
        self.clear_status();

        // CMD8
        match self.send_cmd(8, 0x1AA, 0) {
            Ok(resp) => { self.is_sdhc = (resp & 0xFF) == 0xAA; }
            Err(_) => { self.is_sdhc = false; self.clear_status(); }
        }

        // ACMD41 loop
        for _ in 0..50 {
            let _ = self.send_cmd(55, 0, 0);
            let hcs = if self.is_sdhc { 0x40000000 } else { 0 };
            if let Ok(ocr) = self.send_cmd(41, 0x00FF8000 | hcs, 0) {
                if (ocr & 0x80000000) != 0 {
                    self.is_sdhc = (ocr & 0x40000000) != 0;
                    break;
                }
            }
        }

        // CMD2, CMD3, CMD7
        self.send_cmd(2, 0, SDHOST_CMD_LONG_RSP)?;
        let resp = self.send_cmd(3, 0, 0)?;
        self.rca = resp & 0xFFFF0000;
        self.send_cmd(7, self.rca, SDHOST_CMD_BUSY)?;

        mmio_write(SDHOST_CDIV, 4);
        mmio_write(SDHOST_HBCT, 512);

        self.initialized = true;
        Ok(())
    }

    fn read_sector(&mut self, lba: u32, buffer: &mut [u8; 512]) -> Result<(), &'static str> {
        if !self.initialized { return Err("Not init"); }

        mmio_write(SDHOST_HBCT, 512);
        mmio_write(SDHOST_HBLC, 1);

        let addr = if self.is_sdhc { lba } else { lba * 512 };
        self.clear_status();
        mmio_write(SDHOST_ARG, addr);
        mmio_write(SDHOST_CMD, 17 | SDHOST_CMD_READ | SDHOST_CMD_NEW);
        self.wait_cmd()?;

        let mut idx = 0;
        for _ in 0..500000 {
            if idx >= 512 { break; }
            let hsts = mmio_read(SDHOST_HSTS);
            if (hsts & SDHOST_HSTS_DATA_FLAG) != 0 {
                let word = mmio_read(SDHOST_DATA);
                buffer[idx] = (word >> 0) as u8;
                buffer[idx + 1] = (word >> 8) as u8;
                buffer[idx + 2] = (word >> 16) as u8;
                buffer[idx + 3] = (word >> 24) as u8;
                idx += 4;
            }
        }

        self.clear_status();
        if idx < 512 { return Err("Data timeout"); }
        Ok(())
    }
}

// ============================================================================
// FAT32 Filesystem
// ============================================================================

const SECTOR_SIZE: usize = 512;

struct Fat32 {
    sd: SdCard,
    mounted: bool,
    fat_start_sector: u32,
    data_start_sector: u32,
    root_cluster: u32,
    sectors_per_cluster: u8,
    bytes_per_sector: u32,
}

impl Fat32 {
    const fn new() -> Self {
        Self {
            sd: SdCard::new(),
            mounted: false,
            fat_start_sector: 0,
            data_start_sector: 0,
            root_cluster: 0,
            sectors_per_cluster: 0,
            bytes_per_sector: 512,
        }
    }

    fn mount(&mut self) -> Result<(), &'static str> {
        self.sd.init()?;

        // Read MBR
        let mut sector = [0u8; 512];
        self.sd.read_sector(0, &mut sector)?;

        if sector[510] != 0x55 || sector[511] != 0xAA {
            return Err("Invalid MBR");
        }

        // Get partition start from MBR
        let part_start = u32::from_le_bytes([
            sector[0x1BE + 8], sector[0x1BE + 9],
            sector[0x1BE + 10], sector[0x1BE + 11],
        ]);

        // Read FAT32 boot sector
        self.sd.read_sector(part_start, &mut sector)?;

        if sector[510] != 0x55 || sector[511] != 0xAA {
            return Err("Invalid VBR");
        }

        self.bytes_per_sector = u16::from_le_bytes([sector[11], sector[12]]) as u32;
        self.sectors_per_cluster = sector[13];
        let reserved_sectors = u16::from_le_bytes([sector[14], sector[15]]) as u32;
        let num_fats = sector[16] as u32;
        let fat_size = u32::from_le_bytes([sector[36], sector[37], sector[38], sector[39]]);
        self.root_cluster = u32::from_le_bytes([sector[44], sector[45], sector[46], sector[47]]);

        self.fat_start_sector = part_start + reserved_sectors;
        self.data_start_sector = self.fat_start_sector + (num_fats * fat_size);
        self.mounted = true;

        Ok(())
    }

    fn cluster_to_sector(&self, cluster: u32) -> u64 {
        let cluster_offset = (cluster - 2) as u64;
        self.data_start_sector as u64 + (cluster_offset * self.sectors_per_cluster as u64)
    }

    fn get_next_cluster(&mut self, cluster: u32) -> Result<u32, &'static str> {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start_sector + (fat_offset / self.bytes_per_sector);
        let entry_offset = (fat_offset % self.bytes_per_sector) as usize;

        let mut sector = [0u8; 512];
        self.sd.read_sector(fat_sector, &mut sector)?;

        let next = u32::from_le_bytes([
            sector[entry_offset], sector[entry_offset + 1],
            sector[entry_offset + 2], sector[entry_offset + 3],
        ]) & 0x0FFFFFFF;

        Ok(next)
    }

    /// Count .gb/.gbc ROM files in root directory
    fn count_roms(&mut self) -> usize {
        if !self.mounted { return 0; }

        let mut sector = [0u8; 512];
        let mut count = 0;
        let mut current_cluster = self.root_cluster;

        while current_cluster >= 2 && current_cluster < 0x0FFFFFF8 {
            let cluster_lba = self.cluster_to_sector(current_cluster) as u32;

            for sector_offset in 0..self.sectors_per_cluster {
                if self.sd.read_sector(cluster_lba + sector_offset as u32, &mut sector).is_err() {
                    return count;
                }

                for i in 0..16 {
                    let offset = i * 32;
                    let first_byte = sector[offset];

                    if first_byte == 0x00 { return count; }
                    if first_byte == 0xE5 { continue; }

                    let attr = sector[offset + 11];
                    if attr == 0x0F || attr == 0x08 || (attr & 0x10) != 0 { continue; }

                    let ext0 = sector[offset + 8].to_ascii_uppercase();
                    let ext1 = sector[offset + 9].to_ascii_uppercase();
                    let ext2 = sector[offset + 10].to_ascii_uppercase();

                    if ext0 == b'G' && ext1 == b'B' && (ext2 == b' ' || ext2 == b'C') {
                        count += 1;
                    }
                }
            }

            current_cluster = match self.get_next_cluster(current_cluster) {
                Ok(next) => next,
                Err(_) => break,
            };
        }

        count
    }

    /// Find ROM at index, returns (cluster, size)
    fn find_rom(&mut self, index: usize) -> Option<(u32, u32)> {
        if !self.mounted { return None; }

        let mut sector = [0u8; 512];
        let mut rom_index = 0;
        let mut current_cluster = self.root_cluster;

        while current_cluster >= 2 && current_cluster < 0x0FFFFFF8 {
            let cluster_lba = self.cluster_to_sector(current_cluster) as u32;

            for sector_offset in 0..self.sectors_per_cluster {
                if self.sd.read_sector(cluster_lba + sector_offset as u32, &mut sector).is_err() {
                    return None;
                }

                for i in 0..16 {
                    let offset = i * 32;
                    let first_byte = sector[offset];

                    if first_byte == 0x00 { return None; }
                    if first_byte == 0xE5 { continue; }

                    let attr = sector[offset + 11];
                    if attr == 0x0F || attr == 0x08 || (attr & 0x10) != 0 { continue; }

                    let ext0 = sector[offset + 8].to_ascii_uppercase();
                    let ext1 = sector[offset + 9].to_ascii_uppercase();
                    let ext2 = sector[offset + 10].to_ascii_uppercase();

                    if ext0 == b'G' && ext1 == b'B' && (ext2 == b' ' || ext2 == b'C') {
                        if rom_index == index {
                            let cluster_lo = u16::from_le_bytes([sector[offset + 26], sector[offset + 27]]);
                            let cluster_hi = u16::from_le_bytes([sector[offset + 20], sector[offset + 21]]);
                            let cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);
                            let size = u32::from_le_bytes([
                                sector[offset + 28], sector[offset + 29],
                                sector[offset + 30], sector[offset + 31],
                            ]);
                            return Some((cluster, size));
                        }
                        rom_index += 1;
                    }
                }
            }

            current_cluster = match self.get_next_cluster(current_cluster) {
                Ok(next) => next,
                Err(_) => break,
            };
        }

        None
    }

    /// Get ROM filename at index (8.3 format)
    fn get_rom_name(&mut self, index: usize, name_buf: &mut [u8; 12]) -> bool {
        if !self.mounted { return false; }

        let mut sector = [0u8; 512];
        let mut rom_index = 0;
        let mut current_cluster = self.root_cluster;

        while current_cluster >= 2 && current_cluster < 0x0FFFFFF8 {
            let cluster_lba = self.cluster_to_sector(current_cluster) as u32;

            for sector_offset in 0..self.sectors_per_cluster {
                if self.sd.read_sector(cluster_lba + sector_offset as u32, &mut sector).is_err() {
                    return false;
                }

                for i in 0..16 {
                    let offset = i * 32;
                    let first_byte = sector[offset];

                    if first_byte == 0x00 { return false; }
                    if first_byte == 0xE5 { continue; }

                    let attr = sector[offset + 11];
                    if attr == 0x0F || attr == 0x08 || (attr & 0x10) != 0 { continue; }

                    let ext0 = sector[offset + 8].to_ascii_uppercase();
                    let ext1 = sector[offset + 9].to_ascii_uppercase();
                    let ext2 = sector[offset + 10].to_ascii_uppercase();

                    if ext0 == b'G' && ext1 == b'B' && (ext2 == b' ' || ext2 == b'C') {
                        if rom_index == index {
                            // Copy filename
                            for j in 0..8 {
                                name_buf[j] = sector[offset + j];
                            }
                            name_buf[8] = b'.';
                            for j in 0..3 {
                                name_buf[9 + j] = sector[offset + 8 + j];
                            }
                            return true;
                        }
                        rom_index += 1;
                    }
                }
            }

            current_cluster = match self.get_next_cluster(current_cluster) {
                Ok(next) => next,
                Err(_) => break,
            };
        }

        false
    }

    /// Read file data into buffer
    fn read_file(&mut self, cluster: u32, size: u32, buffer: &mut [u8]) -> Result<usize, &'static str> {
        if !self.mounted { return Err("Not mounted"); }
        if cluster < 2 { return Err("Invalid cluster"); }

        let to_read = (size as usize).min(buffer.len());
        let mut bytes_read = 0;
        let mut current_cluster = cluster;
        let mut sector_buf = [0u8; 512];

        while bytes_read < to_read && current_cluster >= 2 && current_cluster < 0x0FFFFFF8 {
            let cluster_lba = self.cluster_to_sector(current_cluster) as u32;

            for s in 0..self.sectors_per_cluster {
                if bytes_read >= to_read { break; }

                self.sd.read_sector(cluster_lba + s as u32, &mut sector_buf)?;

                let copy_len = (to_read - bytes_read).min(SECTOR_SIZE);
                buffer[bytes_read..bytes_read + copy_len].copy_from_slice(&sector_buf[..copy_len]);
                bytes_read += copy_len;
            }

            current_cluster = self.get_next_cluster(current_cluster)?;
        }

        Ok(bytes_read)
    }
}

// ============================================================================
// ROM Browser UI
// ============================================================================

struct RomBrowser {
    rom_count: usize,
    selected: usize,
}

impl RomBrowser {
    fn new(rom_count: usize) -> Self {
        Self { rom_count, selected: 0 }
    }

    fn draw(&self, fb: &Framebuffer, fs: &mut Fat32) {
        fb.clear(DARK_BLUE);

        // Title
        draw_string(fb, 200, 20, "GB-OS ROM Browser", CYAN, DARK_BLUE);
        draw_string(fb, 180, 40, "Select ROM with D-Pad", WHITE, DARK_BLUE);

        if self.rom_count == 0 {
            draw_string(fb, 200, 200, "No ROMs found!", RED, DARK_BLUE);
            draw_string(fb, 120, 230, "Place .gb or .gbc files on SD", WHITE, DARK_BLUE);
            return;
        }

        // Draw ROM list
        let list_y = 80;
        let item_height = 20;
        let visible_items = 15.min(self.rom_count);

        for i in 0..visible_items {
            let y = list_y + (i as u32) * item_height;
            let mut name_buf = [0u8; 12];

            if fs.get_rom_name(i, &mut name_buf) {
                let name_str = core::str::from_utf8(&name_buf).unwrap_or("????????.???");

                let (fg, bg) = if i == self.selected {
                    (BLACK, CYAN)
                } else {
                    (WHITE, DARK_BLUE)
                };

                // Highlight bar for selected
                if i == self.selected {
                    fb.fill_rect(100, y, 440, item_height as u32 - 2, bg);
                }

                draw_string(fb, 110, y + 4, name_str, fg, bg);
            }
        }

        // Instructions
        let _ = core::write!(
            &mut StringWriter::new(fb, 150, 420, WHITE, DARK_BLUE),
            "ROMs found: {}  |  Press A to start", self.rom_count
        );
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.selected < self.rom_count.saturating_sub(1) {
            self.selected += 1;
        }
    }

    fn get_selection(&self) -> usize {
        self.selected
    }
}

/// Simple string writer for formatted output
struct StringWriter<'a> {
    fb: &'a Framebuffer,
    x: u32,
    y: u32,
    fg: u32,
    bg: u32,
}

impl<'a> StringWriter<'a> {
    fn new(fb: &'a Framebuffer, x: u32, y: u32, fg: u32, bg: u32) -> Self {
        Self { fb, x, y, fg, bg }
    }
}

impl<'a> Write for StringWriter<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            draw_char(self.fb, self.x, self.y, c, self.fg, self.bg);
            self.x += 8;
        }
        Ok(())
    }
}

// ============================================================================
// Input (GPIO Buttons for GPi Case 2W)
// ============================================================================

// GPi Case 2W button GPIO pins (directly active-low)
const GPIO_UP: u8 = 5;
const GPIO_DOWN: u8 = 6;
const GPIO_LEFT: u8 = 13;
const GPIO_RIGHT: u8 = 19;
const GPIO_A: u8 = 26;
const GPIO_B: u8 = 21;
const GPIO_X: u8 = 4;
const GPIO_Y: u8 = 12;
const GPIO_START: u8 = 22;
const GPIO_SELECT: u8 = 17;
const GPIO_L: u8 = 16;
const GPIO_R: u8 = 20;
const GPIO_HOME: u8 = 27;

// Button bit flags (for u16 state)
const BTN_UP: u16     = 1 << 0;
const BTN_DOWN: u16   = 1 << 1;
const BTN_LEFT: u16   = 1 << 2;
const BTN_RIGHT: u16  = 1 << 3;
const BTN_A: u16      = 1 << 4;
const BTN_B: u16      = 1 << 5;
const BTN_X: u16      = 1 << 6;
const BTN_Y: u16      = 1 << 7;
const BTN_START: u16  = 1 << 8;
const BTN_SELECT: u16 = 1 << 9;
const BTN_L: u16      = 1 << 10;
const BTN_R: u16      = 1 << 11;
const BTN_HOME: u16   = 1 << 12;

fn configure_gpio_for_input() {
    // Set all button GPIOs as inputs with pull-ups
    let buttons = [
        GPIO_UP, GPIO_DOWN, GPIO_LEFT, GPIO_RIGHT,
        GPIO_A, GPIO_B, GPIO_X, GPIO_Y,
        GPIO_START, GPIO_SELECT,
        GPIO_L, GPIO_R, GPIO_HOME,
    ];

    for &pin in &buttons {
        gpio_set_function(pin, 0);  // Input
        gpio_set_pull(pin, 2);      // Pull-up
    }
}

fn read_buttons() -> u16 {
    let mut state = 0u16;

    // D-Pad
    if !gpio_read(GPIO_UP)     { state |= BTN_UP; }
    if !gpio_read(GPIO_DOWN)   { state |= BTN_DOWN; }
    if !gpio_read(GPIO_LEFT)   { state |= BTN_LEFT; }
    if !gpio_read(GPIO_RIGHT)  { state |= BTN_RIGHT; }

    // Face buttons
    if !gpio_read(GPIO_A)      { state |= BTN_A; }
    if !gpio_read(GPIO_B)      { state |= BTN_B; }
    if !gpio_read(GPIO_X)      { state |= BTN_X; }
    if !gpio_read(GPIO_Y)      { state |= BTN_Y; }

    // Menu buttons
    if !gpio_read(GPIO_START)  { state |= BTN_START; }
    if !gpio_read(GPIO_SELECT) { state |= BTN_SELECT; }

    // Shoulder buttons
    if !gpio_read(GPIO_L)      { state |= BTN_L; }
    if !gpio_read(GPIO_R)      { state |= BTN_R; }

    // Home button
    if !gpio_read(GPIO_HOME)   { state |= BTN_HOME; }

    state
}

// ============================================================================
// GameBoy Emulator Stub
// This is where you would integrate the full GameBoy emulator modules:
// - cpu.rs, gpu.rs, mmu.rs, mbc/*.rs, etc.
// For now, this is a placeholder showing the integration structure.
// ============================================================================

/// Placeholder for the full GameBoy Device from gameboy/device.rs
/// In the actual implementation, you would:
/// 1. Copy the gameboy/ directory from the kernel
/// 2. Adapt it to work with this framebuffer
struct GameBoyDevice {
    // The actual device would contain:
    // cpu: CPU,
    // Which internally has: mmu: MMU, gpu: GPU, etc.

    // For now, we'll simulate with a simple placeholder
    frame_buffer: [u8; GB_WIDTH * GB_HEIGHT],
    frame_count: u32,
}

impl GameBoyDevice {
    fn new(_rom_data: &[u8]) -> Result<Self, &'static str> {
        // In the real implementation:
        // let cart = mbc::get_mbc(rom_data.to_vec(), false)?;
        // CPU::new(cart).map(|cpu| Device { cpu })

        Ok(Self {
            frame_buffer: [0; GB_WIDTH * GB_HEIGHT],
            frame_count: 0,
        })
    }

    /// Run one frame worth of CPU cycles
    fn do_frame(&mut self) {
        // In the real implementation:
        // let mut cycles = 0;
        // while cycles < CYCLES_PER_FRAME {
        //     cycles += self.cpu.do_cycle();
        // }

        // Placeholder: generate a simple test pattern
        self.frame_count = self.frame_count.wrapping_add(1);

        for y in 0..GB_HEIGHT {
            for x in 0..GB_WIDTH {
                // Scrolling pattern to show it's alive
                let pattern = ((x + y + self.frame_count as usize / 10) / 20) % 4;
                self.frame_buffer[y * GB_WIDTH + x] = pattern as u8;
            }
        }
    }

    /// Get palette-indexed pixel data for DMG
    fn get_pal_data(&self) -> &[u8] {
        &self.frame_buffer
    }

    /// Process a button press
    fn keydown(&mut self, _key: u8) {
        // In real implementation: self.cpu.mmu.keydown(key)
    }

    /// Process a button release
    fn keyup(&mut self, _key: u8) {
        // In real implementation: self.cpu.mmu.keyup(key)
    }
}

// ============================================================================
// Main Emulator Loop
// ============================================================================

fn run_emulator(fb: &Framebuffer, rom_data: &[u8]) {
    // Create emulator device
    let mut device = match GameBoyDevice::new(rom_data) {
        Ok(d) => d,
        Err(e) => {
            fb.clear(RED);
            draw_string(fb, 100, 200, "Emulator init failed!", WHITE, RED);
            draw_string(fb, 100, 220, e, WHITE, RED);
            loop { unsafe { core::arch::asm!("wfe"); } }
        }
    };

    // Draw border around GB screen
    fb.draw_gb_border(GRAY);

    let mut last_frame_time = micros();
    let mut prev_buttons = 0u16;

    loop {
        device.do_frame();
        fb.blit_gb_screen_dmg(device.get_pal_data());

        let buttons = read_buttons();
        let pressed = buttons & !prev_buttons;
        let released = !buttons & prev_buttons;

        // Map to GameBoy keys (Right=0, Left=1, Up=2, Down=3, A=4, B=5, Select=6, Start=7)
        if pressed & BTN_UP != 0     { device.keydown(2); }
        if released & BTN_UP != 0    { device.keyup(2); }
        if pressed & BTN_DOWN != 0   { device.keydown(3); }
        if released & BTN_DOWN != 0  { device.keyup(3); }
        if pressed & BTN_LEFT != 0   { device.keydown(1); }
        if released & BTN_LEFT != 0  { device.keyup(1); }
        if pressed & BTN_RIGHT != 0  { device.keydown(0); }
        if released & BTN_RIGHT != 0 { device.keyup(0); }
        if pressed & BTN_A != 0      { device.keydown(4); }
        if released & BTN_A != 0     { device.keyup(4); }
        if pressed & BTN_B != 0      { device.keydown(5); }
        if released & BTN_B != 0     { device.keyup(5); }
        if pressed & BTN_START != 0  { device.keydown(7); }
        if released & BTN_START != 0 { device.keyup(7); }
        if pressed & BTN_SELECT != 0 { device.keydown(6); }
        if released & BTN_SELECT != 0 { device.keyup(6); }

        // Extra buttons for special functions
        if pressed & BTN_HOME != 0 {
            // Return to ROM browser (reset)
            return;  // Would need to change return type from `-> !`
        }

        // L/R and X/Y can be used for turbo, fast-forward, save states, etc.

        prev_buttons = buttons;

        // Frame timing
        let target_time = last_frame_time.wrapping_add(FRAME_TIME_US);
        while micros().wrapping_sub(target_time) > 0x80000000 {}
        last_frame_time = micros();
    }
}

// ============================================================================
// Static ROM Buffer
// ============================================================================

static mut ROM_BUFFER: [u8; 2 * 1024 * 1024] = [0; 2 * 1024 * 1024];

// ============================================================================
// Main Entry Point
// ============================================================================

#[no_mangle]
pub extern "C" fn boot_main() -> ! {
    // Initialize DPI display
    configure_gpio_for_dpi();

    // Initialize framebuffer
    let fb = match Framebuffer::new() {
        Some(f) => f,
        None => loop { unsafe { core::arch::asm!("wfe"); } }
    };

    fb.clear(DARK_BLUE);

    let mut con = Console::new(&fb, WHITE, DARK_BLUE);

    // Title
    con.set_color(CYAN, DARK_BLUE);
    con.println("=== GB-OS for GPi Case 2W ===");
    con.newline();

    // Initialize buttons
    con.set_color(WHITE, DARK_BLUE);
    con.println("Configuring buttons...");
    configure_gpio_for_input();

    // Initialize filesystem
    con.println("Mounting SD card...");
    let mut fs = Fat32::new();

    match fs.mount() {
        Ok(()) => {
            con.set_color(GREEN, DARK_BLUE);
            con.println("SD card mounted!");
            con.set_color(WHITE, DARK_BLUE);
        }
        Err(e) => {
            con.set_color(RED, DARK_BLUE);
            let _ = write!(con, "Mount failed: {}\n", e);
            con.println("Insert SD card with FAT32 partition");
            loop { unsafe { core::arch::asm!("wfe"); } }
        }
    }

    // Count ROMs
    let mut rom_count = fs.count_roms();
    let _ = write!(con, "Found {} ROM(s)\n", rom_count);

    if rom_count == 0 {
        con.set_color(YELLOW, DARK_BLUE);
        con.println("No .gb or .gbc files found!");
        con.println("Place ROMs in SD card root directory");
        loop { unsafe { core::arch::asm!("wfe"); } }
    }

    con.newline();
    con.println("Starting ROM browser...");

    // ROM Browser
    let mut browser = RomBrowser::new(rom_count);
    browser.draw(&fb, &mut fs);

    let mut prev_buttons = 0u16;

    loop {
        let buttons = read_buttons();
        let pressed = buttons & !prev_buttons;

        if pressed & BTN_UP != 0 {
            browser.move_up();
            browser.draw(&fb, &mut fs);
        }
        if pressed & BTN_DOWN != 0 {
            browser.move_down();
            browser.draw(&fb, &mut fs);
        }

        if pressed & BTN_A != 0{  // A - Select
            let selected = browser.get_selection();

            // Load ROM
            fb.clear(DARK_BLUE);
            draw_string(&fb, 200, 200, "Loading ROM...", WHITE, DARK_BLUE);

            if let Some((cluster, size)) = fs.find_rom(selected) {
                let rom_buf = unsafe { &mut ROM_BUFFER };

                match fs.read_file(cluster, size, rom_buf) {
                    Ok(bytes_read) => {
                        let _ = write!(
                            &mut StringWriter::new(&fb, 180, 230, GREEN, DARK_BLUE),
                            "Loaded {} bytes", bytes_read
                        );

                        // Run emulator
                        let rom_slice = &rom_buf[..bytes_read];
                        run_emulator(&fb, rom_slice);
                    }
                    Err(e) => {
                        draw_string(&fb, 180, 230, "Load failed!", RED, DARK_BLUE);
                        draw_string(&fb, 180, 250, e, RED, DARK_BLUE);
                        browser.draw(&fb, &mut fs);
                    }
                }
            }
        }

        prev_buttons = buttons;
        delay_us(16000);  // ~60Hz polling
    }
}

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfe"); } }
}
