//! GPi Case 2W - Native 640x480 with correct colors
//!
//! WORKING! Full screen 640x480 display!
//! Fixed color order: RGB format

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
const GPPUD: u32 = GPIO_BASE + 0x94;
const GPPUDCLK0: u32 = GPIO_BASE + 0x98;

const MAILBOX_BASE: u32 = PERIPHERAL_BASE + 0x0000B880;
const MAILBOX_READ: u32 = MAILBOX_BASE + 0x00;
const MAILBOX_STATUS: u32 = MAILBOX_BASE + 0x18;
const MAILBOX_WRITE: u32 = MAILBOX_BASE + 0x20;

const GPIO_FUNC_ALT2: u32 = 0b110;

fn mmio_write(reg: u32, val: u32) { unsafe { write_volatile(reg as *mut u32, val) } }
fn mmio_read(reg: u32) -> u32 { unsafe { read_volatile(reg as *const u32) } }
fn delay(count: u32) { for _ in 0..count { unsafe { core::arch::asm!("nop") } } }

fn configure_gpio_for_dpi() {
    let gpfsel0_val: u32 =
        (GPIO_FUNC_ALT2 << 0)  | (GPIO_FUNC_ALT2 << 3)  | (GPIO_FUNC_ALT2 << 6)  |
            (GPIO_FUNC_ALT2 << 9)  | (GPIO_FUNC_ALT2 << 12) | (GPIO_FUNC_ALT2 << 15) |
            (GPIO_FUNC_ALT2 << 18) | (GPIO_FUNC_ALT2 << 21) | (GPIO_FUNC_ALT2 << 24) |
            (GPIO_FUNC_ALT2 << 27);
    mmio_write(GPFSEL0, gpfsel0_val);

    let gpfsel1_val: u32 =
        (GPIO_FUNC_ALT2 << 0)  | (GPIO_FUNC_ALT2 << 3)  | (GPIO_FUNC_ALT2 << 6)  |
            (GPIO_FUNC_ALT2 << 9)  | (GPIO_FUNC_ALT2 << 12) | (GPIO_FUNC_ALT2 << 15) |
            (GPIO_FUNC_ALT2 << 18) | (GPIO_FUNC_ALT2 << 21) | (GPIO_FUNC_ALT2 << 24) |
            (GPIO_FUNC_ALT2 << 27);
    mmio_write(GPFSEL1, gpfsel1_val);

    let gpfsel2_val: u32 = (GPIO_FUNC_ALT2 << 0) | (GPIO_FUNC_ALT2 << 3) | (1 << 27);
    mmio_write(GPFSEL2, gpfsel2_val);

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

static mut FB_ADDR: *mut u32 = core::ptr::null_mut();
static mut FB_WIDTH: u32 = 0;
static mut FB_HEIGHT: u32 = 0;
static mut FB_PITCH: u32 = 0;

fn fb_init() -> bool {
    unsafe {
        for i in 0..36 { MBOX.data[i] = 0; }

        let mut i = 0;
        MBOX.data[i] = 35 * 4; i += 1;
        MBOX.data[i] = 0; i += 1;

        // Request 640x480
        MBOX.data[i] = 0x00048003; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 640; i += 1;
        MBOX.data[i] = 480; i += 1;

        MBOX.data[i] = 0x00048004; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 640; i += 1;
        MBOX.data[i] = 480; i += 1;

        MBOX.data[i] = 0x00048009; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 0; i += 1;

        MBOX.data[i] = 0x00048005; i += 1;
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 32; i += 1;

        MBOX.data[i] = 0x00048006; i += 1;
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 1; i += 1;  // RGB order (was 0 for BGR)

        MBOX.data[i] = 0x00040001; i += 1;
        MBOX.data[i] = 8; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 4096; i += 1;
        MBOX.data[i] = 0; i += 1;

        MBOX.data[i] = 0x00040008; i += 1;
        MBOX.data[i] = 4; i += 1;
        MBOX.data[i] = 0; i += 1;
        MBOX.data[i] = 0; i += 1;

        MBOX.data[i] = 0;

        if !mailbox_call(8) {
            return false;
        }

        FB_WIDTH = MBOX.data[5];
        FB_HEIGHT = MBOX.data[6];
        FB_PITCH = MBOX.data[33];

        let fb_bus_addr = MBOX.data[28];
        if fb_bus_addr == 0 || FB_PITCH == 0 {
            return false;
        }

        FB_ADDR = (fb_bus_addr & 0x3FFFFFFF) as *mut u32;
        true
    }
}

/// Create color in ARGB format
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

#[inline(always)]
fn fb_pixel(x: u32, y: u32, color: u32) {
    unsafe {
        if x >= FB_WIDTH || y >= FB_HEIGHT || FB_ADDR.is_null() {
            return;
        }
        let pitch_px = FB_PITCH / 4;
        let offset = y * pitch_px + x;
        write_volatile(FB_ADDR.add(offset as usize), color);
    }
}

fn draw_pattern() {
    unsafe {
        let w = FB_WIDTH;
        let h = FB_HEIGHT;

        // Clear to dark blue
        for y in 0..h {
            for x in 0..w {
                fb_pixel(x, y, rgb(0, 0, 64));
            }
        }

        // Cyan border
        let cyan = rgb(0, 255, 255);
        for i in 0..8 {
            for x in 0..w { fb_pixel(x, i, cyan); fb_pixel(x, h - 1 - i, cyan); }
            for y in 0..h { fb_pixel(i, y, cyan); fb_pixel(w - 1 - i, y, cyan); }
        }

        // Horizontal color bars
        let bar_h = h / 8;
        let colors = [
            rgb(255, 0, 0),     // Red
            rgb(0, 255, 0),     // Green
            rgb(0, 0, 255),     // Blue
            rgb(255, 255, 0),   // Yellow
            rgb(255, 0, 255),   // Magenta
            rgb(0, 255, 255),   // Cyan
        ];

        for (idx, &color) in colors.iter().enumerate() {
            let y_start = 30 + (idx as u32) * bar_h;
            let y_end = (y_start + bar_h - 4).min(h - 30);
            for y in y_start..y_end {
                for x in 20..(w - 20) {
                    fb_pixel(x, y, color);
                }
            }
        }

        // Corner markers
        let cs = 30u32;
        // Top-left: Red
        for y in 15..(15 + cs) {
            for x in 15..(15 + cs) { fb_pixel(x, y, rgb(255, 0, 0)); }
        }
        // Top-right: Green
        for y in 15..(15 + cs) {
            for x in (w - 15 - cs)..(w - 15) { fb_pixel(x, y, rgb(0, 255, 0)); }
        }
        // Bottom-left: Blue
        for y in (h - 15 - cs)..(h - 15) {
            for x in 15..(15 + cs) { fb_pixel(x, y, rgb(0, 0, 255)); }
        }
        // Bottom-right: White
        for y in (h - 15 - cs)..(h - 15) {
            for x in (w - 15 - cs)..(w - 15) { fb_pixel(x, y, rgb(255, 255, 255)); }
        }

        // Diagonal
        for i in 0..w.min(h) {
            fb_pixel(i, i, rgb(255, 255, 255));
        }

        // Center cross
        let cx = w / 2;
        let cy = h / 2;
        for x in (cx - 50)..(cx + 50) { fb_pixel(x, cy, rgb(255, 255, 255)); }
        for y in (cy - 50)..(cy + 50) { fb_pixel(cx, y, rgb(255, 255, 255)); }
    }
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    delay(500000);
    configure_gpio_for_dpi();

    if !fb_init() {
        loop { delay(1000000); }
    }

    draw_pattern();

    loop { core::hint::spin_loop(); }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop { core::hint::spin_loop(); }
}

#[used]
#[link_section = ".rodata.padding"]
static PADDING: [u8; 131072] = [0xAA; 131072];
