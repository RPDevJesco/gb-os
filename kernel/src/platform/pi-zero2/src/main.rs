//! Raspberry Pi Zero 2 W Bare-Metal Kernel
//!
//! Entry point for the Pi Zero 2 W (BCM2710, Cortex-A53).
//! The GPU firmware loads this as kernel8.img at 0x80000.

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// Boot Assembly (must be first in binary - .text.boot section)
// ============================================================================

global_asm!(
    r#"
.section .text.boot

.global _start

_start:
    // Read core ID from MPIDR_EL1
    mrs     x0, mpidr_el1
    and     x0, x0, #0xFF
    
    // If not core 0, park it
    cbz     x0, core0_boot
    
park_loop:
    wfe                         // Wait for event (low power)
    b       park_loop

core0_boot:
    // Set up stack pointer (grows down from 0x80000)
    ldr     x0, =_start
    mov     sp, x0
    
    // Clear BSS section
    ldr     x0, =__bss_start
    ldr     x1, =__bss_size
    cbz     x1, bss_done
    
bss_clear:
    str     xzr, [x0], #8
    subs    x1, x1, #1
    bne     bss_clear
    
bss_done:
    // Jump to Rust kernel_main
    bl      kernel_main
    
    // If kernel_main returns, halt
halt:
    wfe
    b       halt
"#
);

// ============================================================================
// Hardware Constants (BCM2710 - Pi Zero 2 W)
// ============================================================================

/// BCM2710 Peripheral Base Address
const PERIPHERAL_BASE: usize = 0x3F000000;

/// GPIO Registers
const GPIO_BASE: usize = PERIPHERAL_BASE + 0x200000;
const GPFSEL2: usize = GPIO_BASE + 0x08;
const GPSET0: usize = GPIO_BASE + 0x1C;
const GPCLR0: usize = GPIO_BASE + 0x28;

/// Mailbox Registers
const MAILBOX_BASE: usize = PERIPHERAL_BASE + 0xB880;
const MAILBOX_READ: usize = MAILBOX_BASE + 0x00;
const MAILBOX_STATUS: usize = MAILBOX_BASE + 0x18;
const MAILBOX_WRITE: usize = MAILBOX_BASE + 0x20;

const MAILBOX_FULL: u32 = 0x80000000;
const MAILBOX_EMPTY: u32 = 0x40000000;
const MAILBOX_CH_PROP: u8 = 8;

/// Framebuffer Tags
const TAG_FB_SET_PHYS_WH: u32 = 0x00048003;
const TAG_FB_SET_VIRT_WH: u32 = 0x00048004;
const TAG_FB_SET_VIRT_OFF: u32 = 0x00048009;
const TAG_FB_SET_DEPTH: u32 = 0x00048005;
const TAG_FB_ALLOC: u32 = 0x00040001;
const TAG_FB_GET_PITCH: u32 = 0x00040008;
const TAG_END: u32 = 0x00000000;

/// ACT LED is on GPIO 29 (active LOW on Pi Zero 2 W)
const ACT_LED_PIN: u32 = 29;

// ============================================================================
// MMIO Helpers
// ============================================================================

#[inline(always)]
fn mmio_read(addr: usize) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}

#[inline(always)]
fn mmio_write(addr: usize, value: u32) {
    unsafe { write_volatile(addr as *mut u32, value) }
}

#[inline(always)]
fn delay(count: u32) {
    for _ in 0..count {
        core::hint::spin_loop();
    }
}

// ============================================================================
// LED Control
// ============================================================================

fn led_init() {
    // GPIO 29 is in GPFSEL2 (pins 20-29), bits 27-29
    let sel = mmio_read(GPFSEL2);
    let sel = (sel & !(7 << 27)) | (1 << 27); // Set as output (001)
    mmio_write(GPFSEL2, sel);
}

fn led_on() {
    // Active LOW - clear to turn on
    mmio_write(GPCLR0, 1 << ACT_LED_PIN);
}

fn led_off() {
    // Active LOW - set to turn off
    mmio_write(GPSET0, 1 << ACT_LED_PIN);
}

fn led_blink(count: u32, delay_cycles: u32) {
    for _ in 0..count {
        led_on();
        delay(delay_cycles);
        led_off();
        delay(delay_cycles);
    }
}

// ============================================================================
// Mailbox
// ============================================================================

/// Mailbox buffer - 16-byte aligned for DMA
#[repr(C, align(16))]
struct MailboxBuffer {
    data: [u32; 36],
}

static mut MAILBOX_BUFFER: MailboxBuffer = MailboxBuffer { data: [0; 36] };

fn mailbox_call(channel: u8) -> bool {
    unsafe {
        let addr = core::ptr::addr_of!(MAILBOX_BUFFER) as u32;
        
        // Wait until mailbox is not full
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
                return MAILBOX_BUFFER.data[1] == 0x80000000;
            }
        }
    }
}

// ============================================================================
// Framebuffer
// ============================================================================

struct Framebuffer {
    width: u32,
    height: u32,
    pitch: u32,
    buffer: *mut u8,
    #[allow(dead_code)]
    size: u32,
}

static mut FB_INFO: Framebuffer = Framebuffer {
    width: 0,
    height: 0,
    pitch: 0,
    buffer: core::ptr::null_mut(),
    size: 0,
};

fn fb_init(width: u32, height: u32, depth: u32) -> bool {
    unsafe {
        let mut i = 0usize;
        
        MAILBOX_BUFFER.data[i] = 0; i += 1;  // Size (fill later)
        MAILBOX_BUFFER.data[i] = 0; i += 1;  // Request code
        
        // Set physical display size
        MAILBOX_BUFFER.data[i] = TAG_FB_SET_PHYS_WH; i += 1;
        MAILBOX_BUFFER.data[i] = 8; i += 1;   // Value buffer size
        MAILBOX_BUFFER.data[i] = 0; i += 1;   // Request/response code
        MAILBOX_BUFFER.data[i] = width; i += 1;
        MAILBOX_BUFFER.data[i] = height; i += 1;
        
        // Set virtual display size (same as physical)
        MAILBOX_BUFFER.data[i] = TAG_FB_SET_VIRT_WH; i += 1;
        MAILBOX_BUFFER.data[i] = 8; i += 1;
        MAILBOX_BUFFER.data[i] = 0; i += 1;
        MAILBOX_BUFFER.data[i] = width; i += 1;
        MAILBOX_BUFFER.data[i] = height; i += 1;
        
        // Set virtual offset to 0,0
        MAILBOX_BUFFER.data[i] = TAG_FB_SET_VIRT_OFF; i += 1;
        MAILBOX_BUFFER.data[i] = 8; i += 1;
        MAILBOX_BUFFER.data[i] = 0; i += 1;
        MAILBOX_BUFFER.data[i] = 0; i += 1;  // X offset
        MAILBOX_BUFFER.data[i] = 0; i += 1;  // Y offset
        
        // Set color depth
        MAILBOX_BUFFER.data[i] = TAG_FB_SET_DEPTH; i += 1;
        MAILBOX_BUFFER.data[i] = 4; i += 1;
        MAILBOX_BUFFER.data[i] = 0; i += 1;
        MAILBOX_BUFFER.data[i] = depth; i += 1;
        
        // Allocate framebuffer
        MAILBOX_BUFFER.data[i] = TAG_FB_ALLOC; i += 1;
        MAILBOX_BUFFER.data[i] = 8; i += 1;
        MAILBOX_BUFFER.data[i] = 0; i += 1;
        MAILBOX_BUFFER.data[i] = 16; i += 1;  // Alignment
        MAILBOX_BUFFER.data[i] = 0; i += 1;   // Size (response)
        
        // Get pitch
        MAILBOX_BUFFER.data[i] = TAG_FB_GET_PITCH; i += 1;
        MAILBOX_BUFFER.data[i] = 4; i += 1;
        MAILBOX_BUFFER.data[i] = 0; i += 1;
        MAILBOX_BUFFER.data[i] = 0; i += 1;   // Pitch (response)
        
        // End tag
        MAILBOX_BUFFER.data[i] = TAG_END; i += 1;
        
        // Set message size
        MAILBOX_BUFFER.data[0] = (i * 4) as u32;
        
        // Send to GPU
        if !mailbox_call(MAILBOX_CH_PROP) {
            return false;
        }
        
        // Check if we got a framebuffer (address at index 24)
        if MAILBOX_BUFFER.data[24] == 0 {
            return false;
        }
        
        // Extract framebuffer info
        FB_INFO.width = width;
        FB_INFO.height = height;
        // Convert bus address to ARM address (mask off upper bits)
        FB_INFO.buffer = (MAILBOX_BUFFER.data[24] & 0x3FFFFFFF) as *mut u8;
        FB_INFO.size = MAILBOX_BUFFER.data[25];
        FB_INFO.pitch = MAILBOX_BUFFER.data[29];
        
        true
    }
}

// ============================================================================
// Drawing
// ============================================================================

/// Color in BGRA format
#[derive(Clone, Copy)]
struct Color {
    b: u8,
    g: u8,
    r: u8,
    a: u8,
}

const COLOR_BLACK: Color = Color { b: 0, g: 0, r: 0, a: 255 };
const COLOR_GREEN: Color = Color { b: 0, g: 200, r: 0, a: 255 };

fn fb_put_pixel(x: u32, y: u32, color: Color) {
    unsafe {
        if x >= FB_INFO.width || y >= FB_INFO.height {
            return;
        }
        
        let offset = (y * FB_INFO.pitch + x * 4) as isize;
        let ptr = FB_INFO.buffer.offset(offset);
        
        *ptr.offset(0) = color.b;
        *ptr.offset(1) = color.g;
        *ptr.offset(2) = color.r;
        *ptr.offset(3) = color.a;
    }
}

fn fb_clear(color: Color) {
    unsafe {
        for y in 0..FB_INFO.height {
            for x in 0..FB_INFO.width {
                fb_put_pixel(x, y, color);
            }
        }
    }
}

fn fb_fill_rect(x: u32, y: u32, w: u32, h: u32, color: Color) {
    for py in y..(y + h) {
        for px in x..(x + w) {
            fb_put_pixel(px, py, color);
        }
    }
}

// Simple 8x8 font (just digits for demo)
static FONT_0: [u8; 8] = [0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00];
static FONT_1: [u8; 8] = [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00];
static FONT_2: [u8; 8] = [0x3C, 0x66, 0x06, 0x0C, 0x18, 0x30, 0x7E, 0x00];

fn fb_draw_char(x: u32, y: u32, c: char, fg: Color, bg: Color) {
    let glyph = match c {
        '0' => &FONT_0,
        '1' => &FONT_1,
        '2' => &FONT_2,
        _ => &FONT_0,
    };
    
    for row in 0..8u32 {
        let bits = glyph[row as usize];
        for col in 0..8u32 {
            let pixel = if (bits & (0x80 >> col)) != 0 { fg } else { bg };
            fb_put_pixel(x + col, y + row, pixel);
        }
    }
}

// ============================================================================
// Main Kernel Entry Point
// ============================================================================

const BLINK_DELAY: u32 = 500000;

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    // Initialize LED for debugging
    led_init();
    
    // Blink 1: Kernel started
    led_blink(1, BLINK_DELAY);
    delay(BLINK_DELAY * 2);
    
    // Initialize framebuffer (1280x720 @ 32bpp)
    if !fb_init(1280, 720, 32) {
        // FB failed - blink rapidly forever
        loop {
            led_blink(5, BLINK_DELAY / 5);
            delay(BLINK_DELAY * 2);
        }
    }
    
    // Blink 2: Framebuffer initialized
    led_blink(2, BLINK_DELAY);
    delay(BLINK_DELAY * 2);
    
    // Clear screen to black
    fb_clear(COLOR_BLACK);
    
    // Blink 3: Screen cleared
    led_blink(3, BLINK_DELAY);
    
    // Draw a green border
    unsafe {
        for x in 0..FB_INFO.width {
            fb_put_pixel(x, 0, COLOR_GREEN);
            fb_put_pixel(x, FB_INFO.height - 1, COLOR_GREEN);
        }
        for y in 0..FB_INFO.height {
            fb_put_pixel(0, y, COLOR_GREEN);
            fb_put_pixel(FB_INFO.width - 1, y, COLOR_GREEN);
        }
    }
    
    // Draw a title box
    fb_fill_rect(40, 40, 600, 50, COLOR_GREEN);
    
    // Draw some test characters
    fb_draw_char(60, 60, '0', COLOR_BLACK, COLOR_GREEN);
    fb_draw_char(70, 60, '1', COLOR_BLACK, COLOR_GREEN);
    fb_draw_char(80, 60, '2', COLOR_BLACK, COLOR_GREEN);
    
    // Success - slow heartbeat blink
    loop {
        led_on();
        delay(BLINK_DELAY / 2);
        led_off();
        delay(BLINK_DELAY * 4);
    }
}

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Rapid LED blink on panic
    led_init();
    loop {
        led_blink(10, BLINK_DELAY / 10);
        delay(BLINK_DELAY);
    }
}
