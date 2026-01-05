//! VideoCore Mailbox Interface
//!
//! The mailbox is used to communicate with the VideoCore GPU for:
//! - Framebuffer allocation
//! - Power management
//! - Clock configuration
//! - Hardware information queries
//!
//! # Protocol
//!
//! 1. Build message buffer (16-byte aligned)
//! 2. Write buffer address | channel to MBOX_WRITE
//! 3. Wait for response on MBOX_READ
//! 4. Check response code in buffer

use crate::mmio::{self, PERIPHERAL_BASE};

// ============================================================================
// Register Addresses
// ============================================================================

const MBOX_BASE: usize = PERIPHERAL_BASE + 0x0000_B880;

const MBOX_READ: usize = MBOX_BASE + 0x00;
const MBOX_STATUS: usize = MBOX_BASE + 0x18;
const MBOX_WRITE: usize = MBOX_BASE + 0x20;

// Status bits
const MBOX_FULL: u32 = 0x8000_0000;
const MBOX_EMPTY: u32 = 0x4000_0000;

// Response codes
const RESPONSE_SUCCESS: u32 = 0x8000_0000;

// ============================================================================
// Mailbox Channels
// ============================================================================

/// Mailbox channel numbers.
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Channel {
    Power = 0,
    Framebuffer = 1,
    VirtualUart = 2,
    Vchiq = 3,
    Leds = 4,
    Buttons = 5,
    Touchscreen = 6,
    PropertyArmToVc = 8,
    PropertyVcToArm = 9,
}

// ============================================================================
// Property Tags
// ============================================================================

/// Property tag IDs for mailbox requests.
pub mod tag {
    // Hardware info
    pub const GET_FIRMWARE_REV: u32 = 0x0000_0001;
    pub const GET_BOARD_MODEL: u32 = 0x0001_0001;
    pub const GET_BOARD_REV: u32 = 0x0001_0002;
    pub const GET_BOARD_SERIAL: u32 = 0x0001_0004;
    pub const GET_ARM_MEMORY: u32 = 0x0001_0005;
    pub const GET_VC_MEMORY: u32 = 0x0001_0006;

    // Power
    pub const GET_POWER_STATE: u32 = 0x0002_0001;
    pub const SET_POWER_STATE: u32 = 0x0002_8001;

    // Clocks
    pub const GET_CLOCK_RATE: u32 = 0x0003_0002;
    pub const SET_CLOCK_RATE: u32 = 0x0003_8002;

    // Framebuffer
    pub const ALLOCATE_BUFFER: u32 = 0x0004_0001;
    pub const RELEASE_BUFFER: u32 = 0x0004_8001;
    pub const SET_PHYSICAL_SIZE: u32 = 0x0004_8003;
    pub const SET_VIRTUAL_SIZE: u32 = 0x0004_8004;
    pub const SET_DEPTH: u32 = 0x0004_8005;
    pub const SET_PIXEL_ORDER: u32 = 0x0004_8006;
    pub const GET_PITCH: u32 = 0x0004_0008;
    pub const SET_VIRTUAL_OFFSET: u32 = 0x0004_8009;

    pub const END: u32 = 0x0000_0000;
}

/// Power device IDs.
pub mod device {
    pub const SD_CARD: u32 = 0;
    pub const UART0: u32 = 1;
    pub const UART1: u32 = 2;
    pub const USB_HCD: u32 = 3;
    pub const I2C0: u32 = 4;
    pub const I2C1: u32 = 5;
    pub const I2C2: u32 = 6;
    pub const SPI: u32 = 7;
    pub const CCP2TX: u32 = 8;
}

// ============================================================================
// Mailbox Buffer
// ============================================================================

/// 16-byte aligned mailbox buffer.
#[repr(C, align(16))]
pub struct MailboxBuffer {
    pub data: [u32; 64],
}

impl MailboxBuffer {
    pub const fn new() -> Self {
        Self { data: [0; 64] }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.data.fill(0);
    }
}

// ============================================================================
// Low-Level Mailbox Access
// ============================================================================

/// Send a message to the mailbox and wait for response.
///
/// # Arguments
/// * `buffer` - 16-byte aligned buffer with message
/// * `channel` - Mailbox channel
///
/// # Returns
/// `true` if the response indicates success.
pub fn call(buffer: &mut MailboxBuffer, channel: Channel) -> bool {
    let addr = buffer.data.as_ptr() as u32;

    // Wait for mailbox to have space
    while (mmio::read(MBOX_STATUS) & MBOX_FULL) != 0 {
        core::hint::spin_loop();
    }

    // Write message (address | channel)
    mmio::write(MBOX_WRITE, (addr & !0xF) | (channel as u32 & 0xF));

    // Wait for response
    loop {
        while (mmio::read(MBOX_STATUS) & MBOX_EMPTY) != 0 {
            core::hint::spin_loop();
        }

        let response = mmio::read(MBOX_READ);
        if (response & 0xF) == channel as u32 {
            break;
        }
    }

    // Check response code
    buffer.data[1] == RESPONSE_SUCCESS
}

// ============================================================================
// High-Level API
// ============================================================================

/// Set power state for a device.
///
/// # Arguments
/// * `device_id` - Device ID from `device` module
/// * `on` - `true` to power on, `false` to power off
///
/// # Returns
/// `true` if the device is now in the requested state.
pub fn set_power_state(device_id: u32, on: bool) -> bool {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;              // Buffer size
    mbox.data[1] = 0;                  // Request code
    mbox.data[2] = tag::SET_POWER_STATE;
    mbox.data[3] = 8;                  // Value buffer size
    mbox.data[4] = 8;                  // Request size
    mbox.data[5] = device_id;
    mbox.data[6] = if on { 3 } else { 0 };  // State (bit 0 = on, bit 1 = wait)
    mbox.data[7] = tag::END;

    call(&mut mbox, Channel::PropertyArmToVc) && (mbox.data[6] & 1) != 0
}

/// Get ARM memory base and size.
pub fn get_arm_memory() -> Option<(u32, u32)> {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = tag::GET_ARM_MEMORY;
    mbox.data[3] = 8;
    mbox.data[4] = 0;
    mbox.data[5] = 0;
    mbox.data[6] = 0;
    mbox.data[7] = tag::END;

    if call(&mut mbox, Channel::PropertyArmToVc) {
        Some((mbox.data[5], mbox.data[6]))
    } else {
        None
    }
}

/// Get VideoCore memory base and size.
pub fn get_vc_memory() -> Option<(u32, u32)> {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = tag::GET_VC_MEMORY;
    mbox.data[3] = 8;
    mbox.data[4] = 0;
    mbox.data[5] = 0;
    mbox.data[6] = 0;
    mbox.data[7] = tag::END;

    if call(&mut mbox, Channel::PropertyArmToVc) {
        Some((mbox.data[5], mbox.data[6]))
    } else {
        None
    }
}

/// Get board revision.
pub fn get_board_revision() -> Option<u32> {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 7 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = tag::GET_BOARD_REV;
    mbox.data[3] = 4;
    mbox.data[4] = 0;
    mbox.data[5] = 0;
    mbox.data[6] = tag::END;

    if call(&mut mbox, Channel::PropertyArmToVc) {
        Some(mbox.data[5])
    } else {
        None
    }
}

// ============================================================================
// Framebuffer Allocation
// ============================================================================

/// Framebuffer information returned from allocation.
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    pub addr: u32,
    pub size: u32,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub depth: u32,
}

/// Allocate a framebuffer with the specified parameters.
///
/// # Arguments
/// * `width` - Display width in pixels
/// * `height` - Display height in pixels
/// * `depth` - Bits per pixel (16, 24, or 32)
///
/// # Returns
/// `Some(FramebufferInfo)` on success, `None` on failure.
pub fn allocate_framebuffer(width: u32, height: u32, depth: u32) -> Option<FramebufferInfo> {
    let mut mbox = MailboxBuffer::new();

    // Build multi-tag request
    mbox.data[0] = 35 * 4;             // Buffer size
    mbox.data[1] = 0;                  // Request code

    // Set physical size
    mbox.data[2] = tag::SET_PHYSICAL_SIZE;
    mbox.data[3] = 8;
    mbox.data[4] = 8;
    mbox.data[5] = width;
    mbox.data[6] = height;

    // Set virtual size (same as physical for single buffer)
    mbox.data[7] = tag::SET_VIRTUAL_SIZE;
    mbox.data[8] = 8;
    mbox.data[9] = 8;
    mbox.data[10] = width;
    mbox.data[11] = height;

    // Set virtual offset
    mbox.data[12] = tag::SET_VIRTUAL_OFFSET;
    mbox.data[13] = 8;
    mbox.data[14] = 8;
    mbox.data[15] = 0;
    mbox.data[16] = 0;

    // Set depth
    mbox.data[17] = tag::SET_DEPTH;
    mbox.data[18] = 4;
    mbox.data[19] = 4;
    mbox.data[20] = depth;

    // Set pixel order (0 = BGR, 1 = RGB)
    mbox.data[21] = tag::SET_PIXEL_ORDER;
    mbox.data[22] = 4;
    mbox.data[23] = 4;
    mbox.data[24] = 1;                 // RGB

    // Allocate buffer
    mbox.data[25] = tag::ALLOCATE_BUFFER;
    mbox.data[26] = 8;
    mbox.data[27] = 8;
    mbox.data[28] = 16;                // Alignment
    mbox.data[29] = 0;                 // Size (filled by response)

    // Get pitch
    mbox.data[30] = tag::GET_PITCH;
    mbox.data[31] = 4;
    mbox.data[32] = 4;
    mbox.data[33] = 0;

    // End tag
    mbox.data[34] = tag::END;

    if !call(&mut mbox, Channel::PropertyArmToVc) {
        return None;
    }

    // Extract results (convert bus address to ARM physical)
    let fb_addr = mbox.data[28] & 0x3FFF_FFFF;
    let fb_size = mbox.data[29];
    let pitch = mbox.data[33];

    if fb_addr == 0 || fb_size == 0 {
        return None;
    }

    Some(FramebufferInfo {
        addr: fb_addr,
        size: fb_size,
        width,
        height,
        pitch,
        depth,
    })
}
