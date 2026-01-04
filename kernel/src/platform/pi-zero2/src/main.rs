//! Raspberry Pi Zero 2 W - GPi Case 2W Fixed Kernel
//!
//! Fixed version that properly handles framebuffer pitch/stride.
//! The issue was that the display pitch may not equal width * bytes_per_pixel.

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

// Hardware constants
const PERIPHERAL_BASE: u32 = 0x3F000000;
const GPIO_BASE: u32 = PERIPHERAL_BASE + 0x00200000;
const GPFSEL2: u32 = GPIO_BASE + 0x08;
const GPSET0: u32 = GPIO_BASE + 0x1C;
const GPCLR0: u32 = GPIO_BASE + 0x28;

const MAILBOX_BASE: u32 = PERIPHERAL_BASE + 0x0000B880;
const MAILBOX_READ: u32 = MAILBOX_BASE + 0x00;
const MAILBOX_STATUS: u32 = MAILBOX_BASE + 0x18;
const MAILBOX_WRITE: u32 = MAILBOX_BASE + 0x20;
const MAILBOX_FULL: u32 = 0x80000000;
const MAILBOX_EMPTY: u32 = 0x40000000;

// Mailbox tags
const TAG_FB_SET_PHYS_WH: u32 = 0x00048003;
const TAG_FB_SET_VIRT_WH: u32 = 0x00048004;
const TAG_FB_SET_VIRT_OFF: u32 = 0x00048009;
const TAG_FB_SET_DEPTH: u32 = 0x00048005;
const TAG_FB_SET_PIXEL_ORDER: u32 = 0x00048006;
const TAG_FB_ALLOC: u32 = 0x00040001;
const TAG_FB_GET_PITCH: u32 = 0x00040008;
const TAG_END: u32 = 0;

const ACT_LED_PIN: u32 = 29;
const BLINK_DELAY: u32 = 200000;

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
        led_on(); delay(BLINK_DELAY);
        led_off(); delay(BLINK_DELAY);
    }
    delay(BLINK_DELAY * 4);
}

#[repr(C, align(16))]
struct MailboxBuffer { data: [u32; 36], }
static mut MBOX: MailboxBuffer = MailboxBuffer { data: [0; 36] };

fn mailbox_call(channel: u8) -> bool {
    unsafe {
        let addr = core::ptr::addr_of!(MBOX) as u32;
        while (mmio_read(MAILBOX_STATUS) & MAILBOX_FULL) != 0 {}
        mmio_write(MAILBOX_WRITE, (addr & 0xFFFFFFF0) | (channel as u32));
        loop {
            while (mmio_read(MAILBOX_STATUS) & MAILBOX_EMPTY) != 0 {}
            let data = mmio_read(MAILBOX_READ);
            if (data & 0xF) == channel as u32 {
                return MBOX.data[1] == 0x80000000;
            }
        }
    }
}

// Framebuffer
static mut FB_ADDR: *mut u8 = core::ptr::null_mut();
static mut FB_WIDTH: u32 = 0;
static mut FB_HEIGHT: u32 = 0;
static mut FB_PITCH: u32 = 0;  // Bytes per row (may differ from width * bpp!)
static mut FB_BPP: u32 = 0;    // Bytes per pixel

fn fb_init() -> bool {
    unsafe {
        // Use the NATIVE resolution of the panel: 240x320
        // The GPU will handle rotation via display_rotate=1 in config.txt
        // BUT we request the LOGICAL (post-rotation) size
        let width: u32 = 320;
        let height: u32 = 240;
        let depth: u32 = 16;  // Try 16bpp - might work better with DPI

        for i in 0..36 { MBOX.data[i] = 0; }

        let mut i = 0;
        MBOX.data[i] = 35 * 4; i += 1;  // Buffer size
        MBOX.data[i] = 0; i += 1;        // Request

        // Physical size
        MBOX.data[i] = TAG_FB_SET_PHYS_WH; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 8; i += 1;  // Request size = response size
        MBOX.data[i] = width; i += 1;   // 5
        MBOX.data[i] = height; i += 1;  // 6

        // Virtual size (same as physical)
        MBOX.data[i] = TAG_FB_SET_VIRT_WH; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = width; i += 1;   // 10
        MBOX.data[i] = height; i += 1;  // 11

        // Virtual offset
        MBOX.data[i] = TAG_FB_SET_VIRT_OFF; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 0; i += 1;  // 15
        MBOX.data[i] = 0; i += 1;  // 16

        // Depth
        MBOX.data[i] = TAG_FB_SET_DEPTH; i += 1;
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = depth; i += 1;  // 20

        // Pixel order (0=BGR, 1=RGB) - try BGR for DPI
        MBOX.data[i] = TAG_FB_SET_PIXEL_ORDER; i += 1;
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 0; i += 1;  // 24: BGR order

        // Allocate framebuffer
        MBOX.data[i] = TAG_FB_ALLOC; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 4096; i += 1;  // 28: Alignment (4K)
        MBOX.data[i] = 0; i += 1;     // 29: Size (response)

        // Get pitch
        MBOX.data[i] = TAG_FB_GET_PITCH; i += 1;
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 0; i += 1;  // 33: Pitch (response)

        MBOX.data[i] = TAG_END;

        if !mailbox_call(8) {
            return false;
        }

        // Extract response values
        // Response overwrites request values in place
        FB_WIDTH = MBOX.data[5];   // Physical width response
        FB_HEIGHT = MBOX.data[6];  // Physical height response

        let actual_depth = MBOX.data[20];
        FB_BPP = actual_depth / 8;

        let fb_bus_addr = MBOX.data[28];
        if fb_bus_addr == 0 {
            return false;
        }
        FB_ADDR = (fb_bus_addr & 0x3FFFFFFF) as *mut u8;

        // CRITICAL: Use the pitch from GPU, not calculated!
        FB_PITCH = MBOX.data[33];

        // Sanity check
        if FB_PITCH == 0 {
            // Fallback: calculate pitch
            FB_PITCH = FB_WIDTH * FB_BPP;
        }

        true
    }
}

/// Put pixel using the ACTUAL pitch from GPU
#[inline]
fn fb_pixel(x: u32, y: u32, color: u16) {
    unsafe {
        if x >= FB_WIDTH || y >= FB_HEIGHT || FB_ADDR.is_null() {
            return;
        }
        // Use FB_PITCH (bytes per row) not width * bpp
        let offset = y * FB_PITCH + x * 2;  // 2 bytes for 16bpp
        let ptr = FB_ADDR.add(offset as usize) as *mut u16;
        *ptr = color;
    }
}

/// RGB565 color: 5 bits red, 6 bits green, 5 bits blue
#[inline]
const fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | (b as u16 >> 3)
}

fn fb_clear(color: u16) {
    unsafe {
        for y in 0..FB_HEIGHT {
            for x in 0..FB_WIDTH {
                fb_pixel(x, y, color);
            }
        }
    }
}

fn fb_fill_rect(x: u32, y: u32, w: u32, h: u32, color: u16) {
    unsafe {
        let max_x = (x + w).min(FB_WIDTH);
        let max_y = (y + h).min(FB_HEIGHT);
        for py in y..max_y {
            for px in x..max_x {
                fb_pixel(px, py, color);
            }
        }
    }
}

fn draw_test_pattern() {
    unsafe {
        let w = FB_WIDTH;
        let h = FB_HEIGHT;

        // Clear to dark blue
        fb_clear(rgb565(0, 0, 64));

        // Draw white border
        for i in 0..3 {
            for x in 0..w {
                fb_pixel(x, i, rgb565(255, 255, 255));
                fb_pixel(x, h - 1 - i, rgb565(255, 255, 255));
            }
            for y in 0..h {
                fb_pixel(i, y, rgb565(255, 255, 255));
                fb_pixel(w - 1 - i, y, rgb565(255, 255, 255));
            }
        }

        // Color bars across FULL width
        let bar_h = 30u32;
        let colors = [
            rgb565(255, 0, 0),     // Red
            rgb565(0, 255, 0),     // Green
            rgb565(0, 0, 255),     // Blue
            rgb565(255, 255, 0),   // Yellow
            rgb565(255, 0, 255),   // Magenta
            rgb565(0, 255, 255),   // Cyan
            rgb565(255, 255, 255), // White
        ];

        for (i, &color) in colors.iter().enumerate() {
            fb_fill_rect(10, 20 + i as u32 * bar_h, w - 20, bar_h - 2, color);
        }

        // Diagonal line to verify aspect ratio
        let diag_len = w.min(h);
        for i in 0..diag_len {
            fb_pixel(i, i, rgb565(255, 255, 255));
            fb_pixel(i + 1, i, rgb565(255, 255, 255));
        }
    }
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    led_init();
    led_blink(1);

    if !fb_init() {
        // Error: rapid blinking
        loop {
            led_on(); delay(50000);
            led_off(); delay(50000);
        }
    }

    led_blink(2);

    draw_test_pattern();

    led_blink(3);
    led_on();

    loop { core::hint::spin_loop(); }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop { led_on(); delay(30000); led_off(); delay(30000); }
}
