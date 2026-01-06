//! GB-OS for Pi Zero 2W / GPi Case 2W
//!
//! A bare-metal GameBoy emulator that boots directly on Raspberry Pi Zero 2W.
//! This integrates:
//! - USB HID input via DWC2 controller (GPi Case 2W gamepad)
//! - SD card reading via SDHOST controller
//! - FAT32 filesystem for ROM loading
//! - ROM browser UI
//! - GameBoy Color emulator (from kernel::gameboy)
//! - DPI display output (640x480 ARGB)

#![allow(dead_code)]
#![allow(unused_variables)]

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use alloc::vec::Vec;
use alloc::alloc as heap_alloc;
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use core::fmt::Write;
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

// Import the real GameBoy emulator from kernel crate
use kernel::gameboy::{Device, KeypadKey, gbmode::GbMode};

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
const GPLEV0: usize = GPIO_BASE + 0x34;
const GPSET0: usize = GPIO_BASE + 0x1C;
const GPCLR0: usize = GPIO_BASE + 0x28;
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

// System Timer
const SYSTIMER_BASE: usize = PERIPHERAL_BASE + 0x0000_3000;
const SYSTIMER_CLO: usize = SYSTIMER_BASE + 0x04;

// SDHOST Controller
const SDHOST_BASE: usize = PERIPHERAL_BASE + 0x0020_2000;
const SDHOST_CMD: usize = SDHOST_BASE + 0x00;
const SDHOST_ARG: usize = SDHOST_BASE + 0x04;
const SDHOST_TOUT: usize = SDHOST_BASE + 0x08;
const SDHOST_CDIV: usize = SDHOST_BASE + 0x0C;
const SDHOST_RSP0: usize = SDHOST_BASE + 0x10;
const SDHOST_HSTS: usize = SDHOST_BASE + 0x20;
const SDHOST_VDD: usize = SDHOST_BASE + 0x30;
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

// DWC2 USB Controller
const USB_BASE: usize = PERIPHERAL_BASE + 0x0098_0000;
const USB_GOTGCTL: usize = USB_BASE + 0x000;
const USB_GSNPSID: usize = USB_BASE + 0x040;
const USB_GAHBCFG: usize = USB_BASE + 0x008;
const USB_GUSBCFG: usize = USB_BASE + 0x00C;
const USB_GRSTCTL: usize = USB_BASE + 0x010;
const USB_GINTSTS: usize = USB_BASE + 0x014;
const USB_GINTMSK: usize = USB_BASE + 0x018;
const USB_GRXSTSR: usize = USB_BASE + 0x01C;
const USB_GRXSTSP: usize = USB_BASE + 0x020;
const USB_GRXFSIZ: usize = USB_BASE + 0x024;
const USB_GNPTXFSIZ: usize = USB_BASE + 0x028;
const USB_GNPTXSTS: usize = USB_BASE + 0x02C;
const USB_HPTXFSIZ: usize = USB_BASE + 0x100;
const USB_HCFG: usize = USB_BASE + 0x400;
const USB_HFIR: usize = USB_BASE + 0x404;
const USB_HFNUM: usize = USB_BASE + 0x408;
const USB_HAINT: usize = USB_BASE + 0x414;
const USB_HAINTMSK: usize = USB_BASE + 0x418;
const USB_HPRT: usize = USB_BASE + 0x440;
const USB_HCCHAR0: usize = USB_BASE + 0x500;
const USB_HCSPLT0: usize = USB_BASE + 0x504;
const USB_HCINT0: usize = USB_BASE + 0x508;
const USB_HCINTMSK0: usize = USB_BASE + 0x50C;
const USB_HCTSIZ0: usize = USB_BASE + 0x510;
const USB_HC_STRIDE: usize = 0x20;
const USB_PCGCCTL: usize = USB_BASE + 0xE00;
const USB_FIFO0: usize = USB_BASE + 0x1000;

// DWC2 Register Bits
const GAHBCFG_GLBL_INTR_EN: u32 = 1 << 0;
const GUSBCFG_PHYSEL: u32 = 1 << 6;
const GUSBCFG_FORCE_HOST: u32 = 1 << 29;
const GUSBCFG_FORCE_DEV: u32 = 1 << 30;
const GRSTCTL_CSRST: u32 = 1 << 0;
const GRSTCTL_RXFFLSH: u32 = 1 << 4;
const GRSTCTL_TXFFLSH: u32 = 1 << 5;
const GRSTCTL_TXFNUM_ALL: u32 = 0x10 << 6;
const GRSTCTL_AHB_IDLE: u32 = 1 << 31;
const GINTSTS_CURMOD: u32 = 1 << 0;
const GINTSTS_SOF: u32 = 1 << 3;
const GINTSTS_RXFLVL: u32 = 1 << 4;
const GINTSTS_HPRTINT: u32 = 1 << 24;
const GINTSTS_HCINT: u32 = 1 << 25;
const HPRT_CONN_STS: u32 = 1 << 0;
const HPRT_CONN_DET: u32 = 1 << 1;
const HPRT_ENA: u32 = 1 << 2;
const HPRT_ENA_CHNG: u32 = 1 << 3;
const HPRT_OVRCUR_CHNG: u32 = 1 << 5;
const HPRT_RST: u32 = 1 << 8;
const HPRT_PWR: u32 = 1 << 12;
const HPRT_SPD_SHIFT: u32 = 17;
const HPRT_SPD_MASK: u32 = 0x3 << 17;
const HPRT_W1C_MASK: u32 = HPRT_CONN_DET | HPRT_ENA | HPRT_ENA_CHNG | HPRT_OVRCUR_CHNG;
const HCCHAR_MPS_MASK: u32 = 0x7FF;
const HCCHAR_EPNUM_SHIFT: u32 = 11;
const HCCHAR_EPDIR_IN: u32 = 1 << 15;
const HCCHAR_LSDEV: u32 = 1 << 17;
const HCCHAR_EPTYPE_CTRL: u32 = 0 << 18;
const HCCHAR_EPTYPE_INTR: u32 = 3 << 18;
const HCCHAR_MC_SHIFT: u32 = 20;
const HCCHAR_DEVADDR_SHIFT: u32 = 22;
const HCCHAR_ODDFRM: u32 = 1 << 29;
const HCCHAR_CHDIS: u32 = 1 << 30;
const HCCHAR_CHEN: u32 = 1 << 31;
const HCTSIZ_XFERSIZE_SHIFT: u32 = 0;
const HCTSIZ_PKTCNT_SHIFT: u32 = 19;
const HCTSIZ_PID_DATA0: u32 = 0 << 29;
const HCTSIZ_PID_DATA1: u32 = 2 << 29;
const HCTSIZ_PID_SETUP: u32 = 3 << 29;
const HCINT_XFERCOMP: u32 = 1 << 0;
const HCINT_CHHLT: u32 = 1 << 1;
const HCINT_AHBERR: u32 = 1 << 2;
const HCINT_STALL: u32 = 1 << 3;
const HCINT_NAK: u32 = 1 << 4;
const HCINT_ACK: u32 = 1 << 5;
const HCINT_XACTERR: u32 = 1 << 7;
const HCINT_BBLERR: u32 = 1 << 8;
const HCINT_DATATGLERR: u32 = 1 << 10;
const HCINT_ERROR_MASK: u32 = HCINT_AHBERR | HCINT_STALL | HCINT_XACTERR | HCINT_BBLERR;

// USB Protocol
const USB_REQ_SET_ADDRESS: u8 = 0x05;
const USB_REQ_GET_DESCRIPTOR: u8 = 0x06;
const USB_REQ_SET_CONFIGURATION: u8 = 0x09;
const USB_DESC_DEVICE: u8 = 0x01;
const USB_DESC_CONFIGURATION: u8 = 0x02;
const USB_DESC_ENDPOINT: u8 = 0x05;
const USB_REQTYPE_DIR_IN: u8 = 0x80;
const USB_REQTYPE_TYPE_STANDARD: u8 = 0x00;
const USB_REQTYPE_RECIP_DEVICE: u8 = 0x00;

// Display
const SCREEN_WIDTH: u32 = 640;
const SCREEN_HEIGHT: u32 = 480;

// GameBoy
const GB_WIDTH: usize = 160;
const GB_HEIGHT: usize = 144;
const GB_SCALE: usize = 2;
const GB_SCALED_W: usize = GB_WIDTH * GB_SCALE;
const GB_SCALED_H: usize = GB_HEIGHT * GB_SCALE;
const GB_OFFSET_X: usize = (SCREEN_WIDTH as usize - GB_SCALED_W) / 2;
const GB_OFFSET_Y: usize = (SCREEN_HEIGHT as usize - GB_SCALED_H) / 2;

// Emulator timing
const FRAME_TIME_US: u32 = 16742;

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

// GameBoy DMG palette
const GB_PALETTE: [u32; 4] = [
    0xFFE0F8D0,
    0xFF88C070,
    0xFF346856,
    0xFF081820,
];

// ============================================================================
// Global Allocator
// ============================================================================

extern "C" {
    static __heap_start: u8;
    static __heap_end: u8;
}

struct BumpAllocator {
    // Use UnsafeCell instead of AtomicUsize - no atomics needed on bare metal single-thread
    next: core::cell::UnsafeCell<usize>,
}

unsafe impl Sync for BumpAllocator {}

impl BumpAllocator {
    const fn new() -> Self {
        Self {
            next: core::cell::UnsafeCell::new(0),
        }
    }

    fn init(&self) {
        let start = unsafe { &__heap_start as *const u8 as usize };
        unsafe { core::ptr::write_volatile(self.next.get(), start); }
    }

    pub fn heap_end(&self) -> usize {
        unsafe { &__heap_end as *const u8 as usize }
    }

    pub fn current_pos(&self) -> usize {
        unsafe { core::ptr::read_volatile(self.next.get()) }
    }
}

// Debug: global flag to show alloc debug on screen
static ALLOC_DEBUG_ENABLED: AtomicUsize = AtomicUsize::new(0);
static ALLOC_DEBUG_Y: AtomicUsize = AtomicUsize::new(0);
static ALLOC_DEBUG_FB_ADDR: AtomicUsize = AtomicUsize::new(0);

fn enable_alloc_debug(fb_addr: usize, start_y: usize) {
    ALLOC_DEBUG_FB_ADDR.store(fb_addr, Ordering::Relaxed);
    ALLOC_DEBUG_Y.store(start_y, Ordering::Relaxed);
    ALLOC_DEBUG_ENABLED.store(1, Ordering::Relaxed);
}

fn disable_alloc_debug() {
    ALLOC_DEBUG_ENABLED.store(0, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();

        // DEBUG: Green pixel at (500, 100) = entered alloc
        let fb_addr = ALLOC_DEBUG_FB_ADDR.load(Ordering::Relaxed);
        if fb_addr != 0 {
            let pitch = 640 * 4;
            let offset = (100 * pitch + 500 * 4) as usize;
            core::ptr::write_volatile((fb_addr + offset) as *mut u32, 0xFF00FF00);
        }

        // Blue = about to read next
        if fb_addr != 0 {
            let pitch = 640 * 4;
            let offset = (110 * pitch + 500 * 4) as usize;
            core::ptr::write_volatile((fb_addr + offset) as *mut u32, 0xFF0000FF);
        }

        // Simple volatile read (no atomics!)
        let current = core::ptr::read_volatile(self.next.get());

        // Cyan = read next OK
        if fb_addr != 0 {
            let pitch = 640 * 4;
            let offset = (110 * pitch + 510 * 4) as usize;
            core::ptr::write_volatile((fb_addr + offset) as *mut u32, 0xFF00FFFF);
        }

        let aligned = (current + align - 1) & !(align - 1);

        // White = aligned OK
        if fb_addr != 0 {
            let pitch = 640 * 4;
            let offset = (110 * pitch + 520 * 4) as usize;
            core::ptr::write_volatile((fb_addr + offset) as *mut u32, 0xFFFFFFFF);
        }

        let new_next = aligned + size;

        // Orange = new_next OK
        if fb_addr != 0 {
            let pitch = 640 * 4;
            let offset = (110 * pitch + 530 * 4) as usize;
            core::ptr::write_volatile((fb_addr + offset) as *mut u32, 0xFFFF8000);
        }

        let heap_end = self.heap_end();

        // Magenta = heap_end OK
        if fb_addr != 0 {
            let pitch = 640 * 4;
            let offset = (110 * pitch + 540 * 4) as usize;
            core::ptr::write_volatile((fb_addr + offset) as *mut u32, 0xFFFF00FF);
        }

        if new_next > heap_end {
            // OOM - Red bar
            if fb_addr != 0 {
                let pitch = 640 * 4;
                for i in 0..100u32 {
                    let offset = (120 * pitch + (500 + i) * 4) as usize;
                    core::ptr::write_volatile((fb_addr + offset) as *mut u32, 0xFFFF0000);
                }
            }
            return core::ptr::null_mut();
        }

        // Simple volatile write (no CAS!)
        core::ptr::write_volatile(self.next.get(), new_next);

        // Green at 550 = success
        if fb_addr != 0 {
            let pitch = 640 * 4;
            let offset = (110 * pitch + 550 * 4) as usize;
            core::ptr::write_volatile((fb_addr + offset) as *mut u32, 0xFF00FF00);
        }

        aligned as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't free
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = self.alloc(layout);
        if !ptr.is_null() {
            core::ptr::write_bytes(ptr, 0, layout.size());
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // For bump allocator, just allocate new space and copy
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let new_ptr = self.alloc(new_layout);
        if !new_ptr.is_null() && layout.size() > 0 {
            let copy_size = layout.size().min(new_size);
            core::ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new();

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    // Write big red rectangle across the screen to show alloc_error was triggered
    let fb_addr = ALLOC_DEBUG_FB_ADDR.load(Ordering::Relaxed);
    if fb_addr != 0 {
        unsafe {
            let pitch = 640 * 4;
            // Draw thick red bar from y=0 to y=30
            for y in 0u32..30 {
                for x in 0u32..640 {
                    let offset = (y * pitch + x * 4) as usize;
                    core::ptr::write_volatile((fb_addr + offset) as *mut u32, 0xFFFF0000);
                }
            }
        }
    }
    loop {
        unsafe { core::arch::asm!("wfe"); }
    }
}

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

#[inline]
fn dmb() {
    unsafe { core::arch::asm!("dmb sy"); }
}

fn micros() -> u32 {
    mmio_read(SYSTIMER_CLO)
}

fn delay_us(us: u32) {
    let start = micros();
    while micros().wrapping_sub(start) < us {
        core::hint::spin_loop();
    }
}

fn delay_ms(ms: u32) {
    delay_us(ms * 1000);
}

// ============================================================================
// GPIO Configuration
// ============================================================================

fn configure_gpio_for_dpi() {
    const ALT2: u32 = 0b110;
    let gpfsel0_val: u32 = (ALT2 << 0) | (ALT2 << 3) | (ALT2 << 6) | (ALT2 << 9) |
        (ALT2 << 12) | (ALT2 << 15) | (ALT2 << 18) | (ALT2 << 21) |
        (ALT2 << 24) | (ALT2 << 27);
    let gpfsel1_val: u32 = (ALT2 << 0) | (ALT2 << 3) | (ALT2 << 6) | (ALT2 << 9) |
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

fn configure_gpio_for_sd() {
    const ALT0: u32 = 0b100;
    let gpfsel4 = mmio_read(GPFSEL4);
    let gpfsel4_new = (gpfsel4 & 0xC0FFFFFF) | (ALT0 << 24) | (ALT0 << 27);
    mmio_write(GPFSEL4, gpfsel4_new);
    let gpfsel5 = mmio_read(GPFSEL5);
    let gpfsel5_new = (gpfsel5 & 0xFFFFF000) | (ALT0 << 0) | (ALT0 << 3) | (ALT0 << 6) | (ALT0 << 9);
    mmio_write(GPFSEL5, gpfsel5_new);
    mmio_write(GPPUD, 2);
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
    while (mmio_read(MBOX_STATUS) & MBOX_FULL) != 0 { core::hint::spin_loop(); }
    mmio_write(MBOX_WRITE, (addr & !0xF) | (channel as u32 & 0xF));
    loop {
        while (mmio_read(MBOX_STATUS) & MBOX_EMPTY) != 0 { core::hint::spin_loop(); }
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

    fn blit_gb_screen_dmg(&self, pal_data: &[u8]) {
        for y in 0..GB_HEIGHT {
            for x in 0..GB_WIDTH {
                let pal_idx = pal_data[y * GB_WIDTH + x] as usize;
                let color = if pal_idx < 4 { GB_PALETTE[pal_idx] } else { BLACK };
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

    fn draw_gb_border(&self, color: u32) {
        let border = 4;
        let x = GB_OFFSET_X as u32 - border;
        let y = GB_OFFSET_Y as u32 - border;
        let w = GB_SCALED_W as u32 + border * 2;
        let h = GB_SCALED_H as u32 + border * 2;
        self.fill_rect(x, y, w, border, color);
        self.fill_rect(x, y + h - border, w, border, color);
        self.fill_rect(x, y, border, h, color);
        self.fill_rect(x + w - border, y, border, h, color);
    }
}

// ============================================================================
// Font and Text
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
        delay_ms(10);
        mmio_write(SDHOST_HCFG, SDHOST_HCFG_SLOW_CARD | SDHOST_HCFG_INTBUS);
        mmio_write(SDHOST_CDIV, 0x148);
        delay_ms(10);
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

        mmio_write(SDHOST_ARG, 0);
        mmio_write(SDHOST_CMD, 0 | SDHOST_CMD_NO_RSP | SDHOST_CMD_NEW);
        delay_ms(50);
        self.clear_status();

        match self.send_cmd(8, 0x1AA, 0) {
            Ok(resp) => { self.is_sdhc = (resp & 0xFF) == 0xAA; }
            Err(_) => { self.is_sdhc = false; self.clear_status(); }
        }

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

        let mut sector = [0u8; 512];
        self.sd.read_sector(0, &mut sector)?;

        if sector[510] != 0x55 || sector[511] != 0xAA {
            return Err("Invalid MBR");
        }

        let part_start = u32::from_le_bytes([
            sector[0x1BE + 8], sector[0x1BE + 9],
            sector[0x1BE + 10], sector[0x1BE + 11],
        ]);

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
                            for j in 0..8 { name_buf[j] = sector[offset + j]; }
                            name_buf[8] = b'.';
                            for j in 0..3 { name_buf[9 + j] = sector[offset + 8 + j]; }
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
// USB HID Input Driver for GPi Case 2W
// ============================================================================

/// USB Setup Packet
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct UsbSetupPacket {
    bm_request_type: u8,
    b_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
}

impl UsbSetupPacket {
    const fn get_descriptor(desc_type: u8, desc_index: u8, length: u16) -> Self {
        Self {
            bm_request_type: USB_REQTYPE_DIR_IN | USB_REQTYPE_TYPE_STANDARD | USB_REQTYPE_RECIP_DEVICE,
            b_request: USB_REQ_GET_DESCRIPTOR,
            w_value: ((desc_type as u16) << 8) | (desc_index as u16),
            w_index: 0,
            w_length: length,
        }
    }

    const fn set_address(addr: u8) -> Self {
        Self {
            bm_request_type: USB_REQTYPE_TYPE_STANDARD | USB_REQTYPE_RECIP_DEVICE,
            b_request: USB_REQ_SET_ADDRESS,
            w_value: addr as u16,
            w_index: 0,
            w_length: 0,
        }
    }

    const fn set_configuration(config: u8) -> Self {
        Self {
            bm_request_type: USB_REQTYPE_TYPE_STANDARD | USB_REQTYPE_RECIP_DEVICE,
            b_request: USB_REQ_SET_CONFIGURATION,
            w_value: config as u16,
            w_index: 0,
            w_length: 0,
        }
    }
}

/// Xbox 360 Controller Input Report (20 bytes)
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct Xbox360InputReport {
    report_id: u8,
    report_length: u8,
    buttons_low: u8,
    buttons_high: u8,
    left_trigger: u8,
    right_trigger: u8,
    left_stick_x: i16,
    left_stick_y: i16,
    right_stick_x: i16,
    right_stick_y: i16,
    _reserved: [u8; 6],
}

impl Xbox360InputReport {
    const DPAD_UP: u8    = 1 << 0;
    const DPAD_DOWN: u8  = 1 << 1;
    const DPAD_LEFT: u8  = 1 << 2;
    const DPAD_RIGHT: u8 = 1 << 3;
    const START: u8      = 1 << 4;
    const BACK: u8       = 1 << 5;

    const LB: u8    = 1 << 0;
    const RB: u8    = 1 << 1;
    const GUIDE: u8 = 1 << 2;
    const A: u8     = 1 << 4;
    const B: u8     = 1 << 5;
    const X: u8     = 1 << 6;
    const Y: u8     = 1 << 7;
}

/// GPi Case 2W Button State
#[derive(Clone, Copy, Default)]
pub struct GpiButtonState {
    pub current: u16,
    pub previous: u16,
}

// Button bit positions
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

impl GpiButtonState {
    pub fn update_from_xbox(&mut self, report: &Xbox360InputReport) {
        self.previous = self.current;
        self.current = 0;

        if report.buttons_low & Xbox360InputReport::DPAD_UP != 0    { self.current |= BTN_UP; }
        if report.buttons_low & Xbox360InputReport::DPAD_DOWN != 0  { self.current |= BTN_DOWN; }
        if report.buttons_low & Xbox360InputReport::DPAD_LEFT != 0  { self.current |= BTN_LEFT; }
        if report.buttons_low & Xbox360InputReport::DPAD_RIGHT != 0 { self.current |= BTN_RIGHT; }
        if report.buttons_high & Xbox360InputReport::A != 0         { self.current |= BTN_A; }
        if report.buttons_high & Xbox360InputReport::B != 0         { self.current |= BTN_B; }
        if report.buttons_high & Xbox360InputReport::X != 0         { self.current |= BTN_X; }
        if report.buttons_high & Xbox360InputReport::Y != 0         { self.current |= BTN_Y; }
        if report.buttons_low & Xbox360InputReport::START != 0      { self.current |= BTN_START; }
        if report.buttons_low & Xbox360InputReport::BACK != 0       { self.current |= BTN_SELECT; }
        if report.buttons_high & Xbox360InputReport::LB != 0        { self.current |= BTN_L; }
        if report.buttons_high & Xbox360InputReport::RB != 0        { self.current |= BTN_R; }
        if report.buttons_high & Xbox360InputReport::GUIDE != 0     { self.current |= BTN_HOME; }
    }

    #[inline]
    pub fn is_pressed(&self, button: u16) -> bool {
        (self.current & button) != 0
    }

    #[inline]
    pub fn just_pressed(&self, button: u16) -> bool {
        (self.current & button) != 0 && (self.previous & button) == 0
    }

    #[inline]
    pub fn just_released(&self, button: u16) -> bool {
        (self.current & button) == 0 && (self.previous & button) != 0
    }
}

/// Transfer result
#[derive(Clone, Copy)]
enum TransferResult {
    Success(usize),
    Nak,
    Stall,
    Error,
    Timeout,
}

/// USB Host Controller for DWC2
pub struct UsbHost {
    device_address: u8,
    ep0_max_packet: u16,
    hid_endpoint: u8,
    hid_max_packet: u16,
    hid_data_toggle: bool,
    enumerated: bool,
    port_speed: u8,
}

impl UsbHost {
    pub const fn new() -> Self {
        Self {
            device_address: 0,
            ep0_max_packet: 8,
            hid_endpoint: 0,
            hid_max_packet: 0,
            hid_data_toggle: false,
            enumerated: false,
            port_speed: 1,
        }
    }

    fn power_on(&self) -> bool {
        #[repr(C, align(16))]
        struct UsbMbox { data: [u32; 8] }
        static mut USB_MBOX: UsbMbox = UsbMbox { data: [0; 8] };

        let mbox = unsafe { &mut USB_MBOX.data };
        mbox[0] = 8 * 4; mbox[1] = 0;
        mbox[2] = 0x28001; mbox[3] = 8; mbox[4] = 8;
        mbox[5] = 3; mbox[6] = 3; mbox[7] = 0;

        dmb();
        let mbox_addr = mbox.as_ptr() as u32;
        let mbox_msg = (mbox_addr & !0xF) | 8;

        for _ in 0..10000 { if mmio_read(MBOX_STATUS) & MBOX_FULL == 0 { break; } delay_us(1); }
        mmio_write(MBOX_WRITE, mbox_msg);

        for _ in 0..100000 {
            if mmio_read(MBOX_STATUS) & MBOX_EMPTY == 0 {
                let response = mmio_read(MBOX_READ);
                if response == mbox_msg { return mbox[6] & 1 == 1; }
            }
            delay_us(10);
        }
        false
    }

    fn wait_for_sof(&self) {
        mmio_write(USB_GINTSTS, GINTSTS_SOF);
        for _ in 0..3000 {
            if mmio_read(USB_GINTSTS) & GINTSTS_SOF != 0 {
                mmio_write(USB_GINTSTS, GINTSTS_SOF);
                return;
            }
            delay_us(1);
        }
    }

    fn wait_tx_fifo(&self, words: u32) -> bool {
        for _ in 0..10000 {
            let txsts = mmio_read(USB_GNPTXSTS);
            if (txsts & 0xFFFF) >= words { return true; }
            delay_us(1);
        }
        false
    }

    fn disable_channel(&self, ch: usize) {
        let hcchar_addr = USB_HCCHAR0 + ch * USB_HC_STRIDE;
        let hcint_addr = USB_HCINT0 + ch * USB_HC_STRIDE;

        let hcchar = mmio_read(hcchar_addr);
        if hcchar & HCCHAR_CHEN != 0 {
            mmio_write(hcchar_addr, hcchar | HCCHAR_CHDIS);
            for _ in 0..10000 {
                if mmio_read(hcint_addr) & HCINT_CHHLT != 0 { break; }
                delay_us(1);
            }
        }
        mmio_write(hcint_addr, 0xFFFFFFFF);
    }

    pub fn init(&mut self) -> Result<(), &'static str> {
        self.power_on();
        delay_ms(50);

        let snpsid = mmio_read(USB_GSNPSID);
        if (snpsid & 0xFFFFF000) != 0x4F542000 {
            return Err("No DWC2");
        }

        mmio_write(USB_GINTMSK, 0);
        mmio_write(USB_GAHBCFG, 0);

        for _ in 0..100000 {
            if mmio_read(USB_GRSTCTL) & GRSTCTL_AHB_IDLE != 0 { break; }
            delay_us(1);
        }

        mmio_write(USB_GRSTCTL, GRSTCTL_CSRST);
        for _ in 0..100000 {
            if mmio_read(USB_GRSTCTL) & GRSTCTL_CSRST == 0 { break; }
            delay_us(1);
        }
        delay_ms(100);

        mmio_write(USB_PCGCCTL, 0);
        delay_ms(10);

        let gusbcfg = mmio_read(USB_GUSBCFG);
        mmio_write(USB_GUSBCFG, (gusbcfg & !GUSBCFG_FORCE_DEV) | GUSBCFG_FORCE_HOST | GUSBCFG_PHYSEL);
        delay_ms(50);

        for _ in 0..100000 {
            if mmio_read(USB_GINTSTS) & GINTSTS_CURMOD != 0 { break; }
            delay_us(1);
        }

        mmio_write(USB_GRXFSIZ, 512);
        mmio_write(USB_GNPTXFSIZ, (256 << 16) | 512);
        mmio_write(USB_HPTXFSIZ, (256 << 16) | 768);

        mmio_write(USB_GRSTCTL, GRSTCTL_TXFFLSH | GRSTCTL_TXFNUM_ALL);
        for _ in 0..10000 { if mmio_read(USB_GRSTCTL) & GRSTCTL_TXFFLSH == 0 { break; } delay_us(1); }
        mmio_write(USB_GRSTCTL, GRSTCTL_RXFFLSH);
        for _ in 0..10000 { if mmio_read(USB_GRSTCTL) & GRSTCTL_RXFFLSH == 0 { break; } delay_us(1); }

        mmio_write(USB_HCFG, 1);
        mmio_write(USB_HFIR, 48000);

        for ch in 0..8usize {
            self.disable_channel(ch);
            mmio_write(USB_HCINTMSK0 + ch * USB_HC_STRIDE,
                       HCINT_XFERCOMP | HCINT_CHHLT | HCINT_STALL | HCINT_NAK |
                           HCINT_ACK | HCINT_XACTERR | HCINT_BBLERR | HCINT_DATATGLERR);
        }

        mmio_write(USB_HAINTMSK, 0xFF);
        mmio_write(USB_GINTSTS, 0xFFFFFFFF);
        mmio_write(USB_GINTMSK, GINTSTS_SOF | GINTSTS_RXFLVL | GINTSTS_HPRTINT | GINTSTS_HCINT);
        mmio_write(USB_GAHBCFG, GAHBCFG_GLBL_INTR_EN);

        let hprt = mmio_read(USB_HPRT);
        mmio_write(USB_HPRT, (hprt & !HPRT_W1C_MASK) | HPRT_PWR);
        delay_ms(100);

        Ok(())
    }

    pub fn wait_for_connection(&self, timeout_ms: u32) -> bool {
        let start = micros();
        loop {
            if mmio_read(USB_HPRT) & HPRT_CONN_STS != 0 { return true; }
            if micros().wrapping_sub(start) > timeout_ms * 1000 { return false; }
            delay_ms(10);
        }
    }

    pub fn reset_port(&mut self) -> Result<(), &'static str> {
        let hprt = mmio_read(USB_HPRT);
        if hprt & HPRT_CONN_STS == 0 { return Err("No device"); }

        mmio_write(USB_HPRT, (hprt & !HPRT_ENA) | HPRT_CONN_DET | HPRT_ENA_CHNG | HPRT_OVRCUR_CHNG);
        delay_ms(10);

        let hprt = mmio_read(USB_HPRT);
        mmio_write(USB_HPRT, (hprt & !HPRT_W1C_MASK) | HPRT_RST);
        delay_ms(60);

        let hprt = mmio_read(USB_HPRT);
        mmio_write(USB_HPRT, hprt & !HPRT_W1C_MASK & !HPRT_RST);
        delay_ms(20);

        for _ in 0..50 {
            let hprt = mmio_read(USB_HPRT);
            if hprt & HPRT_ENA_CHNG != 0 {
                mmio_write(USB_HPRT, (hprt & !HPRT_ENA) | HPRT_ENA_CHNG);
            }
            if hprt & HPRT_ENA != 0 {
                self.port_speed = ((hprt & HPRT_SPD_MASK) >> HPRT_SPD_SHIFT) as u8;
                self.device_address = 0;
                self.ep0_max_packet = 8;
                self.enumerated = false;
                return Ok(());
            }
            delay_ms(10);
        }

        Err("Port enable timeout")
    }

    fn do_transfer(&mut self, ch: usize, ep: u8, is_in: bool, ep_type: u32,
                   pid: u32, buf: &mut [u8], len: usize) -> TransferResult {
        self.disable_channel(ch);

        if ep_type == HCCHAR_EPTYPE_CTRL {
            self.wait_for_sof();
        }

        let hcchar_addr = USB_HCCHAR0 + ch * USB_HC_STRIDE;
        let hctsiz_addr = USB_HCTSIZ0 + ch * USB_HC_STRIDE;
        let hcint_addr = USB_HCINT0 + ch * USB_HC_STRIDE;
        let hcsplt_addr = USB_HCSPLT0 + ch * USB_HC_STRIDE;
        let fifo_addr = USB_FIFO0 + ch * 0x1000;

        mmio_write(hcsplt_addr, 0);

        let max_pkt = if ep == 0 { self.ep0_max_packet } else { self.hid_max_packet };
        let dir_bit = if is_in { HCCHAR_EPDIR_IN } else { 0 };
        let ls_bit = if self.port_speed == 2 { HCCHAR_LSDEV } else { 0 };
        let frame = mmio_read(USB_HFNUM) & 1;
        let odd_frame = if frame != 0 { HCCHAR_ODDFRM } else { 0 };

        let hcchar = (max_pkt as u32 & HCCHAR_MPS_MASK)
            | ((ep as u32) << HCCHAR_EPNUM_SHIFT)
            | dir_bit | ls_bit | ep_type
            | (1 << HCCHAR_MC_SHIFT)
            | ((self.device_address as u32) << HCCHAR_DEVADDR_SHIFT)
            | odd_frame;

        let request_len = if is_in { max_pkt as usize } else { len.min(max_pkt as usize) };
        let hctsiz = ((request_len as u32) << HCTSIZ_XFERSIZE_SHIFT)
            | (1 << HCTSIZ_PKTCNT_SHIFT) | pid;

        mmio_write(hcint_addr, 0xFFFFFFFF);

        if !is_in && request_len > 0 {
            if !self.wait_tx_fifo(((request_len + 3) / 4) as u32) {
                return TransferResult::Error;
            }
        }

        mmio_write(hctsiz_addr, hctsiz);
        dmb();
        mmio_write(hcchar_addr, hcchar | HCCHAR_CHEN);
        dmb();

        if !is_in && request_len > 0 {
            let words = (request_len + 3) / 4;
            for i in 0..words {
                let start = i * 4;
                let mut word = 0u32;
                for j in 0..4 {
                    if start + j < len { word |= (buf[start + j] as u32) << (j * 8); }
                }
                mmio_write(fifo_addr, word);
            }
            dmb();
        }

        let mut received = 0usize;
        let timeout_us = if ep_type == HCCHAR_EPTYPE_CTRL { 500_000 } else { 5_000 };
        let start = micros();

        loop {
            if is_in {
                while mmio_read(USB_GINTSTS) & GINTSTS_RXFLVL != 0 {
                    let rxsts = mmio_read(USB_GRXSTSR);
                    let rx_ch = (rxsts & 0xF) as usize;
                    if rx_ch != ch { let _ = mmio_read(USB_GRXSTSP); continue; }

                    let rxsts = mmio_read(USB_GRXSTSP);
                    let byte_count = ((rxsts >> 4) & 0x7FF) as usize;
                    let pkt_status = ((rxsts >> 17) & 0xF) as u8;

                    if pkt_status == 2 && byte_count > 0 {
                        let words = (byte_count + 3) / 4;
                        for i in 0..words {
                            let word = mmio_read(fifo_addr);
                            for j in 0..4 {
                                let idx = received + i * 4 + j;
                                if idx < buf.len() && (i * 4 + j) < byte_count {
                                    buf[idx] = ((word >> (j * 8)) & 0xFF) as u8;
                                }
                            }
                        }
                        received += byte_count;
                    }
                    if pkt_status == 3 || pkt_status == 7 { break; }
                }
            }

            let hcint = mmio_read(hcint_addr);

            if hcint & HCINT_XFERCOMP != 0 {
                mmio_write(hcint_addr, 0xFFFFFFFF);
                return TransferResult::Success(if is_in { received } else { request_len });
            }

            if hcint & HCINT_CHHLT != 0 {
                mmio_write(hcint_addr, 0xFFFFFFFF);
                if is_in && received > 0 && (hcint & HCINT_ERROR_MASK) == 0 {
                    return TransferResult::Success(received);
                }
                if hcint & HCINT_STALL != 0 { return TransferResult::Stall; }
                if hcint & HCINT_NAK != 0 { return TransferResult::Nak; }
                if (hcint & HCINT_ACK != 0) && is_in && received > 0 {
                    return TransferResult::Success(received);
                }
                return TransferResult::Error;
            }

            if micros().wrapping_sub(start) > timeout_us {
                self.disable_channel(ch);
                if is_in && received > 0 { return TransferResult::Success(received); }
                return TransferResult::Timeout;
            }

            delay_us(1);
        }
    }

    fn control_transfer(&mut self, setup: &UsbSetupPacket, data: Option<&mut [u8]>) -> Result<usize, &'static str> {
        const CH: usize = 0;
        const MAX_RETRIES: u32 = 50;

        let setup_bytes = unsafe { core::slice::from_raw_parts(setup as *const _ as *const u8, 8) };
        let mut setup_buf = [0u8; 8];
        setup_buf.copy_from_slice(setup_bytes);

        for _ in 0..MAX_RETRIES {
            match self.do_transfer(CH, 0, false, HCCHAR_EPTYPE_CTRL, HCTSIZ_PID_SETUP, &mut setup_buf, 8) {
                TransferResult::Success(_) => break,
                TransferResult::Nak => { delay_ms(1); continue; }
                _ => return Err("SETUP failed"),
            }
        }

        let mut transferred = 0usize;

        if let Some(buf) = data {
            if !buf.is_empty() && setup.w_length > 0 {
                let is_in = (setup.bm_request_type & USB_REQTYPE_DIR_IN) != 0;
                let mut data_toggle = HCTSIZ_PID_DATA1;
                let mut offset = 0usize;
                let total_len = (setup.w_length as usize).min(buf.len());

                while offset < total_len {
                    let chunk_len = (total_len - offset).min(self.ep0_max_packet as usize);

                    for _ in 0..MAX_RETRIES {
                        let result = self.do_transfer(CH, 0, is_in, HCCHAR_EPTYPE_CTRL,
                                                      data_toggle, &mut buf[offset..offset + chunk_len], chunk_len);

                        match result {
                            TransferResult::Success(n) => {
                                offset += n;
                                transferred = offset;
                                data_toggle = if data_toggle == HCTSIZ_PID_DATA1 { HCTSIZ_PID_DATA0 } else { HCTSIZ_PID_DATA1 };
                                if n < self.ep0_max_packet as usize { offset = total_len; }
                                break;
                            }
                            TransferResult::Nak => { delay_ms(1); continue; }
                            _ => return Err("DATA failed"),
                        }
                    }
                }
            }
        }

        let status_in = setup.w_length == 0 || (setup.bm_request_type & USB_REQTYPE_DIR_IN) == 0;
        let mut status_buf = [0u8; 8];

        for _ in 0..MAX_RETRIES {
            match self.do_transfer(CH, 0, status_in, HCCHAR_EPTYPE_CTRL, HCTSIZ_PID_DATA1, &mut status_buf, 0) {
                TransferResult::Success(_) => return Ok(transferred),
                TransferResult::Nak => { delay_ms(1); continue; }
                _ => return Err("STATUS failed"),
            }
        }

        Err("STATUS timeout")
    }

    pub fn enumerate(&mut self) -> Result<(), &'static str> {
        let mut desc_buf = [0u8; 18];
        let setup = UsbSetupPacket::get_descriptor(USB_DESC_DEVICE, 0, 8);
        self.control_transfer(&setup, Some(&mut desc_buf[..8]))?;

        self.ep0_max_packet = desc_buf[7] as u16;
        if self.ep0_max_packet == 0 || self.ep0_max_packet > 64 { self.ep0_max_packet = 8; }

        self.reset_port()?;
        delay_ms(20);

        let setup = UsbSetupPacket::set_address(1);
        self.control_transfer(&setup, None)?;
        self.device_address = 1;
        delay_ms(10);

        let setup = UsbSetupPacket::get_descriptor(USB_DESC_DEVICE, 0, 18);
        self.control_transfer(&setup, Some(&mut desc_buf))?;

        let mut config_buf = [0u8; 64];
        let setup = UsbSetupPacket::get_descriptor(USB_DESC_CONFIGURATION, 0, 64);
        let len = self.control_transfer(&setup, Some(&mut config_buf))?;

        self.parse_config_descriptor(&config_buf[..len])?;

        let config_val = if len >= 6 { config_buf[5] } else { 1 };
        let setup = UsbSetupPacket::set_configuration(config_val);
        self.control_transfer(&setup, None)?;

        self.enumerated = true;
        Ok(())
    }

    fn parse_config_descriptor(&mut self, data: &[u8]) -> Result<(), &'static str> {
        let mut pos = 0;
        while pos + 2 <= data.len() {
            let len = data[pos] as usize;
            let desc_type = data[pos + 1];
            if len == 0 || pos + len > data.len() { break; }

            if desc_type == USB_DESC_ENDPOINT && len >= 7 {
                let ep_addr = data[pos + 2];
                let ep_attr = data[pos + 3];
                let ep_max_pkt = u16::from_le_bytes([data[pos + 4], data[pos + 5]]);
                let is_in = (ep_addr & 0x80) != 0;
                let ep_type = ep_attr & 0x03;

                if is_in && ep_type == 3 {
                    self.hid_endpoint = ep_addr & 0x0F;
                    self.hid_max_packet = ep_max_pkt;
                    return Ok(());
                }
            }
            pos += len;
        }
        Err("No HID endpoint")
    }

    pub fn read_input(&mut self, report: &mut Xbox360InputReport) -> Result<bool, &'static str> {
        if !self.enumerated || self.hid_endpoint == 0 { return Err("Not enumerated"); }

        const CH: usize = 1;
        let pid = if self.hid_data_toggle { HCTSIZ_PID_DATA1 } else { HCTSIZ_PID_DATA0 };
        let len = core::mem::size_of::<Xbox360InputReport>().min(self.hid_max_packet as usize);

        let report_bytes = unsafe {
            core::slice::from_raw_parts_mut(report as *mut _ as *mut u8, len)
        };

        match self.do_transfer(CH, self.hid_endpoint, true, HCCHAR_EPTYPE_INTR, pid, report_bytes, len) {
            TransferResult::Success(_) => {
                self.hid_data_toggle = !self.hid_data_toggle;
                Ok(true)
            }
            TransferResult::Nak => Ok(false),
            TransferResult::Timeout => Ok(false),
            _ => Err("Transfer error"),
        }
    }

    pub fn is_enumerated(&self) -> bool { self.enumerated }
}

// ============================================================================
// Global USB Host and Button State
// ============================================================================

static mut USB_HOST: UsbHost = UsbHost::new();
static mut BUTTON_STATE: GpiButtonState = GpiButtonState { current: 0, previous: 0 };
static mut USB_INITIALIZED: bool = false;

/// Poll USB input and update button state
fn poll_usb_input() {
    unsafe {
        if !USB_INITIALIZED { return; }

        // Poll up to 4 times for better responsiveness
        for _ in 0..4 {
            let mut report = Xbox360InputReport::default();
            match USB_HOST.read_input(&mut report) {
                Ok(true) => {
                    BUTTON_STATE.update_from_xbox(&report);
                    break;
                }
                Ok(false) => { delay_us(500); }
                Err(_) => { break; }
            }
        }
    }
}

/// Get current button state
fn get_buttons() -> u16 {
    unsafe { BUTTON_STATE.current }
}

/// Check if button was just pressed this frame
fn button_just_pressed(button: u16) -> bool {
    unsafe { BUTTON_STATE.just_pressed(button) }
}

/// Check if button was just released this frame
fn button_just_released(button: u16) -> bool {
    unsafe { BUTTON_STATE.just_released(button) }
}

/// Check if button is currently held
fn button_pressed(button: u16) -> bool {
    unsafe { BUTTON_STATE.is_pressed(button) }
}

// ============================================================================
// ROM Buffer - Allocated on heap (Pi has limited .bss space)
// ============================================================================

const MAX_ROM_SIZE: usize = 2 * 1024 * 1024; // 2MB max

// ============================================================================
// ROM Browser UI
// ============================================================================

struct RomBrowser {
    rom_count: usize,
    selected: usize,
    scroll_offset: usize,
}

impl RomBrowser {
    const VISIBLE_ITEMS: usize = 15;

    fn new(rom_count: usize) -> Self {
        Self { rom_count, selected: 0, scroll_offset: 0 }
    }

    fn draw(&self, fb: &Framebuffer, fs: &mut Fat32) {
        fb.clear(DARK_BLUE);

        draw_string(fb, 200, 20, "GB-OS ROM Browser", CYAN, DARK_BLUE);
        draw_string(fb, 180, 40, "Select ROM with D-Pad", WHITE, DARK_BLUE);

        if self.rom_count == 0 {
            draw_string(fb, 200, 200, "No ROMs found!", RED, DARK_BLUE);
            draw_string(fb, 120, 230, "Place .gb or .gbc files on SD", WHITE, DARK_BLUE);
            return;
        }

        let list_y = 80;
        let item_height = 20;
        let visible_count = Self::VISIBLE_ITEMS.min(self.rom_count);

        for i in 0..visible_count {
            let rom_idx = self.scroll_offset + i;
            if rom_idx >= self.rom_count { break; }

            let y = list_y + (i as u32) * item_height;
            let mut name_buf = [0u8; 12];

            if fs.get_rom_name(rom_idx, &mut name_buf) {
                let name_str = core::str::from_utf8(&name_buf).unwrap_or("????????.???");

                let (fg, bg) = if rom_idx == self.selected {
                    (BLACK, CYAN)
                } else {
                    (WHITE, DARK_BLUE)
                };

                if rom_idx == self.selected {
                    fb.fill_rect(100, y, 440, item_height as u32 - 2, bg);
                }

                draw_string(fb, 110, y + 4, name_str, fg, bg);
            }
        }

        let _ = core::write!(
            &mut StringWriter::new(fb, 150, 420, WHITE, DARK_BLUE),
            "ROMs found: {}  |  Press A to start", self.rom_count
        );
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            }
        }
    }

    fn move_down(&mut self) {
        if self.selected < self.rom_count.saturating_sub(1) {
            self.selected += 1;
            if self.selected >= self.scroll_offset + Self::VISIBLE_ITEMS {
                self.scroll_offset = self.selected - Self::VISIBLE_ITEMS + 1;
            }
        }
    }

    fn get_selection(&self) -> usize {
        self.selected
    }
}

/// Show ROM browser and return selected ROM index
/// Returns None if no ROMs found or user cancels
fn select_rom(fb: &Framebuffer, fs: &mut Fat32) -> Option<usize> {
    let rom_count = fs.count_roms();
    if rom_count == 0 {
        return None;
    }

    let mut browser = RomBrowser::new(rom_count);
    browser.draw(fb, fs);

    loop {
        poll_usb_input();

        let mut needs_redraw = false;

        if button_just_pressed(BTN_UP) {
            browser.move_up();
            needs_redraw = true;
        }
        if button_just_pressed(BTN_DOWN) {
            browser.move_down();
            needs_redraw = true;
        }

        if button_just_pressed(BTN_A) {
            // Debug: show we detected A press before returning
            draw_string(fb, 10, 460, "A pressed! Returning...", YELLOW, DARK_BLUE);
            delay_ms(500);
            return Some(browser.get_selection());
        }

        if needs_redraw {
            browser.draw(fb, fs);
        }

        delay_ms(8);
    }
}

/// Load ROM at given index into heap-allocated Vec
/// Returns the ROM data as a Vec if successful
fn load_rom(fb: &Framebuffer, fs: &mut Fat32, index: usize) -> Option<Vec<u8>> {
    draw_string(fb, 10, 130, "load_rom: entered", WHITE, DARK_BLUE);

    // Test 1KB allocation right here
    draw_string(fb, 10, 150, "Quick 1KB test...", WHITE, DARK_BLUE);
    let test_layout = Layout::from_size_align(1024, 8).unwrap();
    let test_ptr = unsafe { heap_alloc::alloc(test_layout) };
    if test_ptr.is_null() {
        draw_string(fb, 150, 150, "FAIL", RED, DARK_BLUE);
        return None;
    }
    draw_string(fb, 150, 150, "OK", GREEN, DARK_BLUE);
    unsafe { heap_alloc::dealloc(test_ptr, test_layout); }

    draw_string(fb, 10, 170, "Finding ROM...", WHITE, DARK_BLUE);

    let (cluster, size) = match fs.find_rom(index) {
        Some(info) => {
            draw_string(fb, 150, 170, "OK", GREEN, DARK_BLUE);
            info
        }
        None => {
            draw_string(fb, 150, 170, "FAIL", RED, DARK_BLUE);
            return None;
        }
    };

    let rom_size = size as usize;

    // Show ROM size
    let size_kb = rom_size / 1024;
    draw_string(fb, 10, 190, "Size KB:", WHITE, DARK_BLUE);
    let d0 = ((size_kb / 1000) % 10) as u8;
    let d1 = ((size_kb / 100) % 10) as u8;
    let d2 = ((size_kb / 10) % 10) as u8;
    let d3 = (size_kb % 10) as u8;
    draw_char(fb, 80, 190, (b'0' + d0) as char, GREEN, DARK_BLUE);
    draw_char(fb, 88, 190, (b'0' + d1) as char, GREEN, DARK_BLUE);
    draw_char(fb, 96, 190, (b'0' + d2) as char, GREEN, DARK_BLUE);
    draw_char(fb, 104, 190, (b'0' + d3) as char, GREEN, DARK_BLUE);

    if rom_size > MAX_ROM_SIZE {
        draw_string(fb, 10, 210, "ROM too large!", RED, DARK_BLUE);
        return None;
    }

    // Use RAW ALLOCATION - bypass Vec's broken grow machinery
    draw_string(fb, 10, 210, "Testing alloc sizes...", WHITE, DARK_BLUE);
    delay_ms(100);

    // Test 64KB first
    draw_string(fb, 10, 230, "64KB...", WHITE, DARK_BLUE);
    let test_layout = Layout::from_size_align(64 * 1024, 8).unwrap();
    let test_ptr = unsafe { heap_alloc::alloc(test_layout) };
    if test_ptr.is_null() {
        draw_string(fb, 70, 230, "FAIL", RED, DARK_BLUE);
        return None;
    }
    draw_string(fb, 70, 230, "OK", GREEN, DARK_BLUE);
    unsafe { heap_alloc::dealloc(test_ptr, test_layout); }

    // Test 256KB
    draw_string(fb, 110, 230, "256KB...", WHITE, DARK_BLUE);
    let test_layout = Layout::from_size_align(256 * 1024, 8).unwrap();
    let test_ptr = unsafe { heap_alloc::alloc(test_layout) };
    if test_ptr.is_null() {
        draw_string(fb, 180, 230, "FAIL", RED, DARK_BLUE);
        return None;
    }
    draw_string(fb, 180, 230, "OK", GREEN, DARK_BLUE);
    unsafe { heap_alloc::dealloc(test_ptr, test_layout); }

    // Test 1MB
    draw_string(fb, 220, 230, "1MB...", WHITE, DARK_BLUE);
    let test_layout = Layout::from_size_align(1024 * 1024, 8).unwrap();
    let test_ptr = unsafe { heap_alloc::alloc(test_layout) };
    if test_ptr.is_null() {
        draw_string(fb, 270, 230, "FAIL", RED, DARK_BLUE);
        return None;
    }
    draw_string(fb, 270, 230, "OK", GREEN, DARK_BLUE);
    unsafe { heap_alloc::dealloc(test_ptr, test_layout); }

    // Test 2MB
    draw_string(fb, 310, 230, "2MB...", WHITE, DARK_BLUE);
    let test_layout = Layout::from_size_align(2 * 1024 * 1024, 8).unwrap();
    let test_ptr = unsafe { heap_alloc::alloc(test_layout) };
    if test_ptr.is_null() {
        draw_string(fb, 360, 230, "FAIL", RED, DARK_BLUE);
        return None;
    }
    draw_string(fb, 360, 230, "OK", GREEN, DARK_BLUE);
    unsafe { heap_alloc::dealloc(test_ptr, test_layout); }

    draw_string(fb, 400, 230, "ALL OK!", GREEN, DARK_BLUE);
    delay_ms(500);

    // Now allocate actual ROM size
    draw_string(fb, 10, 250, "ROM alloc:", WHITE, DARK_BLUE);
    let layout = match Layout::from_size_align(rom_size, 8) {
        Ok(l) => {
            draw_string(fb, 100, 250, "layout OK", GREEN, DARK_BLUE);
            l
        }
        Err(_) => {
            draw_string(fb, 100, 250, "layout FAIL", RED, DARK_BLUE);
            return None;
        }
    };
    delay_ms(100);

    draw_string(fb, 10, 270, "Calling alloc...", WHITE, DARK_BLUE);
    draw_string(fb, 150, 270, "NOW", YELLOW, DARK_BLUE);
    delay_ms(100);

    // Enable allocator debug to see if it gets called
    enable_alloc_debug(fb.addr as usize, 50);

    let raw_ptr = unsafe { heap_alloc::alloc(layout) };

    disable_alloc_debug();

    draw_string(fb, 200, 270, "DONE", CYAN, DARK_BLUE);

    if raw_ptr.is_null() {
        draw_string(fb, 10, 290, "Alloc returned NULL!", RED, DARK_BLUE);
        return None;
    }

    draw_string(fb, 10, 290, "Alloc OK!", GREEN, DARK_BLUE);

    // Create mutable slice
    draw_string(fb, 10, 310, "Creating slice...", WHITE, DARK_BLUE);
    let rom_buffer = unsafe { core::slice::from_raw_parts_mut(raw_ptr, rom_size) };
    draw_string(fb, 180, 310, "OK", GREEN, DARK_BLUE);

    draw_string(fb, 10, 330, "Reading from SD...", WHITE, DARK_BLUE);
    delay_ms(100);

    match fs.read_file(cluster, size, rom_buffer) {
        Ok(bytes_read) => {
            if bytes_read == 0 {
                draw_string(fb, 10, 350, "Zero bytes read!", RED, DARK_BLUE);
                unsafe { heap_alloc::dealloc(raw_ptr, layout); }
                return None;
            }

            draw_string(fb, 200, 330, "OK", GREEN, DARK_BLUE);

            // Convert raw allocation to Vec using from_raw_parts
            draw_string(fb, 10, 350, "Creating Vec...", WHITE, DARK_BLUE);
            let rom_data = unsafe {
                Vec::from_raw_parts(raw_ptr, bytes_read, rom_size)
            };
            draw_string(fb, 180, 350, "OK", GREEN, DARK_BLUE);

            draw_string(fb, 10, 370, "ROM loaded!", GREEN, DARK_BLUE);
            delay_ms(500);

            Some(rom_data)
        }
        Err(e) => {
            draw_string(fb, 10, 350, "Read failed!", RED, DARK_BLUE);
            draw_string(fb, 10, 370, e, RED, DARK_BLUE);
            unsafe { heap_alloc::dealloc(raw_ptr, layout); }
            None
        }
    }
}

// ============================================================================
// Emulator Loop - Never returns (matches x86 architecture)
// ============================================================================

fn run_emulator(fb: &Framebuffer, rom_data: Vec<u8>) -> ! {
    draw_string(fb, 200, 240, "Creating Device...", WHITE, DARK_BLUE);
    delay_ms(100);

    let mut device = match Device::new_cgb(rom_data, false) {
        Ok(d) => {
            draw_string(fb, 200, 260, "Device OK!", GREEN, DARK_BLUE);
            delay_ms(500);
            d
        }
        Err(e) => {
            fb.clear(RED);
            draw_string(fb, 100, 200, "Emulator init failed!", WHITE, RED);
            draw_string(fb, 100, 220, e, WHITE, RED);
            loop { unsafe { core::arch::asm!("wfe"); } }
        }
    };

    draw_string(fb, 200, 280, "Starting game!", CYAN, DARK_BLUE);
    delay_ms(500);

    fb.clear(BLACK);
    fb.draw_gb_border(GRAY);

    let mut last_frame_time = micros();

    const CYCLES_PER_FRAME: u32 = 70224;

    loop {
        // Run one frame of emulation
        let mut cycles: u32 = 0;
        while cycles < CYCLES_PER_FRAME {
            cycles += device.do_cycle();
        }

        // Render
        if device.mode() == GbMode::Color {
            fb.blit_gb_screen_gbc(device.get_gpu_data());
        } else {
            fb.blit_gb_screen_dmg(device.get_pal_data());
        }

        // Poll USB input
        poll_usb_input();

        // Handle input - map GPi buttons to GameBoy keys
        if button_just_pressed(BTN_RIGHT)  { device.keydown(KeypadKey::Right); }
        if button_just_released(BTN_RIGHT) { device.keyup(KeypadKey::Right); }
        if button_just_pressed(BTN_LEFT)   { device.keydown(KeypadKey::Left); }
        if button_just_released(BTN_LEFT)  { device.keyup(KeypadKey::Left); }
        if button_just_pressed(BTN_UP)     { device.keydown(KeypadKey::Up); }
        if button_just_released(BTN_UP)    { device.keyup(KeypadKey::Up); }
        if button_just_pressed(BTN_DOWN)   { device.keydown(KeypadKey::Down); }
        if button_just_released(BTN_DOWN)  { device.keyup(KeypadKey::Down); }
        if button_just_pressed(BTN_A)      { device.keydown(KeypadKey::A); }
        if button_just_released(BTN_A)     { device.keyup(KeypadKey::A); }
        if button_just_pressed(BTN_B)      { device.keydown(KeypadKey::B); }
        if button_just_released(BTN_B)     { device.keyup(KeypadKey::B); }
        if button_just_pressed(BTN_START)  { device.keydown(KeypadKey::Start); }
        if button_just_released(BTN_START) { device.keyup(KeypadKey::Start); }
        if button_just_pressed(BTN_SELECT) { device.keydown(KeypadKey::Select); }
        if button_just_released(BTN_SELECT){ device.keyup(KeypadKey::Select); }

        // Frame timing (~59.7 fps)
        let target_time = last_frame_time.wrapping_add(FRAME_TIME_US);
        while micros().wrapping_sub(target_time) > 0x80000000 {}
        last_frame_time = micros();
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[no_mangle]
pub extern "C" fn boot_main() -> ! {
    ALLOCATOR.init();
    configure_gpio_for_dpi();

    let fb = match Framebuffer::new() {
        Some(f) => f,
        None => loop { unsafe { core::arch::asm!("wfe"); } }
    };

    // Store framebuffer address globally for debug output
    ALLOC_DEBUG_FB_ADDR.store(fb.addr as usize, Ordering::Relaxed);

    fb.clear(DARK_BLUE);

    let mut con = Console::new(&fb, WHITE, DARK_BLUE);

    con.set_color(CYAN, DARK_BLUE);
    con.println("=== GB-OS for GPi Case 2W ===");
    con.set_color(RED, DARK_BLUE);
    con.println("*** DEBUG BUILD v19 ***");
    con.set_color(WHITE, DARK_BLUE);
    con.newline();

    // Initialize USB HID input
    con.set_color(WHITE, DARK_BLUE);
    con.println("Initializing USB gamepad...");

    let usb = unsafe { &mut USB_HOST };

    match usb.init() {
        Ok(()) => {
            con.set_color(GREEN, DARK_BLUE);
            con.println("  USB controller initialized");

            con.set_color(WHITE, DARK_BLUE);
            con.println("  Waiting for gamepad...");

            if usb.wait_for_connection(3000) {
                delay_ms(150);

                match usb.reset_port() {
                    Ok(()) => {
                        con.set_color(GREEN, DARK_BLUE);
                        con.println("  Port reset OK");

                        con.set_color(WHITE, DARK_BLUE);
                        con.println("  Enumerating device...");

                        match usb.enumerate() {
                            Ok(()) => {
                                unsafe { USB_INITIALIZED = true; }
                                con.set_color(GREEN, DARK_BLUE);
                                con.println("  Gamepad ready!");
                            }
                            Err(e) => {
                                con.set_color(RED, DARK_BLUE);
                                let _ = write!(con, "  Enumeration failed: {}\n", e);
                            }
                        }
                    }
                    Err(e) => {
                        con.set_color(RED, DARK_BLUE);
                        let _ = write!(con, "  Port reset failed: {}\n", e);
                    }
                }
            } else {
                con.set_color(YELLOW, DARK_BLUE);
                con.println("  No USB device detected");
            }
        }
        Err(e) => {
            con.set_color(RED, DARK_BLUE);
            let _ = write!(con, "  USB init failed: {}\n", e);
        }
    }

    con.newline();
    con.set_color(WHITE, DARK_BLUE);
    con.println("Mounting SD card...");

    // TEST ALLOC #1: Before Fat32 - with granular debug
    con.set_color(YELLOW, DARK_BLUE);
    con.print("Alloc test: ");
    con.print("1");  // Made it here

    // Create layout manually without any function calls
    // Layout for 1024 bytes, 8 byte alignment
    // SAFETY: These values are valid (size > 0, align is power of 2)
    let test_layout = unsafe { Layout::from_size_align_unchecked(1024, 8) };
    con.print("2");  // Layout created

    con.print("3");  // About to call alloc

    // Try calling allocator directly
    let test_ptr = unsafe { ALLOCATOR.alloc(test_layout) };

    con.print("4");  // Returned from alloc

    if test_ptr.is_null() {
        con.set_color(RED, DARK_BLUE);
        con.println(" FAIL");
    } else {
        con.set_color(GREEN, DARK_BLUE);
        con.println(" OK");
        unsafe { ALLOCATOR.dealloc(test_ptr, test_layout); }
    }
    con.set_color(WHITE, DARK_BLUE);

    let mut fs = Fat32::new();

    match fs.mount() {
        Ok(()) => {
            con.set_color(GREEN, DARK_BLUE);
            con.println("SD card mounted!");
        }
        Err(e) => {
            con.set_color(RED, DARK_BLUE);
            let _ = write!(con, "Mount failed: {}\n", e);
            con.println("Insert SD card with FAT32 partition");
            loop { unsafe { core::arch::asm!("wfe"); } }
        }
    }

    // TEST ALLOC #2: After mount
    con.set_color(YELLOW, DARK_BLUE);
    con.print("Post-mount: ");
    let test_ptr = unsafe { ALLOCATOR.alloc(test_layout) };
    if test_ptr.is_null() {
        con.set_color(RED, DARK_BLUE);
        con.println("FAIL");
    } else {
        con.set_color(GREEN, DARK_BLUE);
        con.println("OK");
        unsafe { ALLOCATOR.dealloc(test_ptr, test_layout); }
    }
    con.set_color(WHITE, DARK_BLUE);

    let rom_count = fs.count_roms();
    let _ = write!(con, "Found {} ROM(s)\n", rom_count);

    // TEST ALLOC #3: After count_roms
    con.set_color(YELLOW, DARK_BLUE);
    con.print("Post-count: ");
    let test_ptr = unsafe { ALLOCATOR.alloc(test_layout) };
    if test_ptr.is_null() {
        con.set_color(RED, DARK_BLUE);
        con.println("FAIL");
    } else {
        con.set_color(GREEN, DARK_BLUE);
        con.println("OK");
        unsafe { ALLOCATOR.dealloc(test_ptr, test_layout); }
    }
    con.set_color(WHITE, DARK_BLUE);

    if rom_count == 0 {
        con.set_color(YELLOW, DARK_BLUE);
        con.println("No .gb or .gbc files found!");
        con.println("Place ROMs in SD card root directory");
        loop { unsafe { core::arch::asm!("wfe"); } }
    }

    con.newline();
    con.println("Starting ROM browser...");
    con.set_color(YELLOW, DARK_BLUE);
    con.println("DEBUG: About to call select_rom()");
    con.set_color(WHITE, DARK_BLUE);
    delay_ms(1000);

    // Show ROM browser and get selection
    if let Some(rom_index) = select_rom(&fb, &mut fs) {
        // TEST ALLOC #4: After select_rom
        fb.clear(DARK_BLUE);
        draw_string(&fb, 10, 10, "Post-select alloc: ", WHITE, DARK_BLUE);
        let test_ptr = unsafe { ALLOCATOR.alloc(test_layout) };
        if test_ptr.is_null() {
            draw_string(&fb, 180, 10, "FAIL!", RED, DARK_BLUE);
            loop { delay_ms(1000); }
        }
        draw_string(&fb, 180, 10, "OK", GREEN, DARK_BLUE);
        unsafe { ALLOCATOR.dealloc(test_ptr, test_layout); }

        // Show loading screen with step-by-step debug
        draw_string(&fb, 10, 30, "STEP 1: Cleared screen", WHITE, DARK_BLUE);
        draw_string(&fb, 10, 50, "STEP 2: Got ROM index from browser", WHITE, DARK_BLUE);
        draw_string(&fb, 10, 70, "STEP 3: About to call load_rom", WHITE, DARK_BLUE);

        // Show index
        let idx_char = (b'0' + (rom_index % 10) as u8) as char;
        draw_string(&fb, 10, 90, "Index:", WHITE, DARK_BLUE);
        draw_char(&fb, 70, 90, idx_char, GREEN, DARK_BLUE);

        // Pause to see output
        draw_string(&fb, 10, 110, "Calling load_rom in 1 sec...", YELLOW, DARK_BLUE);
        delay_ms(1000);

        // Load selected ROM into heap-allocated Vec
        if let Some(rom_data) = load_rom(&fb, &mut fs, rom_index) {
            // Clear screen before starting emulator
            fb.clear(DARK_BLUE);
            draw_string(&fb, 200, 200, "ROM LOADED!", GREEN, DARK_BLUE);
            draw_string(&fb, 200, 220, "Starting emulator...", WHITE, DARK_BLUE);
            delay_ms(1000);

            // Run emulator - never returns
            run_emulator(&fb, rom_data);
        } else {
            // ROM load failed - debug info already shown
            draw_string(&fb, 200, 460, "ROM LOAD FAILED", RED, DARK_BLUE);
            // Wait forever so we can see the error
            loop {
                delay_ms(1000);
            }
        }
    }

    // Halt if we get here (no ROM selected or load failed)
    loop {
        unsafe { core::arch::asm!("wfe"); }
    }
}

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // Show blue bar for panic
    let fb_addr = ALLOC_DEBUG_FB_ADDR.load(Ordering::Relaxed);
    if fb_addr != 0 {
        unsafe {
            let pitch = 640 * 4;
            // Draw thick blue bar from y=0 to y=30
            for y in 0u32..30 {
                for x in 0u32..640 {
                    let offset = (y * pitch + x * 4) as usize;
                    core::ptr::write_volatile((fb_addr + offset) as *mut u32, 0xFF0000FF);
                }
            }
        }
    }
    loop { unsafe { core::arch::asm!("wfe"); } }
}
