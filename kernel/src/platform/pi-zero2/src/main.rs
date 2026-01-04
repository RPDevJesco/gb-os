//! GPi Case 2W - Hardcoded Native 240x320 Version
//!
//! This kernel hardcodes the native panel dimensions:
//! - Physical: 240 wide x 320 tall (portrait)
//! - Pitch: 960 bytes (240 * 4)
//!
//! Software rotation provides 320x240 landscape interface.
//! This avoids relying on GPU-returned values that may be wrong.

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::ptr::{read_volatile, write_volatile};

global_asm!(
    ".section .text.boot",
    "_start:",
    "    mrs x0, mpidr_el1",
    "    and x0, x0, #3",
    "    cbz x0, 2f",
    "1:  wfe",
    "    b 1b",
    "2:  ldr x0, =0x80000",
    "    mov sp, x0",
    "    bl kernel_main",
    "    b 1b",
);

const PERIPHERAL_BASE: u32 = 0x3F000000;
const GPIO_BASE: u32 = PERIPHERAL_BASE + 0x00200000;
const GPFSEL0: u32 = GPIO_BASE + 0x00;
const GPFSEL1: u32 = GPIO_BASE + 0x04;
const GPFSEL2: u32 = GPIO_BASE + 0x08;
const GPSET0: u32 = GPIO_BASE + 0x1C;
const GPCLR0: u32 = GPIO_BASE + 0x28;
const GPPUD: u32 = GPIO_BASE + 0x94;
const GPPUDCLK0: u32 = GPIO_BASE + 0x98;

const MAILBOX_BASE: u32 = PERIPHERAL_BASE + 0x0000B880;
const MAILBOX_READ: u32 = MAILBOX_BASE + 0x00;
const MAILBOX_STATUS: u32 = MAILBOX_BASE + 0x18;
const MAILBOX_WRITE: u32 = MAILBOX_BASE + 0x20;

const ACT_LED_PIN: u32 = 29;
const GPIO_FUNC_ALT2: u32 = 0b110;

fn mmio_write(reg: u32, val: u32) { unsafe { write_volatile(reg as *mut u32, val) } }
fn mmio_read(reg: u32) -> u32 { unsafe { read_volatile(reg as *const u32) } }
fn delay(count: u32) { for _ in 0..count { unsafe { core::arch::asm!("nop") } } }

fn led_init() {
    let sel = mmio_read(GPFSEL2);
    mmio_write(GPFSEL2, (sel & !(7 << 27)) | (1 << 27));
}
fn led_on() { mmio_write(GPCLR0, 1 << ACT_LED_PIN); }
fn led_off() { mmio_write(GPSET0, 1 << ACT_LED_PIN); }
fn led_blink(count: u32) {
    for _ in 0..count {
        led_on(); delay(200000);
        led_off(); delay(200000);
    }
    delay(400000);
}

/// Configure GPIO 0-21 for DPI (ALT2)
fn configure_gpio_for_dpi() {
    // GPFSEL0: GPIO 0-9, GPFSEL1: GPIO 10-19, GPFSEL2: GPIO 20-29
    // Each GPIO uses 3 bits, ALT2 = 0b110

    // GPIO 0-9: all ALT2
    let gpfsel0_val: u32 =
        (GPIO_FUNC_ALT2 << 0)  |  // GPIO 0
            (GPIO_FUNC_ALT2 << 3)  |  // GPIO 1
            (GPIO_FUNC_ALT2 << 6)  |  // GPIO 2
            (GPIO_FUNC_ALT2 << 9)  |  // GPIO 3
            (GPIO_FUNC_ALT2 << 12) |  // GPIO 4
            (GPIO_FUNC_ALT2 << 15) |  // GPIO 5
            (GPIO_FUNC_ALT2 << 18) |  // GPIO 6
            (GPIO_FUNC_ALT2 << 21) |  // GPIO 7
            (GPIO_FUNC_ALT2 << 24) |  // GPIO 8
            (GPIO_FUNC_ALT2 << 27);   // GPIO 9
    mmio_write(GPFSEL0, gpfsel0_val);

    // GPIO 10-19: all ALT2
    let gpfsel1_val: u32 =
        (GPIO_FUNC_ALT2 << 0)  |  // GPIO 10
            (GPIO_FUNC_ALT2 << 3)  |  // GPIO 11
            (GPIO_FUNC_ALT2 << 6)  |  // GPIO 12
            (GPIO_FUNC_ALT2 << 9)  |  // GPIO 13
            (GPIO_FUNC_ALT2 << 12) |  // GPIO 14
            (GPIO_FUNC_ALT2 << 15) |  // GPIO 15
            (GPIO_FUNC_ALT2 << 18) |  // GPIO 16
            (GPIO_FUNC_ALT2 << 21) |  // GPIO 17
            (GPIO_FUNC_ALT2 << 24) |  // GPIO 18
            (GPIO_FUNC_ALT2 << 27);   // GPIO 19
    mmio_write(GPFSEL1, gpfsel1_val);

    // GPIO 20-21: ALT2, preserve GPIO 29 (LED)
    let gpfsel2_val: u32 =
        (GPIO_FUNC_ALT2 << 0)  |  // GPIO 20
            (GPIO_FUNC_ALT2 << 3)  |  // GPIO 21
            (1 << 27);                // GPIO 29 = output for LED
    mmio_write(GPFSEL2, gpfsel2_val);

    // Disable pull-up/down on GPIO 0-21
    mmio_write(GPPUD, 0);
    delay(150);
    mmio_write(GPPUDCLK0, 0x003FFFFF);
    delay(150);
    mmio_write(GPPUDCLK0, 0);
}

#[repr(C, align(16))]
struct MailboxBuffer { data: [u32; 36] }
static mut MBOX: MailboxBuffer = MailboxBuffer { data: [0; 36] };

fn mailbox_call(channel: u8) -> bool {
    unsafe {
        let addr = core::ptr::addr_of!(MBOX) as u32;
        while (mmio_read(MAILBOX_STATUS) & 0x80000000) != 0 {}
        mmio_write(MAILBOX_WRITE, (addr & 0xFFFFFFF0) | (channel as u32));
        loop {
            while (mmio_read(MAILBOX_STATUS) & 0x40000000) != 0 {}
            let data = mmio_read(MAILBOX_READ);
            if (data & 0xF) == channel as u32 {
                return MBOX.data[1] == 0x80000000;
            }
        }
    }
}

// HARDCODED native panel dimensions - no reliance on GPU response!
const PHYS_WIDTH: u32 = 240;   // Native width (portrait)
const PHYS_HEIGHT: u32 = 320;  // Native height (portrait)
const PHYS_PITCH: u32 = 960;   // 240 * 4 bytes

// Logical dimensions after rotation (landscape)
const SCREEN_WIDTH: u32 = 320;
const SCREEN_HEIGHT: u32 = 240;

static mut FB_ADDR: *mut u8 = core::ptr::null_mut();

fn fb_init() -> bool {
    unsafe {
        for i in 0..36 { MBOX.data[i] = 0; }

        let mut i = 0;
        MBOX.data[i] = 35 * 4; i += 1;  // Buffer size
        MBOX.data[i] = 0; i += 1;        // Request

        // Request NATIVE 240x320 - not rotated!
        MBOX.data[i] = 0x00048003; i += 1;  // Set physical size
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = PHYS_WIDTH; i += 1;   // 240
        MBOX.data[i] = PHYS_HEIGHT; i += 1;  // 320

        MBOX.data[i] = 0x00048004; i += 1;  // Set virtual size
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = PHYS_WIDTH; i += 1;
        MBOX.data[i] = PHYS_HEIGHT; i += 1;

        MBOX.data[i] = 0x00048009; i += 1;  // Set offset
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 0; i += 1;

        MBOX.data[i] = 0x00048005; i += 1;  // Set depth
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 32; i += 1;

        MBOX.data[i] = 0x00048006; i += 1;  // Set pixel order
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 0; i += 1;  // BGR

        MBOX.data[i] = 0x00040001; i += 1;  // Allocate
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 4096; i += 1;  // Alignment
        MBOX.data[i] = 0; i += 1;

        MBOX.data[i] = 0x00040008; i += 1;  // Get pitch
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 0; i += 1;

        MBOX.data[i] = 0;  // End tag

        if !mailbox_call(8) {
            return false;
        }

        let fb_bus_addr = MBOX.data[28];
        if fb_bus_addr == 0 {
            return false;
        }

        FB_ADDR = (fb_bus_addr & 0x3FFFFFFF) as *mut u8;
        true
    }
}

/// Draw at physical coordinates (no rotation)
#[inline(always)]
fn fb_pixel_phys(px: u32, py: u32, color: u32) {
    unsafe {
        if px >= PHYS_WIDTH || py >= PHYS_HEIGHT || FB_ADDR.is_null() {
            return;
        }
        let offset = (py * PHYS_PITCH) + (px * 4);
        let ptr = FB_ADDR.add(offset as usize) as *mut u32;
        write_volatile(ptr, color);
    }
}

/// Draw at logical coordinates (with 90° CW rotation)
/// Logical (0,0) = top-left of landscape screen
/// Maps to physical for portrait panel
#[inline(always)]
fn fb_pixel(lx: u32, ly: u32, color: u32) {
    // 90° clockwise rotation:
    // logical(lx, ly) -> physical(ly, SCREEN_WIDTH - 1 - lx)
    let px = ly;
    let py = SCREEN_WIDTH - 1 - lx;
    fb_pixel_phys(px, py, color);
}

const fn bgra(r: u8, g: u8, b: u8) -> u32 {
    (b as u32) << 16 | (g as u32) << 8 | (r as u32) | 0xFF000000
}

fn fb_clear(color: u32) {
    for py in 0..PHYS_HEIGHT {
        for px in 0..PHYS_WIDTH {
            fb_pixel_phys(px, py, color);
        }
    }
}

fn fb_fill_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    for ly in y..(y + h).min(SCREEN_HEIGHT) {
        for lx in x..(x + w).min(SCREEN_WIDTH) {
            fb_pixel(lx, ly, color);
        }
    }
}

fn draw_test_pattern() {
    // Clear to dark blue
    fb_clear(bgra(0, 0, 64));

    // Cyan border (5 pixels)
    let cyan = bgra(0, 255, 255);
    for i in 0..5 {
        for x in 0..SCREEN_WIDTH {
            fb_pixel(x, i, cyan);
            fb_pixel(x, SCREEN_HEIGHT - 1 - i, cyan);
        }
        for y in 0..SCREEN_HEIGHT {
            fb_pixel(i, y, cyan);
            fb_pixel(SCREEN_WIDTH - 1 - i, y, cyan);
        }
    }

    // Horizontal color bars
    let bar_h = 25u32;
    let colors = [
        bgra(255, 0, 0),     // Red
        bgra(0, 255, 0),     // Green
        bgra(0, 0, 255),     // Blue
        bgra(255, 255, 0),   // Yellow
        bgra(255, 0, 255),   // Magenta
        bgra(0, 255, 255),   // Cyan
        bgra(255, 255, 255), // White
    ];

    for (idx, &color) in colors.iter().enumerate() {
        let y = 20 + (idx as u32) * bar_h;
        fb_fill_rect(10, y, SCREEN_WIDTH - 20, bar_h - 3, color);
    }

    // Corner markers
    let cs = 20u32;
    fb_fill_rect(8, 8, cs, cs, bgra(255, 0, 0));                              // Top-left: Red
    fb_fill_rect(SCREEN_WIDTH - cs - 8, 8, cs, cs, bgra(0, 255, 0));          // Top-right: Green
    fb_fill_rect(8, SCREEN_HEIGHT - cs - 8, cs, cs, bgra(0, 0, 255));         // Bottom-left: Blue
    fb_fill_rect(SCREEN_WIDTH - cs - 8, SCREEN_HEIGHT - cs - 8, cs, cs, bgra(255, 255, 0)); // Bottom-right: Yellow

    // Diagonal line
    let white = bgra(255, 255, 255);
    for i in 0..SCREEN_WIDTH.min(SCREEN_HEIGHT) {
        fb_pixel(i, i, white);
    }
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    led_init();
    led_blink(1);

    configure_gpio_for_dpi();
    led_blink(2);

    if !fb_init() {
        loop { led_on(); delay(50000); led_off(); delay(50000); }
    }
    led_blink(3);

    draw_test_pattern();
    led_blink(4);

    led_on();
    loop { core::hint::spin_loop(); }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop { led_on(); delay(30000); led_off(); delay(30000); }
}

// 128KB padding
#[used]
#[link_section = ".rodata.padding"]
static PADDING: [u8; 131072] = [0xAA; 131072];
