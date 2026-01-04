//! Raspberry Pi Zero 2 W - GPi Case 2W Kernel
//!
//! COMPREHENSIVE FIX for GPi Case 2W display issues:
//!
//! Problem 1: BPP Mismatch
//!   - DPI RGB666 padded mode requires 32bpp framebuffer
//!   - Previous code requested 16bpp, causing pitch corruption
//!
//! Problem 2: GPIO Configuration
//!   - GPIO 0-21 must be set to ALT2 for DPI function
//!   - SPI uses some of these pins (GPIO 7-11) by default
//!   - We must explicitly configure GPIO in kernel, not just config.txt
//!
//! This version:
//!   1. Explicitly sets GPIO 0-21 to ALT2 (DPI mode)
//!   2. Disables pull-up/down on those pins
//!   3. Requests 32bpp framebuffer
//!   4. Uses correct BGRA pixel format

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// Boot Entry Point
// ============================================================================

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

// ============================================================================
// Hardware Constants
// ============================================================================

const PERIPHERAL_BASE: u32 = 0x3F000000;

// GPIO Registers
const GPIO_BASE: u32 = PERIPHERAL_BASE + 0x00200000;
const GPFSEL0: u32 = GPIO_BASE + 0x00;  // GPIO 0-9 function select
const GPFSEL1: u32 = GPIO_BASE + 0x04;  // GPIO 10-19 function select
const GPFSEL2: u32 = GPIO_BASE + 0x08;  // GPIO 20-29 function select
const GPSET0: u32 = GPIO_BASE + 0x1C;   // GPIO set
const GPCLR0: u32 = GPIO_BASE + 0x28;   // GPIO clear
const GPLEV0: u32 = GPIO_BASE + 0x34;   // GPIO level
const GPPUD: u32 = GPIO_BASE + 0x94;    // Pull-up/down enable
const GPPUDCLK0: u32 = GPIO_BASE + 0x98; // Pull-up/down clock

// Mailbox Registers
const MAILBOX_BASE: u32 = PERIPHERAL_BASE + 0x0000B880;
const MAILBOX_READ: u32 = MAILBOX_BASE + 0x00;
const MAILBOX_STATUS: u32 = MAILBOX_BASE + 0x18;
const MAILBOX_WRITE: u32 = MAILBOX_BASE + 0x20;
const MAILBOX_FULL: u32 = 0x80000000;
const MAILBOX_EMPTY: u32 = 0x40000000;

// Mailbox Tags
const TAG_FB_SET_PHYS_WH: u32 = 0x00048003;
const TAG_FB_SET_VIRT_WH: u32 = 0x00048004;
const TAG_FB_SET_VIRT_OFF: u32 = 0x00048009;
const TAG_FB_SET_DEPTH: u32 = 0x00048005;
const TAG_FB_SET_PIXEL_ORDER: u32 = 0x00048006;
const TAG_FB_ALLOC: u32 = 0x00040001;
const TAG_FB_GET_PITCH: u32 = 0x00040008;
const TAG_END: u32 = 0;

// LED (active low on Pi Zero 2 W)
const ACT_LED_PIN: u32 = 29;
const BLINK_DELAY: u32 = 200000;

// GPIO Function Codes
const GPIO_FUNC_INPUT: u32 = 0b000;
const GPIO_FUNC_OUTPUT: u32 = 0b001;
const GPIO_FUNC_ALT0: u32 = 0b100;
const GPIO_FUNC_ALT1: u32 = 0b101;
const GPIO_FUNC_ALT2: u32 = 0b110;  // DPI function!
const GPIO_FUNC_ALT3: u32 = 0b111;
const GPIO_FUNC_ALT4: u32 = 0b011;
const GPIO_FUNC_ALT5: u32 = 0b010;

// ============================================================================
// Low-level I/O
// ============================================================================

#[inline(always)]
fn mmio_write(reg: u32, val: u32) {
    unsafe { write_volatile(reg as *mut u32, val) }
}

#[inline(always)]
fn mmio_read(reg: u32) -> u32 {
    unsafe { read_volatile(reg as *const u32) }
}

#[inline(always)]
fn delay(count: u32) {
    for _ in 0..count {
        unsafe { core::arch::asm!("nop") }
    }
}

// ============================================================================
// GPIO Configuration
// ============================================================================

/// Configure GPIO 0-21 for DPI (ALT2 function)
/// This is CRITICAL - without this, the display won't work properly
fn configure_gpio_for_dpi() {
    // DPI Pin Assignment (ALT2):
    // GPIO 0:  PCLK (pixel clock)
    // GPIO 1:  DE (data enable)
    // GPIO 2:  VSYNC
    // GPIO 3:  HSYNC
    // GPIO 4-9:  Blue [2:7]
    // GPIO 10-15: Green [2:7]
    // GPIO 16-21: Red [2:7]

    // GPFSEL0 controls GPIO 0-9 (3 bits per pin)
    // All 10 pins set to ALT2 (0b110)
    let gpfsel0_val: u32 =
        (GPIO_FUNC_ALT2 << 0)  |  // GPIO 0: PCLK
            (GPIO_FUNC_ALT2 << 3)  |  // GPIO 1: DE
            (GPIO_FUNC_ALT2 << 6)  |  // GPIO 2: VSYNC
            (GPIO_FUNC_ALT2 << 9)  |  // GPIO 3: HSYNC
            (GPIO_FUNC_ALT2 << 12) |  // GPIO 4: B2
            (GPIO_FUNC_ALT2 << 15) |  // GPIO 5: B3
            (GPIO_FUNC_ALT2 << 18) |  // GPIO 6: B4
            (GPIO_FUNC_ALT2 << 21) |  // GPIO 7: B5 (also SPI CE1!)
            (GPIO_FUNC_ALT2 << 24) |  // GPIO 8: B6 (also SPI CE0!)
            (GPIO_FUNC_ALT2 << 27);   // GPIO 9: B7 (also SPI MISO!)

    // GPFSEL1 controls GPIO 10-19
    let gpfsel1_val: u32 =
        (GPIO_FUNC_ALT2 << 0)  |  // GPIO 10: G2 (also SPI MOSI!)
            (GPIO_FUNC_ALT2 << 3)  |  // GPIO 11: G3 (also SPI SCLK!)
            (GPIO_FUNC_ALT2 << 6)  |  // GPIO 12: G4
            (GPIO_FUNC_ALT2 << 9)  |  // GPIO 13: G5
            (GPIO_FUNC_ALT2 << 12) |  // GPIO 14: G6 (also UART TX!)
            (GPIO_FUNC_ALT2 << 15) |  // GPIO 15: G7 (also UART RX!)
            (GPIO_FUNC_ALT2 << 18) |  // GPIO 16: R2
            (GPIO_FUNC_ALT2 << 21) |  // GPIO 17: R3
            (GPIO_FUNC_ALT2 << 24) |  // GPIO 18: R4
            (GPIO_FUNC_ALT2 << 27);   // GPIO 19: R5

    // GPFSEL2 controls GPIO 20-29
    // Only modify GPIO 20-21 for DPI, preserve 22-29 (especially 29 for LED)
    let gpfsel2_current = mmio_read(GPFSEL2);
    // Clear bits for GPIO 20-21 (bits 0-5), preserve rest
    let gpfsel2_val: u32 = (gpfsel2_current & !0x3F) |
        (GPIO_FUNC_ALT2 << 0) |  // GPIO 20: R6
        (GPIO_FUNC_ALT2 << 3);   // GPIO 21: R7

    // Write GPIO function registers
    mmio_write(GPFSEL0, gpfsel0_val);
    mmio_write(GPFSEL1, gpfsel1_val);
    mmio_write(GPFSEL2, gpfsel2_val);

    // Disable pull-up/down on GPIO 0-21
    // This is important for clean signal integrity
    disable_gpio_pulls();
}

/// Disable pull-up/down resistors on GPIO 0-21
fn disable_gpio_pulls() {
    // BCM2835/BCM2710 pull-up/down sequence:
    // 1. Write to GPPUD to set control signal (0 = off)
    // 2. Wait 150 cycles
    // 3. Write to GPPUDCLK0 to clock signal into pins
    // 4. Wait 150 cycles
    // 5. Write to GPPUD to remove control signal
    // 6. Write to GPPUDCLK0 to remove clock

    // Set pull to off (0 = disable pull-up/down)
    mmio_write(GPPUD, 0);
    delay(150);

    // Clock the control signal into GPIO 0-21
    // Bits 0-21 = 0x003FFFFF
    mmio_write(GPPUDCLK0, 0x003FFFFF);
    delay(150);

    // Remove control signal
    mmio_write(GPPUD, 0);
    mmio_write(GPPUDCLK0, 0);
}

/// Verify GPIO configuration (returns true if all pins are ALT2)
fn verify_gpio_config() -> bool {
    let gpfsel0 = mmio_read(GPFSEL0);
    let gpfsel1 = mmio_read(GPFSEL1);
    let gpfsel2 = mmio_read(GPFSEL2);

    // Expected values for all ALT2:
    // GPFSEL0: 0x36DB6DB6 (all 10 pins = 0b110)
    // GPFSEL1: 0x36DB6DB6 (all 10 pins = 0b110)
    // GPFSEL2: bits 0-5 should be 0b110110 = 0x36

    let expected_all_alt2: u32 = 0x36DB6DB6;

    gpfsel0 == expected_all_alt2 &&
        gpfsel1 == expected_all_alt2 &&
        (gpfsel2 & 0x3F) == 0x36
}

// ============================================================================
// LED Control (for debugging)
// ============================================================================

fn led_init() {
    // GPIO 29 is the ACT LED on Pi Zero 2 W
    // Set it to output (bits 27-29 of GPFSEL2)
    let sel = mmio_read(GPFSEL2);
    mmio_write(GPFSEL2, (sel & !(7 << 27)) | (GPIO_FUNC_OUTPUT << 27));
}

fn led_on() {
    // ACT LED is active low on Pi Zero 2 W
    mmio_write(GPCLR0, 1 << ACT_LED_PIN);
}

fn led_off() {
    mmio_write(GPSET0, 1 << ACT_LED_PIN);
}

fn led_blink(count: u32) {
    for _ in 0..count {
        led_on();
        delay(BLINK_DELAY);
        led_off();
        delay(BLINK_DELAY);
    }
    delay(BLINK_DELAY * 4);
}

/// Blink error code (rapid blinking, then pause, then number)
fn led_error_code(code: u32) {
    // 5 rapid blinks to indicate error
    for _ in 0..5 {
        led_on();
        delay(BLINK_DELAY / 4);
        led_off();
        delay(BLINK_DELAY / 4);
    }
    delay(BLINK_DELAY * 2);

    // Then blink the error code
    led_blink(code);
}

// ============================================================================
// Mailbox Communication
// ============================================================================

#[repr(C, align(16))]
struct MailboxBuffer {
    data: [u32; 36],
}

static mut MBOX: MailboxBuffer = MailboxBuffer { data: [0; 36] };

fn mailbox_call(channel: u8) -> bool {
    unsafe {
        let addr = core::ptr::addr_of!(MBOX) as u32;

        // Wait for mailbox to be ready for write
        while (mmio_read(MAILBOX_STATUS) & MAILBOX_FULL) != 0 {
            core::hint::spin_loop();
        }

        // Write address + channel
        mmio_write(MAILBOX_WRITE, (addr & 0xFFFFFFF0) | (channel as u32));

        // Wait for response
        loop {
            while (mmio_read(MAILBOX_STATUS) & MAILBOX_EMPTY) != 0 {
                core::hint::spin_loop();
            }
            let data = mmio_read(MAILBOX_READ);
            if (data & 0xF) == channel as u32 {
                // Check response code
                return MBOX.data[1] == 0x80000000;
            }
        }
    }
}

// ============================================================================
// Framebuffer State
// ============================================================================

static mut FB_ADDR: *mut u8 = core::ptr::null_mut();
static mut FB_WIDTH: u32 = 0;
static mut FB_HEIGHT: u32 = 0;
static mut FB_PITCH: u32 = 0;
static mut FB_DEPTH: u32 = 0;
static mut FB_SIZE: u32 = 0;

/// Initialize framebuffer for DPI display
///
/// CRITICAL: Must request 32bpp for DPI RGB666 padded format!
fn fb_init() -> bool {
    unsafe {
        // GPi Case 2W: 320x240 after rotation
        // DPI format 0x6016 = RGB666 padded = requires 32bpp
        let width: u32 = 320;
        let height: u32 = 240;
        let depth: u32 = 32;  // MUST be 32 for DPI RGB666!

        // Clear mailbox buffer
        for i in 0..36 {
            MBOX.data[i] = 0;
        }

        let mut i = 0;

        // Buffer header
        MBOX.data[i] = 35 * 4;  // Total buffer size
        i += 1;
        MBOX.data[i] = 0;       // Request code
        i += 1;

        // Tag: Set physical size
        MBOX.data[i] = TAG_FB_SET_PHYS_WH;
        i += 1;
        MBOX.data[i] = 8;       // Value buffer size
        i += 1;
        MBOX.data[i] = 8;       // Request size
        i += 1;
        MBOX.data[i] = width;   // [5]
        i += 1;
        MBOX.data[i] = height;  // [6]
        i += 1;

        // Tag: Set virtual size
        MBOX.data[i] = TAG_FB_SET_VIRT_WH;
        i += 1;
        MBOX.data[i] = 8;
        i += 1;
        MBOX.data[i] = 8;
        i += 1;
        MBOX.data[i] = width;   // [10]
        i += 1;
        MBOX.data[i] = height;  // [11]
        i += 1;

        // Tag: Set virtual offset
        MBOX.data[i] = TAG_FB_SET_VIRT_OFF;
        i += 1;
        MBOX.data[i] = 8;
        i += 1;
        MBOX.data[i] = 8;
        i += 1;
        MBOX.data[i] = 0;       // [15] X offset
        i += 1;
        MBOX.data[i] = 0;       // [16] Y offset
        i += 1;

        // Tag: Set depth (32bpp!)
        MBOX.data[i] = TAG_FB_SET_DEPTH;
        i += 1;
        MBOX.data[i] = 4;
        i += 1;
        MBOX.data[i] = 4;
        i += 1;
        MBOX.data[i] = depth;   // [20]
        i += 1;

        // Tag: Set pixel order (0=BGR, 1=RGB)
        // DPI format 0x6016 has BGR bit set, so use BGR
        MBOX.data[i] = TAG_FB_SET_PIXEL_ORDER;
        i += 1;
        MBOX.data[i] = 4;
        i += 1;
        MBOX.data[i] = 4;
        i += 1;
        MBOX.data[i] = 0;       // [24] BGR order
        i += 1;

        // Tag: Allocate framebuffer
        MBOX.data[i] = TAG_FB_ALLOC;
        i += 1;
        MBOX.data[i] = 8;
        i += 1;
        MBOX.data[i] = 8;
        i += 1;
        MBOX.data[i] = 4096;    // [28] Alignment
        i += 1;
        MBOX.data[i] = 0;       // [29] Size (response)
        i += 1;

        // Tag: Get pitch
        MBOX.data[i] = TAG_FB_GET_PITCH;
        i += 1;
        MBOX.data[i] = 4;
        i += 1;
        MBOX.data[i] = 4;
        i += 1;
        MBOX.data[i] = 0;       // [33] Pitch (response)
        i += 1;

        // End tag
        MBOX.data[i] = TAG_END;

        // Send mailbox request
        if !mailbox_call(8) {
            return false;
        }

        // Extract response values
        FB_WIDTH = MBOX.data[5];
        FB_HEIGHT = MBOX.data[6];
        FB_DEPTH = MBOX.data[20];

        let fb_bus_addr = MBOX.data[28];
        FB_SIZE = MBOX.data[29];

        if fb_bus_addr == 0 {
            return false;
        }

        // Convert bus address to ARM physical address
        // Bus address has 0xC0000000 prefix for cached access
        FB_ADDR = (fb_bus_addr & 0x3FFFFFFF) as *mut u8;

        // Get pitch from GPU
        FB_PITCH = MBOX.data[33];

        // Sanity check
        if FB_PITCH == 0 {
            // Calculate expected pitch
            FB_PITCH = FB_WIDTH * (FB_DEPTH / 8);
        }

        true
    }
}

// ============================================================================
// Drawing Functions (32bpp BGRA)
// ============================================================================

/// Put a pixel at (x, y) with 32-bit color
#[inline(always)]
fn fb_pixel(x: u32, y: u32, color: u32) {
    unsafe {
        if x >= FB_WIDTH || y >= FB_HEIGHT || FB_ADDR.is_null() {
            return;
        }

        // Calculate offset using ACTUAL pitch from GPU
        // pitch is in bytes, 4 bytes per pixel for 32bpp
        let offset = (y * FB_PITCH) + (x * 4);
        let ptr = FB_ADDR.add(offset as usize) as *mut u32;
        write_volatile(ptr, color);
    }
}

/// Create BGRA color from RGB components
/// Format: 0xAABBGGRR (little-endian)
#[inline(always)]
const fn bgra(r: u8, g: u8, b: u8) -> u32 {
    (b as u32) << 16 | (g as u32) << 8 | (r as u32) | 0xFF000000
}

/// Clear entire screen
fn fb_clear(color: u32) {
    unsafe {
        for y in 0..FB_HEIGHT {
            for x in 0..FB_WIDTH {
                fb_pixel(x, y, color);
            }
        }
    }
}

/// Fill a rectangle
fn fb_fill_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
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

// ============================================================================
// Test Pattern
// ============================================================================

fn draw_test_pattern() {
    unsafe {
        let w = FB_WIDTH;
        let h = FB_HEIGHT;

        // Clear to dark blue
        fb_clear(bgra(0, 0, 64));

        // Draw cyan border (5 pixels thick for visibility)
        let cyan = bgra(0, 255, 255);
        for i in 0..5 {
            // Top and bottom
            for x in 0..w {
                fb_pixel(x, i, cyan);
                fb_pixel(x, h - 1 - i, cyan);
            }
            // Left and right
            for y in 0..h {
                fb_pixel(i, y, cyan);
                fb_pixel(w - 1 - i, y, cyan);
            }
        }

        // Color bars
        let bar_height = 25u32;
        let bar_x = 15u32;
        let bar_width = w - 30;
        let colors = [
            bgra(255, 0, 0),     // Red
            bgra(0, 255, 0),     // Green
            bgra(0, 0, 255),     // Blue
            bgra(255, 255, 0),   // Yellow
            bgra(255, 0, 255),   // Magenta
            bgra(0, 255, 255),   // Cyan
            bgra(255, 255, 255), // White
        ];

        for (i, &color) in colors.iter().enumerate() {
            let y = 25 + i as u32 * bar_height;
            fb_fill_rect(bar_x, y, bar_width, bar_height - 3, color);
        }

        // Corner markers (helps verify full screen coverage)
        let corner_size = 25u32;

        // Top-left: Red
        fb_fill_rect(8, 8, corner_size, corner_size, bgra(255, 0, 0));
        // Top-right: Green
        fb_fill_rect(w - 8 - corner_size, 8, corner_size, corner_size, bgra(0, 255, 0));
        // Bottom-left: Blue
        fb_fill_rect(8, h - 8 - corner_size, corner_size, corner_size, bgra(0, 0, 255));
        // Bottom-right: Yellow
        fb_fill_rect(w - 8 - corner_size, h - 8 - corner_size, corner_size, corner_size, bgra(255, 255, 0));

        // Diagonal line (verifies aspect ratio and pitch)
        let white = bgra(255, 255, 255);
        let diag_len = w.min(h);
        for i in 0..diag_len {
            fb_pixel(i, i, white);
            if i + 1 < w {
                fb_pixel(i + 1, i, white);
            }
        }
    }
}

// ============================================================================
// Kernel Entry Point
// ============================================================================

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    // Step 1: Initialize LED first (for debug output)
    led_init();
    led_blink(1);  // 1 blink = kernel started

    // Step 2: Configure GPIO for DPI
    // This is CRITICAL - must happen before framebuffer init
    configure_gpio_for_dpi();

    // Verify GPIO configuration
    if !verify_gpio_config() {
        led_error_code(1);  // Error 1 = GPIO config failed
        loop { core::hint::spin_loop(); }
    }

    led_blink(2);  // 2 blinks = GPIO configured

    // Step 3: Initialize framebuffer
    if !fb_init() {
        led_error_code(2);  // Error 2 = framebuffer init failed
        loop { core::hint::spin_loop(); }
    }

    led_blink(3);  // 3 blinks = framebuffer initialized

    // Step 4: Draw test pattern
    draw_test_pattern();

    led_blink(4);  // 4 blinks = test pattern drawn

    // Keep LED on to show we're running
    led_on();

    // Done - spin forever
    loop {
        core::hint::spin_loop();
    }
}

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // Very rapid blinking indicates panic
    loop {
        led_on();
        delay(30000);
        led_off();
        delay(30000);
    }
}

// ============================================================================
// PADDING - To meet minimum kernel size requirement
// ============================================================================

#[used]
#[link_section = ".rodata.padding"]
static PADDING: [u8; 131072] = [0xAA; 131072];  // 128KB padding
