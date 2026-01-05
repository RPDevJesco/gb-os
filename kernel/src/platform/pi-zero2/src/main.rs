//! GB-OS Kernel for Pi Zero 2W / GPi Case 2W
//!
//! SD card diagnostics - now probing BOTH SDHOST and EMMC controllers!

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

// SDHOST Controller (legacy, sometimes used for WiFi)
const SDHOST_BASE: usize = PERIPHERAL_BASE + 0x0020_2000;
const SDHOST_CMD: usize = SDHOST_BASE + 0x00;
const SDHOST_ARG: usize = SDHOST_BASE + 0x04;
const SDHOST_TOUT: usize = SDHOST_BASE + 0x08;
const SDHOST_CDIV: usize = SDHOST_BASE + 0x0C;
const SDHOST_RSP0: usize = SDHOST_BASE + 0x10;
const SDHOST_RSP1: usize = SDHOST_BASE + 0x14;
const SDHOST_RSP2: usize = SDHOST_BASE + 0x18;
const SDHOST_RSP3: usize = SDHOST_BASE + 0x1C;
const SDHOST_HSTS: usize = SDHOST_BASE + 0x20;
const SDHOST_VDD: usize = SDHOST_BASE + 0x30;
const SDHOST_EDM: usize = SDHOST_BASE + 0x34;
const SDHOST_HCFG: usize = SDHOST_BASE + 0x38;
const SDHOST_HBCT: usize = SDHOST_BASE + 0x3C;
const SDHOST_DATA: usize = SDHOST_BASE + 0x40;
const SDHOST_HBLC: usize = SDHOST_BASE + 0x50;

// EMMC/SDHCI Controller (Arasan, often used for SD card on Pi 3/Zero2W)
const EMMC_BASE: usize = PERIPHERAL_BASE + 0x0030_0000;
const EMMC_ARG2: usize = EMMC_BASE + 0x00;
const EMMC_BLKSIZECNT: usize = EMMC_BASE + 0x04;
const EMMC_ARG1: usize = EMMC_BASE + 0x08;
const EMMC_CMDTM: usize = EMMC_BASE + 0x0C;
const EMMC_RESP0: usize = EMMC_BASE + 0x10;
const EMMC_RESP1: usize = EMMC_BASE + 0x14;
const EMMC_RESP2: usize = EMMC_BASE + 0x18;
const EMMC_RESP3: usize = EMMC_BASE + 0x1C;
const EMMC_DATA: usize = EMMC_BASE + 0x20;
const EMMC_STATUS: usize = EMMC_BASE + 0x24;
const EMMC_CONTROL0: usize = EMMC_BASE + 0x28;
const EMMC_CONTROL1: usize = EMMC_BASE + 0x2C;
const EMMC_INTERRUPT: usize = EMMC_BASE + 0x30;
const EMMC_IRPT_MASK: usize = EMMC_BASE + 0x34;
const EMMC_IRPT_EN: usize = EMMC_BASE + 0x38;
const EMMC_CONTROL2: usize = EMMC_BASE + 0x3C;
const EMMC_SLOTISR_VER: usize = EMMC_BASE + 0xFC;

// EMMC Command flags (CMDTM register)
const EMMC_CMD_NEED_APP: u32 = 0x80000000;
const EMMC_CMD_RSPNS_48: u32 = 0x00020000;
const EMMC_CMD_RSPNS_136: u32 = 0x00010000;
const EMMC_CMD_RSPNS_48B: u32 = 0x00030000;
const EMMC_CMD_CRCCHK_EN: u32 = 0x00080000;
const EMMC_CMD_IXCHK_EN: u32 = 0x00100000;
const EMMC_CMD_ISDATA: u32 = 0x00200000;
const EMMC_CMD_DATA_READ: u32 = 0x00000010;
const EMMC_CMD_DATA_WRITE: u32 = 0x00000000;

// EMMC Status bits
const EMMC_STATUS_CMD_INHIBIT: u32 = 0x00000001;
const EMMC_STATUS_DAT_INHIBIT: u32 = 0x00000002;

// EMMC Interrupt bits
const EMMC_INT_CMD_DONE: u32 = 0x00000001;
const EMMC_INT_DATA_DONE: u32 = 0x00000002;
const EMMC_INT_READ_RDY: u32 = 0x00000020;
const EMMC_INT_WRITE_RDY: u32 = 0x00000010;
const EMMC_INT_ERROR: u32 = 0x00008000;
const EMMC_INT_CMD_TIMEOUT: u32 = 0x00010000;
const EMMC_INT_DATA_TIMEOUT: u32 = 0x00100000;
const EMMC_INT_ERR_MASK: u32 = 0xFFFF0000;

// SDHOST Command flags
const SDHOST_CMD_NEW: u32 = 0x8000;
const SDHOST_CMD_FAIL: u32 = 0x4000;
const SDHOST_CMD_BUSY: u32 = 0x0800;
const SDHOST_CMD_NO_RSP: u32 = 0x0400;
const SDHOST_CMD_LONG_RSP: u32 = 0x0200;
const SDHOST_CMD_WRITE: u32 = 0x0080;
const SDHOST_CMD_READ: u32 = 0x0040;

// SDHOST Status bits
const SDHOST_HSTS_ERROR: u32 = 0x0008;
const SDHOST_HSTS_CRC7_ERROR: u32 = 0x0010;
const SDHOST_HSTS_CRC16_ERROR: u32 = 0x0020;
const SDHOST_HSTS_CMD_TIME_OUT: u32 = 0x0040;
const SDHOST_HSTS_REW_TIME_OUT: u32 = 0x0080;
const SDHOST_HSTS_DATA_FLAG: u32 = 0x0001;

// SDHOST Config bits
const SDHOST_HCFG_BUSY_EN: u32 = 0x0400;
const SDHOST_HCFG_SLOW_CARD: u32 = 0x0002;
const SDHOST_HCFG_INTBUS: u32 = 0x0001;

// Display dimensions
const SCREEN_WIDTH: u32 = 640;
const SCREEN_HEIGHT: u32 = 480;

// ============================================================================
// Entry Point
// ============================================================================

core::arch::global_asm!(
    r#"
.section .text.boot
.global _start

_start:
    // Get core ID
    mrs     x0, mpidr_el1
    and     x0, x0, #0xFF

    // Park secondary cores
    cbnz    x0, .Lpark

    // Set up stack (512KB below 1MB mark)
    mov     x1, #0x0010
    lsl     x1, x1, #16
    mov     sp, x1

    // Clear BSS
    ldr     x0, =__bss_start
    ldr     x1, =__bss_end
.Lclear_bss:
    cmp     x0, x1
    b.ge    .Ldone_bss
    str     xzr, [x0], #8
    b       .Lclear_bss
.Ldone_bss:

    // Jump to Rust
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
// MMIO Helpers
// ============================================================================

#[inline(always)]
fn mmio_read(addr: usize) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}

#[inline(always)]
fn mmio_write(addr: usize, val: u32) {
    unsafe { write_volatile(addr as *mut u32, val) }
}

fn delay(cycles: u32) {
    for _ in 0..cycles {
        unsafe { core::arch::asm!("nop") };
    }
}

fn delay_us(us: u32) {
    // ~1 cycle per nop at 1GHz, rough approximation
    delay(us * 1000);
}

fn delay_ms(ms: u32) {
    delay_us(ms * 1000);
}

// ============================================================================
// GPIO Functions
// ============================================================================

fn gpio_set_function(pin: u8, function: u8) {
    let reg = match pin / 10 {
        0 => GPFSEL0,
        1 => GPFSEL1,
        2 => GPFSEL2,
        3 => GPFSEL3,
        4 => GPFSEL4,
        _ => return,
    };

    let shift = (pin % 10) * 3;
    let mask = 0b111 << shift;
    let val = (function as u32) << shift;

    let current = mmio_read(reg);
    mmio_write(reg, (current & !mask) | val);
}

fn gpio_set_pull(pin: u8, pull: u8) {
    // 0 = off, 1 = pull-down, 2 = pull-up
    mmio_write(GPPUD, pull as u32);
    delay(150);

    let bit = 1u32 << pin;
    mmio_write(GPPUDCLK0, bit);
    delay(150);

    mmio_write(GPPUD, 0);
    mmio_write(GPPUDCLK0, 0);
}

fn gpio_set(pin: u8) {
    mmio_write(GPSET0, 1 << pin);
}

fn gpio_clear(pin: u8) {
    mmio_write(GPCLR0, 1 << pin);
}

fn gpio_read(pin: u8) -> bool {
    (mmio_read(GPLEV0) & (1 << pin)) != 0
}

// ============================================================================
// DPI Display GPIO Configuration
// ============================================================================

fn configure_gpio_for_dpi() {
    const ALT2: u32 = 0b110;

    let gpfsel0_val: u32 =
        (ALT2 << 0)  |  // GPIO 0: PCLK
            (ALT2 << 3)  |  // GPIO 1: DE
            (ALT2 << 6)  |  // GPIO 2: VSYNC
            (ALT2 << 9)  |  // GPIO 3: HSYNC
            (ALT2 << 12) |  // GPIO 4: B2
            (ALT2 << 15) |  // GPIO 5: B3
            (ALT2 << 18) |  // GPIO 6: B4
            (ALT2 << 21) |  // GPIO 7: B5
            (ALT2 << 24) |  // GPIO 8: B6
            (ALT2 << 27);   // GPIO 9: B7

    let gpfsel1_val: u32 =
        (ALT2 << 0)  |  // GPIO 10: G2
            (ALT2 << 3)  |  // GPIO 11: G3
            (ALT2 << 6)  |  // GPIO 12: G4
            (ALT2 << 9)  |  // GPIO 13: G5
            (ALT2 << 12) |  // GPIO 14: G6
            (ALT2 << 15) |  // GPIO 15: G7
            (ALT2 << 18) |  // GPIO 16: R2
            (ALT2 << 21) |  // GPIO 17: R3
            (ALT2 << 24) |  // GPIO 18: R4
            (ALT2 << 27);   // GPIO 19: R5

    let gpfsel2_current = mmio_read(GPFSEL2);
    let gpfsel2_mask: u32 = 0b111111;
    let gpfsel2_val: u32 =
        (ALT2 << 0) |   // GPIO 20: R6
            (ALT2 << 3);    // GPIO 21: R7

    mmio_write(GPFSEL0, gpfsel0_val);
    mmio_write(GPFSEL1, gpfsel1_val);
    mmio_write(GPFSEL2, (gpfsel2_current & !gpfsel2_mask) | gpfsel2_val);

    mmio_write(GPPUD, 0);
    delay(150);
    mmio_write(GPPUDCLK0, 0x003F_FFFF);
    delay(150);
    mmio_write(GPPUD, 0);
    mmio_write(GPPUDCLK0, 0);
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

// ============================================================================
// Framebuffer
// ============================================================================

struct Framebuffer {
    addr: u32,
    width: u32,
    height: u32,
    pitch: u32,
    depth: u32,
}

impl Framebuffer {
    fn new() -> Option<Self> {
        let mut mbox = MailboxBuffer::new();

        mbox.data[0] = 35 * 4;
        mbox.data[1] = 0;

        mbox.data[2] = 0x0004_8003;
        mbox.data[3] = 8;
        mbox.data[4] = 8;
        mbox.data[5] = SCREEN_WIDTH;
        mbox.data[6] = SCREEN_HEIGHT;

        mbox.data[7] = 0x0004_8004;
        mbox.data[8] = 8;
        mbox.data[9] = 8;
        mbox.data[10] = SCREEN_WIDTH;
        mbox.data[11] = SCREEN_HEIGHT;

        mbox.data[12] = 0x0004_8005;
        mbox.data[13] = 4;
        mbox.data[14] = 4;
        mbox.data[15] = 32;

        mbox.data[16] = 0x0004_8006;
        mbox.data[17] = 4;
        mbox.data[18] = 4;
        mbox.data[19] = 0;

        mbox.data[20] = 0x0004_0001;
        mbox.data[21] = 8;
        mbox.data[22] = 8;
        mbox.data[23] = 16;
        mbox.data[24] = 0;

        mbox.data[25] = 0x0004_0008;
        mbox.data[26] = 4;
        mbox.data[27] = 4;
        mbox.data[28] = 0;

        mbox.data[29] = 0;

        if mailbox_call(&mut mbox, 8) && mbox.data[23] != 0 {
            Some(Self {
                addr: mbox.data[23] & 0x3FFF_FFFF,
                width: mbox.data[5],
                height: mbox.data[6],
                pitch: mbox.data[28],
                depth: mbox.data[15],
            })
        } else {
            None
        }
    }

    fn put_pixel(&self, x: u32, y: u32, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = y * self.pitch + x * 4;
        unsafe {
            write_volatile((self.addr + offset) as *mut u32, color);
        }
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
}

// ============================================================================
// Simple Text Rendering
// ============================================================================

static FONT_8X8: [[u8; 8]; 96] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x00],
    [0x6C, 0x6C, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x6C, 0x6C, 0xFE, 0x6C, 0xFE, 0x6C, 0x6C, 0x00],
    [0x18, 0x3E, 0x60, 0x3C, 0x06, 0x7C, 0x18, 0x00],
    [0x00, 0x66, 0xAC, 0xD8, 0x36, 0x6A, 0xCC, 0x00],
    [0x38, 0x6C, 0x68, 0x76, 0xDC, 0xCC, 0x76, 0x00],
    [0x18, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00],
    [0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00],
    [0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00],
    [0x00, 0x18, 0x18, 0x7E, 0x18, 0x18, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30],
    [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
    [0x06, 0x0C, 0x18, 0x30, 0x60, 0xC0, 0x80, 0x00],
    [0x3C, 0x66, 0x6E, 0x7E, 0x76, 0x66, 0x3C, 0x00],
    [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
    [0x3C, 0x66, 0x06, 0x1C, 0x30, 0x66, 0x7E, 0x00],
    [0x3C, 0x66, 0x06, 0x1C, 0x06, 0x66, 0x3C, 0x00],
    [0x1C, 0x3C, 0x6C, 0xCC, 0xFE, 0x0C, 0x1E, 0x00],
    [0x7E, 0x60, 0x7C, 0x06, 0x06, 0x66, 0x3C, 0x00],
    [0x1C, 0x30, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00],
    [0x7E, 0x66, 0x06, 0x0C, 0x18, 0x18, 0x18, 0x00],
    [0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00],
    [0x3C, 0x66, 0x66, 0x3E, 0x06, 0x0C, 0x38, 0x00],
    [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00],
    [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x30, 0x00],
    [0x0C, 0x18, 0x30, 0x60, 0x30, 0x18, 0x0C, 0x00],
    [0x00, 0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00],
    [0x30, 0x18, 0x0C, 0x06, 0x0C, 0x18, 0x30, 0x00],
    [0x3C, 0x66, 0x0C, 0x18, 0x18, 0x00, 0x18, 0x00],
    [0x3C, 0x66, 0x6E, 0x6A, 0x6E, 0x60, 0x3C, 0x00],
    [0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00],
    [0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00],
    [0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00],
    [0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00],
    [0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x7E, 0x00],
    [0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x60, 0x00],
    [0x3C, 0x66, 0x60, 0x6E, 0x66, 0x66, 0x3E, 0x00],
    [0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00],
    [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
    [0x3E, 0x0C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38, 0x00],
    [0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C, 0x66, 0x00],
    [0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7E, 0x00],
    [0xC6, 0xEE, 0xFE, 0xD6, 0xC6, 0xC6, 0xC6, 0x00],
    [0x66, 0x76, 0x7E, 0x7E, 0x6E, 0x66, 0x66, 0x00],
    [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
    [0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00],
    [0x3C, 0x66, 0x66, 0x66, 0x6A, 0x6C, 0x36, 0x00],
    [0x7C, 0x66, 0x66, 0x7C, 0x6C, 0x66, 0x66, 0x00],
    [0x3C, 0x66, 0x60, 0x3C, 0x06, 0x66, 0x3C, 0x00],
    [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
    [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
    [0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00],
    [0xC6, 0xC6, 0xC6, 0xD6, 0xFE, 0xEE, 0xC6, 0x00],
    [0x66, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x66, 0x00],
    [0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00],
    [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x7E, 0x00],
    [0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00],
    [0xC0, 0x60, 0x30, 0x18, 0x0C, 0x06, 0x02, 0x00],
    [0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00],
    [0x18, 0x3C, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF],
    [0x30, 0x18, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x3C, 0x06, 0x3E, 0x66, 0x3E, 0x00],
    [0x60, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x7C, 0x00],
    [0x00, 0x00, 0x3C, 0x66, 0x60, 0x66, 0x3C, 0x00],
    [0x06, 0x06, 0x3E, 0x66, 0x66, 0x66, 0x3E, 0x00],
    [0x00, 0x00, 0x3C, 0x66, 0x7E, 0x60, 0x3C, 0x00],
    [0x1C, 0x30, 0x30, 0x7C, 0x30, 0x30, 0x30, 0x00],
    [0x00, 0x00, 0x3E, 0x66, 0x66, 0x3E, 0x06, 0x3C],
    [0x60, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x66, 0x00],
    [0x18, 0x00, 0x38, 0x18, 0x18, 0x18, 0x3C, 0x00],
    [0x18, 0x00, 0x38, 0x18, 0x18, 0x18, 0x18, 0x70],
    [0x60, 0x60, 0x66, 0x6C, 0x78, 0x6C, 0x66, 0x00],
    [0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00],
    [0x00, 0x00, 0x6C, 0xFE, 0xD6, 0xC6, 0xC6, 0x00],
    [0x00, 0x00, 0x7C, 0x66, 0x66, 0x66, 0x66, 0x00],
    [0x00, 0x00, 0x3C, 0x66, 0x66, 0x66, 0x3C, 0x00],
    [0x00, 0x00, 0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60],
    [0x00, 0x00, 0x3E, 0x66, 0x66, 0x3E, 0x06, 0x06],
    [0x00, 0x00, 0x7C, 0x66, 0x60, 0x60, 0x60, 0x00],
    [0x00, 0x00, 0x3E, 0x60, 0x3C, 0x06, 0x7C, 0x00],
    [0x30, 0x30, 0x7C, 0x30, 0x30, 0x30, 0x1C, 0x00],
    [0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x3E, 0x00],
    [0x00, 0x00, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00],
    [0x00, 0x00, 0xC6, 0xC6, 0xD6, 0xFE, 0x6C, 0x00],
    [0x00, 0x00, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x00],
    [0x00, 0x00, 0x66, 0x66, 0x66, 0x3E, 0x06, 0x3C],
    [0x00, 0x00, 0x7E, 0x0C, 0x18, 0x30, 0x7E, 0x00],
    [0x0C, 0x18, 0x18, 0x30, 0x18, 0x18, 0x0C, 0x00],
    [0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
    [0x30, 0x18, 0x18, 0x0C, 0x18, 0x18, 0x30, 0x00],
    [0x00, 0x00, 0x76, 0xDC, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
];

fn draw_char(fb: &Framebuffer, x: u32, y: u32, c: char, fg: u32, bg: u32) {
    let code = c as u8;
    let idx = if code >= 32 && code < 128 {
        (code - 32) as usize
    } else {
        0
    };

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
        if c == '\n' {
            continue;
        }
        draw_char(fb, cx, y, c, fg, bg);
        cx += 8;
        if cx + 8 > fb.width {
            break;
        }
    }
}

// ============================================================================
// Console
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
        if self.y + 10 > self.fb.height {
            self.y = 8;
        }
    }

    fn print(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                self.newline();
                continue;
            }
            draw_char(self.fb, self.x, self.y, c, self.fg, self.bg);
            self.x += 8;
            if self.x + 8 > self.fb.width - 8 {
                self.newline();
            }
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
// Colors (ARGB8888)
// ============================================================================

const BLACK: u32 = 0xFF000000;
const WHITE: u32 = 0xFFFFFFFF;
const RED: u32 = 0xFFFF0000;
const GREEN: u32 = 0xFF00FF00;
const BLUE: u32 = 0xFF0000FF;
const YELLOW: u32 = 0xFFFFFF00;
const CYAN: u32 = 0xFF00FFFF;
const MAGENTA: u32 = 0xFFFF00FF;
const GRAY: u32 = 0xFF808080;
const DARK_GRAY: u32 = 0xFF404040;
const DARK_BLUE: u32 = 0xFF000040;
const DARK_GREEN: u32 = 0xFF004000;
const ORANGE: u32 = 0xFFFFA500;

// ============================================================================
// SDHOST SD Card Controller (Alternative to EMMC)
// ============================================================================

struct SdhostSd {
    initialized: bool,
    is_sdhc: bool,
    rca: u32,
}

impl SdhostSd {
    const fn new() -> Self {
        Self {
            initialized: false,
            is_sdhc: true,
            rca: 0,
        }
    }

    fn clear_status(&self) {
        mmio_write(SDHOST_HSTS, 0x7F8);
    }

    fn reset(&self) {
        // Disable everything
        mmio_write(SDHOST_CMD, 0);
        mmio_write(SDHOST_ARG, 0);
        mmio_write(SDHOST_TOUT, 0xF00000);
        mmio_write(SDHOST_CDIV, 0);
        mmio_write(SDHOST_HSTS, 0x7F8);
        mmio_write(SDHOST_HCFG, 0);
        mmio_write(SDHOST_HBCT, 0);
        mmio_write(SDHOST_HBLC, 0);

        // Set VDD
        mmio_write(SDHOST_VDD, 1);
        delay_ms(10);

        // Configure
        mmio_write(SDHOST_HCFG, SDHOST_HCFG_SLOW_CARD | SDHOST_HCFG_INTBUS);
        mmio_write(SDHOST_CDIV, 0x148);  // ~400 kHz from 250MHz base

        delay_ms(10);
    }

    fn wait_cmd(&self, timeout_us: u32) -> Result<(), &'static str> {
        let mut count = 0u32;
        loop {
            let cmd = mmio_read(SDHOST_CMD);

            if (cmd & SDHOST_CMD_NEW) == 0 {
                // Check for errors
                let hsts = mmio_read(SDHOST_HSTS);
                if (hsts & SDHOST_HSTS_CMD_TIME_OUT) != 0 {
                    self.clear_status();
                    return Err("CMD timeout");
                }
                if (hsts & SDHOST_HSTS_CRC7_ERROR) != 0 {
                    self.clear_status();
                    return Err("CRC error");
                }
                return Ok(());
            }

            if (cmd & SDHOST_CMD_FAIL) != 0 {
                self.clear_status();
                return Err("CMD fail");
            }

            delay_us(10);
            count += 10;
            if count > timeout_us {
                return Err("Wait timeout");
            }
        }
    }

    fn send_cmd(&mut self, cmd_idx: u32, arg: u32, flags: u32) -> Result<u32, &'static str> {
        self.clear_status();
        mmio_write(SDHOST_ARG, arg);
        mmio_write(SDHOST_CMD, (cmd_idx & 0x3F) | flags | SDHOST_CMD_NEW);

        self.wait_cmd(500000)?;

        Ok(mmio_read(SDHOST_RSP0))
    }

    fn init(&mut self, con: &mut Console) -> Result<(), &'static str> {
        // Configure GPIO 48-53 for SDHOST (ALT0)
        con.println("  Configuring GPIO 48-53...");
        configure_gpio_for_sd();

        // Power on
        con.println("  Enabling SD power...");
        set_power_state(POWER_ID_SD, true);
        delay_ms(100);

        // Reset SDHOST
        con.println("  Resetting SDHOST...");
        self.reset();

        // Show state
        let edm = mmio_read(SDHOST_EDM);
        let hsts = mmio_read(SDHOST_HSTS);
        let _ = write!(con, "  EDM=0x{:08X} HSTS=0x{:08X}\n", edm, hsts);

        // CMD0: GO_IDLE
        con.println("  CMD0 (GO_IDLE)...");
        mmio_write(SDHOST_ARG, 0);
        mmio_write(SDHOST_CMD, 0 | SDHOST_CMD_NO_RSP | SDHOST_CMD_NEW);
        delay_ms(50);
        self.clear_status();

        // CMD8: SEND_IF_COND
        con.println("  CMD8 (SEND_IF_COND)...");
        match self.send_cmd(8, 0x1AA, 0) {
            Ok(resp) => {
                self.is_sdhc = (resp & 0xFF) == 0xAA;
                let _ = write!(con, "    Response: 0x{:08X} ", resp);
                if self.is_sdhc {
                    con.set_color(GREEN, DARK_BLUE);
                    con.println("(SDHC)");
                    con.set_color(WHITE, DARK_BLUE);
                } else {
                    con.println("(SD v1)");
                }
            }
            Err(e) => {
                let hsts = mmio_read(SDHOST_HSTS);
                let _ = write!(con, "    HSTS=0x{:08X} ({})\n", hsts, e);
                self.is_sdhc = false;
                self.clear_status();
            }
        }

        // ACMD41 loop
        con.println("  ACMD41 loop...");
        let mut attempts = 0u32;
        loop {
            // CMD55
            let _ = self.send_cmd(55, 0, 0);

            // ACMD41
            let hcs = if self.is_sdhc { 0x40000000 } else { 0 };

            match self.send_cmd(41, 0x00FF8000 | hcs, 0) {
                Ok(ocr) => {
                    if (ocr & 0x80000000) != 0 {
                        self.is_sdhc = (ocr & 0x40000000) != 0;
                        let _ = write!(con, "    OCR: 0x{:08X} ", ocr);
                        con.set_color(GREEN, DARK_BLUE);
                        if self.is_sdhc {
                            con.println("SDHC!");
                        } else {
                            con.println("SDSC!");
                        }
                        con.set_color(WHITE, DARK_BLUE);
                        break;
                    }
                }
                Err(_) => {
                    self.clear_status();
                }
            }

            attempts += 1;
            if attempts > 50 {
                return Err("ACMD41 timeout");
            }
            if attempts % 10 == 0 {
                let _ = write!(con, "    attempt {}...\n", attempts);
            }
            delay_ms(50);
        }

        // CMD2: ALL_SEND_CID
        con.println("  CMD2 (ALL_SEND_CID)...");
        self.send_cmd(2, 0, SDHOST_CMD_LONG_RSP)?;

        // CMD3: SEND_RELATIVE_ADDR
        con.println("  CMD3 (SEND_RELATIVE_ADDR)...");
        let resp = self.send_cmd(3, 0, 0)?;
        self.rca = resp & 0xFFFF0000;
        let _ = write!(con, "    RCA: 0x{:04X}\n", self.rca >> 16);

        // CMD7: SELECT_CARD
        con.println("  CMD7 (SELECT_CARD)...");
        self.send_cmd(7, self.rca, SDHOST_CMD_BUSY)?;

        // Speed up clock
        con.println("  Speeding up clock...");
        mmio_write(SDHOST_CDIV, 4);  // Much faster

        // Set block size
        mmio_write(SDHOST_HBCT, 512);

        self.initialized = true;
        Ok(())
    }

    fn read_sector(&mut self, lba: u32, buffer: &mut [u8; 512]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Not initialized");
        }

        mmio_write(SDHOST_HBCT, 512);
        mmio_write(SDHOST_HBLC, 1);

        let addr = if self.is_sdhc { lba } else { lba * 512 };

        self.clear_status();

        // CMD17: READ_SINGLE_BLOCK
        mmio_write(SDHOST_ARG, addr);
        mmio_write(SDHOST_CMD, 17 | SDHOST_CMD_READ | SDHOST_CMD_NEW);

        self.wait_cmd(500000)?;

        // Read data
        let mut idx = 0usize;
        let mut timeout = 0u32;

        while idx < 512 {
            let hsts = mmio_read(SDHOST_HSTS);

            if (hsts & SDHOST_HSTS_DATA_FLAG) != 0 {
                let word = mmio_read(SDHOST_DATA);
                buffer[idx] = (word >> 0) as u8;
                buffer[idx + 1] = (word >> 8) as u8;
                buffer[idx + 2] = (word >> 16) as u8;
                buffer[idx + 3] = (word >> 24) as u8;
                idx += 4;
                timeout = 0;
            } else {
                timeout += 1;
                if timeout > 500000 {
                    return Err("Data timeout");
                }
                delay_us(1);
            }
        }

        self.clear_status();
        Ok(())
    }
}

// Control1 register bits
const EMMC_CTRL1_CLK_INTLEN: u32 = 0x00000001;  // Internal clock enable
const EMMC_CTRL1_CLK_STABLE: u32 = 0x00000002;  // Internal clock stable
const EMMC_CTRL1_CLK_EN: u32 = 0x00000004;      // SD clock enable
const EMMC_CTRL1_CLK_GENSEL: u32 = 0x00000020;  // Clock generator select
const EMMC_CTRL1_DATA_TOUNIT: u32 = 0x000F0000; // Data timeout unit
const EMMC_CTRL1_SRST_HC: u32 = 0x01000000;     // Reset host controller
const EMMC_CTRL1_SRST_CMD: u32 = 0x02000000;    // Reset CMD line
const EMMC_CTRL1_SRST_DATA: u32 = 0x04000000;   // Reset DATA line

// Mailbox power device IDs
const POWER_ID_SD: u32 = 0;
const POWER_ID_EMMC: u32 = 0;

// Mailbox clock IDs
const CLOCK_ID_EMMC: u32 = 1;

/// Set power state via mailbox
fn set_power_state(device_id: u32, on: bool) -> bool {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;           // Buffer size
    mbox.data[1] = 0;               // Request
    mbox.data[2] = 0x00028001;      // Tag: Set power state
    mbox.data[3] = 8;               // Value size
    mbox.data[4] = 8;               // Request size
    mbox.data[5] = device_id;       // Device ID
    mbox.data[6] = if on { 3 } else { 0 };  // State: bit 0 = on, bit 1 = wait
    mbox.data[7] = 0;               // End tag

    mailbox_call(&mut mbox, 8) && (mbox.data[6] & 1) != 0
}

/// Get clock rate via mailbox
fn get_clock_rate(clock_id: u32) -> u32 {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = 0x00030002;      // Tag: Get clock rate
    mbox.data[3] = 8;
    mbox.data[4] = 4;
    mbox.data[5] = clock_id;
    mbox.data[6] = 0;               // Rate (response)
    mbox.data[7] = 0;

    if mailbox_call(&mut mbox, 8) {
        mbox.data[6]
    } else {
        0
    }
}

/// Set clock rate via mailbox
fn set_clock_rate(clock_id: u32, rate: u32) -> u32 {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 9 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = 0x00038002;      // Tag: Set clock rate
    mbox.data[3] = 12;
    mbox.data[4] = 8;
    mbox.data[5] = clock_id;
    mbox.data[6] = rate;
    mbox.data[7] = 0;               // Skip turbo
    mbox.data[8] = 0;               // End tag

    if mailbox_call(&mut mbox, 8) {
        mbox.data[6]
    } else {
        0
    }
}

/// Configure GPIO pins 48-53 for SD card with pull-ups
fn configure_gpio_for_sd() {
    // GPIO 48-53 need ALT0 function for SD card
    const ALT0: u32 = 0b100;

    // GPFSEL4 controls GPIO 40-49
    let gpfsel4 = mmio_read(GPFSEL4);
    let gpfsel4_new = (gpfsel4 & 0xC0FFFFFF)
        | (ALT0 << 24)   // GPIO 48: SD CLK
        | (ALT0 << 27);  // GPIO 49: SD CMD
    mmio_write(GPFSEL4, gpfsel4_new);

    // GPFSEL5 controls GPIO 50-53
    let gpfsel5 = mmio_read(GPFSEL5);
    let gpfsel5_new = (gpfsel5 & 0xFFFFF000)
        | (ALT0 << 0)    // GPIO 50: SD DAT0
        | (ALT0 << 3)    // GPIO 51: SD DAT1
        | (ALT0 << 6)    // GPIO 52: SD DAT2
        | (ALT0 << 9);   // GPIO 53: SD DAT3
    mmio_write(GPFSEL5, gpfsel5_new);

    delay(150);

    // Enable pull-ups on CMD and DAT lines (GPIO 49-53)
    // SD cards require pull-ups on CMD and DAT[0:3]
    // Pull-up = 2
    mmio_write(GPPUD, 2);  // Enable pull-up
    delay(150);

    // Clock the pull setting into GPIO 49-53
    // Bit 49 is in GPPUDCLK1 (bit 17), bits 50-53 are in GPPUDCLK1 (bits 18-21)
    mmio_write(GPPUDCLK1, (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 21));
    delay(150);

    mmio_write(GPPUD, 0);
    mmio_write(GPPUDCLK1, 0);
    delay(150);
}

struct EmmcSd {
    initialized: bool,
    is_sdhc: bool,
    rca: u32,
    base_clock: u32,
}

impl EmmcSd {
    const fn new() -> Self {
        Self {
            initialized: false,
            is_sdhc: true,
            rca: 0,
            base_clock: 0,
        }
    }

    fn clear_interrupt(&self) {
        mmio_write(EMMC_INTERRUPT, 0xFFFFFFFF);
    }

    fn reset_controller(&self) -> Result<(), &'static str> {
        // Reset the complete host circuit
        let mut ctrl1 = mmio_read(EMMC_CONTROL1);
        ctrl1 |= EMMC_CTRL1_SRST_HC;
        mmio_write(EMMC_CONTROL1, ctrl1);

        // Wait for reset to complete
        let mut timeout = 10000u32;
        while (mmio_read(EMMC_CONTROL1) & EMMC_CTRL1_SRST_HC) != 0 {
            delay_us(10);
            timeout -= 1;
            if timeout == 0 {
                return Err("HC reset timeout");
            }
        }

        Ok(())
    }

    fn setup_clock(&mut self, freq_khz: u32) -> Result<(), &'static str> {
        // Disable clock first
        let mut ctrl1 = mmio_read(EMMC_CONTROL1);
        ctrl1 &= !EMMC_CTRL1_CLK_EN;
        mmio_write(EMMC_CONTROL1, ctrl1);
        delay_us(10);

        // SDHCI clock formula: f = base_clock / (2 * divider)
        // So divider = base_clock / (2 * f)
        let base_hz = if self.base_clock > 0 {
            self.base_clock
        } else {
            41666000  // Default ~41.67 MHz
        };

        let target_hz = freq_khz * 1000;
        let mut divider = (base_hz + target_hz - 1) / (2 * target_hz);  // Round up

        if divider > 0x3FF {
            divider = 0x3FF;
        }
        if divider == 0 {
            divider = 1;  // Minimum divider
        }

        // Set clock divider
        let div_lo = (divider & 0xFF) << 8;
        let div_hi = ((divider >> 8) & 0x3) << 6;

        ctrl1 = mmio_read(EMMC_CONTROL1);
        ctrl1 &= 0xFFFF001F;
        ctrl1 |= div_lo | div_hi;
        ctrl1 |= 0x000E0000;  // Data timeout = max
        ctrl1 |= EMMC_CTRL1_CLK_INTLEN;
        mmio_write(EMMC_CONTROL1, ctrl1);

        // Wait for clock stable
        let mut timeout = 10000u32;
        while (mmio_read(EMMC_CONTROL1) & EMMC_CTRL1_CLK_STABLE) == 0 {
            delay_us(10);
            timeout -= 1;
            if timeout == 0 {
                return Err("Clock not stable");
            }
        }

        // Enable SD clock
        ctrl1 = mmio_read(EMMC_CONTROL1);
        ctrl1 |= EMMC_CTRL1_CLK_EN;
        mmio_write(EMMC_CONTROL1, ctrl1);
        delay_us(100);

        Ok(())
    }

    fn wait_for_cmd(&self, timeout_us: u32) -> Result<(), &'static str> {
        let mut count = 0u32;
        loop {
            let irq = mmio_read(EMMC_INTERRUPT);

            if (irq & EMMC_INT_CMD_DONE) != 0 {
                mmio_write(EMMC_INTERRUPT, EMMC_INT_CMD_DONE);
                return Ok(());
            }

            if (irq & EMMC_INT_CMD_TIMEOUT) != 0 {
                mmio_write(EMMC_INTERRUPT, irq);
                return Err("CMD timeout");
            }

            if (irq & EMMC_INT_ERR_MASK) != 0 {
                mmio_write(EMMC_INTERRUPT, irq);
                return Err("CMD error");
            }

            delay_us(10);
            count += 10;
            if count > timeout_us {
                return Err("Wait timeout");
            }
        }
    }

    fn send_cmd(&mut self, cmd_idx: u32, arg: u32, resp_type: u32) -> Result<u32, &'static str> {
        // Wait for command inhibit to clear
        let mut timeout = 100000u32;
        while (mmio_read(EMMC_STATUS) & EMMC_STATUS_CMD_INHIBIT) != 0 {
            delay_us(10);
            timeout -= 10;
            if timeout == 0 {
                return Err("CMD inhibit");
            }
        }

        self.clear_interrupt();

        mmio_write(EMMC_ARG1, arg);
        mmio_write(EMMC_CMDTM, (cmd_idx << 24) | resp_type);

        self.wait_for_cmd(100000)?;

        Ok(mmio_read(EMMC_RESP0))
    }

    fn send_app_cmd(&mut self, cmd_idx: u32, arg: u32, resp_type: u32) -> Result<u32, &'static str> {
        self.send_cmd(55, self.rca, EMMC_CMD_RSPNS_48)?;
        self.send_cmd(cmd_idx, arg, resp_type)
    }

    fn init(&mut self, con: &mut Console) -> Result<(), &'static str> {
        // Step 1: Configure GPIO for SD card with pull-ups
        con.println("  Configuring GPIO 48-53...");
        configure_gpio_for_sd();

        // Step 2: Enable SD card power via mailbox
        con.println("  Enabling SD power...");
        if set_power_state(POWER_ID_SD, true) {
            con.set_color(GREEN, DARK_BLUE);
            con.println("    Power ON");
            con.set_color(WHITE, DARK_BLUE);
        } else {
            con.set_color(YELLOW, DARK_BLUE);
            con.println("    Power status unclear");
            con.set_color(WHITE, DARK_BLUE);
        }

        // Step 3: Get EMMC clock from GPU
        let emmc_clock = get_clock_rate(CLOCK_ID_EMMC);
        self.base_clock = emmc_clock;
        let _ = write!(con, "  EMMC clock: {} Hz\n", emmc_clock);

        if emmc_clock == 0 {
            con.println("  Setting EMMC clock to 50MHz...");
            let new_rate = set_clock_rate(CLOCK_ID_EMMC, 50000000);
            self.base_clock = new_rate;
            let _ = write!(con, "    Got: {} Hz\n", new_rate);
        }

        // Wait for power to stabilize
        con.println("  Waiting for power stable...");
        delay_ms(100);

        // Step 4: Reset controller
        con.println("  Resetting controller...");
        self.reset_controller()?;

        // Step 5: Setup clock at 400kHz for identification
        con.println("  Setting up clock (400kHz)...");
        self.setup_clock(400)?;

        self.clear_interrupt();

        let ctrl1 = mmio_read(EMMC_CONTROL1);
        let status = mmio_read(EMMC_STATUS);
        let _ = write!(con, "  CTRL1=0x{:08X} STATUS=0x{:08X}\n", ctrl1, status);

        // Wait after clock setup
        delay_ms(50);

        // Send CMD0 with proper completion wait
        con.println("  CMD0 (GO_IDLE)...");
        for i in 0..3 {
            self.clear_interrupt();

            // Wait for any previous command to complete
            let mut wait = 10000u32;
            while (mmio_read(EMMC_STATUS) & EMMC_STATUS_CMD_INHIBIT) != 0 && wait > 0 {
                delay_us(10);
                wait -= 1;
            }

            if wait == 0 {
                // Reset CMD line if stuck
                let _ = write!(con, "    CMD line stuck, resetting...\n");
                let mut ctrl1 = mmio_read(EMMC_CONTROL1);
                ctrl1 |= EMMC_CTRL1_SRST_CMD;
                mmio_write(EMMC_CONTROL1, ctrl1);
                delay_ms(10);
                // Wait for reset to complete
                while (mmio_read(EMMC_CONTROL1) & EMMC_CTRL1_SRST_CMD) != 0 {
                    delay_us(10);
                }
                self.clear_interrupt();
            }

            mmio_write(EMMC_ARG1, 0);
            mmio_write(EMMC_CMDTM, 0 << 24);  // CMD0, no response

            // Wait for CMD0 to actually complete (check CMD_DONE or timeout in interrupt)
            let mut cmd_wait = 10000u32;
            loop {
                let irq = mmio_read(EMMC_INTERRUPT);
                // CMD0 has no response, so we just wait for any activity or timeout
                if irq != 0 {
                    self.clear_interrupt();
                    break;
                }
                let status = mmio_read(EMMC_STATUS);
                if (status & EMMC_STATUS_CMD_INHIBIT) == 0 {
                    break;
                }
                delay_us(10);
                cmd_wait -= 1;
                if cmd_wait == 0 {
                    break;
                }
            }

            if i < 2 {
                delay_ms(10);
            }
        }

        self.clear_interrupt();
        delay_ms(50);

        // Check status after CMD0
        let post_cmd0_status = mmio_read(EMMC_STATUS);
        let _ = write!(con, "    Post-CMD0 STATUS: 0x{:08X}\n", post_cmd0_status);

        if (post_cmd0_status & EMMC_STATUS_CMD_INHIBIT) != 0 {
            con.set_color(RED, DARK_BLUE);
            con.println("    CMD line still stuck!");
            con.set_color(WHITE, DARK_BLUE);

            // Force reset CMD line
            let mut ctrl1 = mmio_read(EMMC_CONTROL1);
            ctrl1 |= EMMC_CTRL1_SRST_CMD;
            mmio_write(EMMC_CONTROL1, ctrl1);
            delay_ms(10);
            while (mmio_read(EMMC_CONTROL1) & EMMC_CTRL1_SRST_CMD) != 0 {
                delay_us(10);
            }
            self.clear_interrupt();
            delay_ms(10);

            let final_status = mmio_read(EMMC_STATUS);
            let _ = write!(con, "    After reset STATUS: 0x{:08X}\n", final_status);
        }

        // CMD8: SEND_IF_COND
        con.println("  CMD8 (SEND_IF_COND)...");

        // Show status before CMD8
        let pre_status = mmio_read(EMMC_STATUS);
        let _ = write!(con, "    Pre-STATUS: 0x{:08X}\n", pre_status);

        match self.send_cmd(8, 0x000001AA, EMMC_CMD_RSPNS_48 | EMMC_CMD_CRCCHK_EN) {
            Ok(resp) => {
                self.is_sdhc = (resp & 0xFF) == 0xAA;
                let _ = write!(con, "    Response: 0x{:08X} ", resp);
                if self.is_sdhc {
                    con.set_color(GREEN, DARK_BLUE);
                    con.println("(SDHC)");
                    con.set_color(WHITE, DARK_BLUE);
                } else {
                    con.println("(SD v1)");
                }
            }
            Err(e) => {
                // Show interrupt register for debugging
                let irq = mmio_read(EMMC_INTERRUPT);
                let _ = write!(con, "    INT: 0x{:08X} ({})\n", irq, e);
                self.is_sdhc = false;
                self.clear_interrupt();
            }
        }

        // ACMD41 loop
        con.println("  ACMD41 loop...");
        let mut attempts = 0u32;
        loop {
            // CMD55 (APP_CMD)
            let _ = self.send_cmd(55, 0, EMMC_CMD_RSPNS_48);

            // ACMD41 - for SDv1 cards, don't set HCS
            let hcs = if self.is_sdhc { 0x40000000 } else { 0 };
            let arg = 0x00FF8000 | hcs;

            match self.send_cmd(41, arg, EMMC_CMD_RSPNS_48) {
                Ok(ocr) => {
                    if (ocr & 0x80000000) != 0 {
                        self.is_sdhc = (ocr & 0x40000000) != 0;
                        let _ = write!(con, "    OCR: 0x{:08X} ", ocr);
                        if self.is_sdhc {
                            con.set_color(GREEN, DARK_BLUE);
                            con.println("SDHC!");
                        } else {
                            con.println("SDSC!");
                        }
                        con.set_color(WHITE, DARK_BLUE);
                        break;
                    }
                }
                Err(_) => {
                    self.clear_interrupt();
                }
            }

            attempts += 1;
            if attempts > 50 {
                let irq = mmio_read(EMMC_INTERRUPT);
                let _ = write!(con, "    Final INT: 0x{:08X}\n", irq);
                return Err("ACMD41 timeout");
            }
            if attempts % 10 == 0 {
                let _ = write!(con, "    attempt {}...\n", attempts);
            }
            delay_ms(50);
        }

        // CMD2: ALL_SEND_CID
        con.println("  CMD2 (ALL_SEND_CID)...");
        self.send_cmd(2, 0, EMMC_CMD_RSPNS_136)?;

        // CMD3: SEND_RELATIVE_ADDR
        con.println("  CMD3 (SEND_RELATIVE_ADDR)...");
        let resp = self.send_cmd(3, 0, EMMC_CMD_RSPNS_48)?;
        self.rca = resp & 0xFFFF0000;
        let _ = write!(con, "    RCA: 0x{:04X}\n", self.rca >> 16);

        // CMD7: SELECT_CARD
        con.println("  CMD7 (SELECT_CARD)...");
        self.send_cmd(7, self.rca, EMMC_CMD_RSPNS_48B)?;

        // Speed up clock
        con.println("  Switching to 25MHz...");
        self.setup_clock(25000)?;

        if !self.is_sdhc {
            self.send_cmd(16, 512, EMMC_CMD_RSPNS_48)?;
        }

        self.initialized = true;
        Ok(())
    }

    fn read_sector(&mut self, lba: u32, buffer: &mut [u8; 512]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Not initialized");
        }

        let mut timeout = 100000u32;
        while (mmio_read(EMMC_STATUS) & EMMC_STATUS_DAT_INHIBIT) != 0 {
            delay_us(10);
            timeout -= 10;
            if timeout == 0 {
                return Err("Data inhibit");
            }
        }

        mmio_write(EMMC_BLKSIZECNT, (1 << 16) | 512);
        self.clear_interrupt();

        let addr = if self.is_sdhc { lba } else { lba * 512 };
        mmio_write(EMMC_ARG1, addr);
        mmio_write(EMMC_CMDTM, (17 << 24) | EMMC_CMD_RSPNS_48 | EMMC_CMD_ISDATA | EMMC_CMD_DATA_READ | EMMC_CMD_CRCCHK_EN);

        self.wait_for_cmd(100000)?;

        let mut idx = 0usize;
        let mut data_timeout = 500000u32;

        while idx < 512 {
            let irq = mmio_read(EMMC_INTERRUPT);

            if (irq & EMMC_INT_READ_RDY) != 0 {
                mmio_write(EMMC_INTERRUPT, EMMC_INT_READ_RDY);

                let word = mmio_read(EMMC_DATA);
                buffer[idx] = (word >> 0) as u8;
                buffer[idx + 1] = (word >> 8) as u8;
                buffer[idx + 2] = (word >> 16) as u8;
                buffer[idx + 3] = (word >> 24) as u8;
                idx += 4;
                data_timeout = 500000;
            } else if (irq & EMMC_INT_DATA_DONE) != 0 {
                mmio_write(EMMC_INTERRUPT, EMMC_INT_DATA_DONE);
                break;
            } else if (irq & EMMC_INT_ERR_MASK) != 0 {
                mmio_write(EMMC_INTERRUPT, irq);
                return Err("Data error");
            }

            delay_us(1);
            data_timeout -= 1;
            if data_timeout == 0 {
                return Err("Data timeout");
            }
        }

        Ok(())
    }
}

// ============================================================================
// System Info
// ============================================================================

fn get_arm_memory() -> (u32, u32) {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = 0x0001_0005;
    mbox.data[3] = 8;
    mbox.data[4] = 0;
    mbox.data[5] = 0;
    mbox.data[6] = 0;
    mbox.data[7] = 0;

    if mailbox_call(&mut mbox, 8) {
        (mbox.data[5], mbox.data[6])
    } else {
        (0, 0)
    }
}

fn get_vc_memory() -> (u32, u32) {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = 0x0001_0006;
    mbox.data[3] = 8;
    mbox.data[4] = 0;
    mbox.data[5] = 0;
    mbox.data[6] = 0;
    mbox.data[7] = 0;

    if mailbox_call(&mut mbox, 8) {
        (mbox.data[5], mbox.data[6])
    } else {
        (0, 0)
    }
}

fn get_board_revision() -> u32 {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 7 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = 0x0001_0002;
    mbox.data[3] = 4;
    mbox.data[4] = 0;
    mbox.data[5] = 0;
    mbox.data[6] = 0;

    if mailbox_call(&mut mbox, 8) {
        mbox.data[5]
    } else {
        0
    }
}

fn get_serial() -> u64 {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = 0x0001_0004;
    mbox.data[3] = 8;
    mbox.data[4] = 0;
    mbox.data[5] = 0;
    mbox.data[6] = 0;
    mbox.data[7] = 0;

    if mailbox_call(&mut mbox, 8) {
        ((mbox.data[6] as u64) << 32) | (mbox.data[5] as u64)
    } else {
        0
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[no_mangle]
pub extern "C" fn boot_main() -> ! {
    configure_gpio_for_dpi();

    let fb = match Framebuffer::new() {
        Some(f) => f,
        None => {
            loop {
                unsafe { core::arch::asm!("wfe") };
            }
        }
    };

    fb.clear(DARK_BLUE);

    // Draw border
    fb.fill_rect(0, 0, fb.width, 4, CYAN);
    fb.fill_rect(0, fb.height - 4, fb.width, 4, CYAN);
    fb.fill_rect(0, 0, 4, fb.height, CYAN);
    fb.fill_rect(fb.width - 4, 0, 4, fb.height, CYAN);

    let mut con = Console::new(&fb, WHITE, DARK_BLUE);

    // Title
    con.set_color(CYAN, DARK_BLUE);
    con.println("=== GB-OS System Diagnostics ===");
    con.newline();

    // System info (condensed)
    con.set_color(YELLOW, DARK_BLUE);
    con.println("System Information:");
    con.set_color(WHITE, DARK_BLUE);

    let revision = get_board_revision();
    let serial = get_serial();
    let (_arm_base, arm_size) = get_arm_memory();
    let (_vc_base, vc_size) = get_vc_memory();

    let _ = write!(con, "  Rev: 0x{:08X}  Serial: {:08X}\n", revision, serial as u32);
    let _ = write!(con, "  ARM: {}MB  VC: {}MB  FB: {}x{}\n",
                   arm_size / 1024 / 1024, vc_size / 1024 / 1024,
                   fb.width, fb.height);
    con.newline();

    // Show BOTH controllers
    con.set_color(YELLOW, DARK_BLUE);
    con.println("SD Controllers:");
    con.set_color(WHITE, DARK_BLUE);

    // SDHOST registers
    let _ = write!(con, "  SDHOST: EDM=0x{:08X} HSTS=0x{:08X}\n",
                   mmio_read(SDHOST_EDM), mmio_read(SDHOST_HSTS));

    // EMMC registers
    let emmc_status = mmio_read(EMMC_STATUS);
    let emmc_ctrl1 = mmio_read(EMMC_CONTROL1);
    let emmc_slotisr = mmio_read(EMMC_SLOTISR_VER);
    let _ = write!(con, "  EMMC: STATUS=0x{:08X} CTRL1=0x{:08X}\n",
                   emmc_status, emmc_ctrl1);
    let _ = write!(con, "  EMMC: SLOT=0x{:08X}\n", emmc_slotisr);

    // Decode slot version
    let vendor = (emmc_slotisr >> 24) & 0xFF;
    let sdver = (emmc_slotisr >> 16) & 0xFF;
    let slot_status = emmc_slotisr & 0xFF;
    let _ = write!(con, "    Vendor:{} SDVer:{} Slot:0x{:02X}\n", vendor, sdver, slot_status);

    con.newline();

    // Try SDHOST first (common on Pi Zero 2W for SD card)
    con.set_color(YELLOW, DARK_BLUE);
    con.println("SD Card (SDHOST Controller):");
    con.set_color(WHITE, DARK_BLUE);

    let mut sdhost = SdhostSd::new();
    let mut sd_success = false;
    let mut is_sdhc = false;
    let mut rca = 0u32;

    match sdhost.init(&mut con) {
        Ok(()) => {
            sd_success = true;
            is_sdhc = sdhost.is_sdhc;
            rca = sdhost.rca;

            con.newline();
            con.set_color(GREEN, DARK_BLUE);
            con.println("=== SDHOST: SD Card Ready! ===");
            con.set_color(WHITE, DARK_BLUE);

            let card_type = if is_sdhc { "SDHC" } else { "SDSC" };
            let _ = write!(con, "  Type: {}  RCA: 0x{:04X}\n", card_type, rca >> 16);
            con.newline();

            // Read sector 0
            con.set_color(CYAN, DARK_BLUE);
            con.println("Reading sector 0 (MBR)...");
            con.set_color(WHITE, DARK_BLUE);

            let mut buffer = [0u8; 512];
            match sdhost.read_sector(0, &mut buffer) {
                Ok(()) => {
                    con.set_color(GREEN, DARK_BLUE);
                    con.println("Read successful!");
                    con.set_color(WHITE, DARK_BLUE);

                    con.print("  ");
                    for i in 0..16 {
                        let _ = write!(con, "{:02X} ", buffer[i]);
                    }
                    con.newline();

                    if buffer[510] == 0x55 && buffer[511] == 0xAA {
                        con.set_color(GREEN, DARK_BLUE);
                        con.println("  Valid MBR (0x55AA)");
                        con.set_color(WHITE, DARK_BLUE);

                        let part_type = buffer[0x1BE + 4];
                        let part_start = u32::from_le_bytes([
                            buffer[0x1BE + 8], buffer[0x1BE + 9],
                            buffer[0x1BE + 10], buffer[0x1BE + 11],
                        ]);
                        let part_size = u32::from_le_bytes([
                            buffer[0x1BE + 12], buffer[0x1BE + 13],
                            buffer[0x1BE + 14], buffer[0x1BE + 15],
                        ]);
                        let _ = write!(con, "  Part0: type=0x{:02X} start={} size={}MB\n",
                                       part_type, part_start, part_size / 2048);
                    }
                }
                Err(e) => {
                    con.set_color(RED, DARK_BLUE);
                    let _ = write!(con, "Read error: {}\n", e);
                }
            }
        }
        Err(e) => {
            con.set_color(YELLOW, DARK_BLUE);
            let _ = write!(con, "SDHOST failed: {}\n", e);
            con.set_color(WHITE, DARK_BLUE);
        }
    }

    // If SDHOST failed, try EMMC
    if !sd_success {
        con.newline();
        con.set_color(YELLOW, DARK_BLUE);
        con.println("Trying EMMC Controller...");
        con.set_color(WHITE, DARK_BLUE);

        let mut emmc = EmmcSd::new();

        match emmc.init(&mut con) {
            Ok(()) => {
                con.newline();
                con.set_color(GREEN, DARK_BLUE);
                con.println("=== EMMC: SD Card Ready! ===");
                con.set_color(WHITE, DARK_BLUE);

                let card_type = if emmc.is_sdhc { "SDHC" } else { "SDSC" };
                let _ = write!(con, "  Type: {}  RCA: 0x{:04X}\n", card_type, emmc.rca >> 16);
                con.newline();

                con.set_color(CYAN, DARK_BLUE);
                con.println("Reading sector 0 (MBR)...");
                con.set_color(WHITE, DARK_BLUE);

                let mut buffer = [0u8; 512];
                match emmc.read_sector(0, &mut buffer) {
                    Ok(()) => {
                        con.set_color(GREEN, DARK_BLUE);
                        con.println("Read successful!");
                        con.set_color(WHITE, DARK_BLUE);

                        con.print("  ");
                        for i in 0..16 {
                            let _ = write!(con, "{:02X} ", buffer[i]);
                        }
                        con.newline();

                        if buffer[510] == 0x55 && buffer[511] == 0xAA {
                            con.set_color(GREEN, DARK_BLUE);
                            con.println("  Valid MBR (0x55AA)");
                        }
                    }
                    Err(e) => {
                        con.set_color(RED, DARK_BLUE);
                        let _ = write!(con, "Read error: {}\n", e);
                    }
                }
            }
            Err(e) => {
                con.set_color(RED, DARK_BLUE);
                let _ = write!(con, "EMMC also failed: {}\n", e);
            }
        }
    }

    // Done
    con.newline();
    con.set_color(GRAY, DARK_BLUE);
    con.println("Diagnostics complete.");

    loop {
        unsafe { core::arch::asm!("wfe") };
    }
}

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("wfe") };
    }
}
