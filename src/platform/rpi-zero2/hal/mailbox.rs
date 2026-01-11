//! VideoCore Mailbox Interface
//!
//! The mailbox is used to communicate with the VideoCore GPU/firmware
//! for operations like:
//! - Framebuffer allocation
//! - Power management
//! - Clock configuration
//! - Hardware queries

use crate::platform_core::mmio::{mmio_read, mmio_write, PERIPHERAL_BASE};

// ============================================================================
// Mailbox Registers
// ============================================================================

const MBOX_BASE: usize = PERIPHERAL_BASE + 0x0000_B880;

const MBOX_READ: usize = MBOX_BASE + 0x00;
const MBOX_STATUS: usize = MBOX_BASE + 0x18;
const MBOX_WRITE: usize = MBOX_BASE + 0x20;

// Status register bits
const MBOX_FULL: u32 = 0x8000_0000;
const MBOX_EMPTY: u32 = 0x4000_0000;

// Response codes
const MBOX_RESPONSE_SUCCESS: u32 = 0x8000_0000;

// ============================================================================
// Mailbox Buffer
// ============================================================================

/// 16-byte aligned mailbox buffer for property tag interface
#[repr(C, align(16))]
pub struct MailboxBuffer {
    pub data: [u32; 64],
}

impl MailboxBuffer {
    /// Create a new zeroed mailbox buffer
    pub const fn new() -> Self {
        Self { data: [0; 64] }
    }

    /// Clear the buffer to zeros
    pub fn clear(&mut self) {
        self.data = [0; 64];
    }
}

// ============================================================================
// Mailbox Operations
// ============================================================================

/// Send a mailbox message and wait for response
///
/// The buffer must be 16-byte aligned. Channel 8 is the property tag channel.
///
/// Returns true if the call was successful (response code 0x80000000)
pub fn mailbox_call(buffer: &mut MailboxBuffer, channel: u8) -> bool {
    let addr = buffer.data.as_ptr() as u32;

    // Wait for mailbox to be not full
    while (mmio_read(MBOX_STATUS) & MBOX_FULL) != 0 {
        core::hint::spin_loop();
    }

    // Write address (with channel in low 4 bits)
    mmio_write(MBOX_WRITE, (addr & !0xF) | (channel as u32 & 0xF));

    // Wait for response
    loop {
        // Wait for mailbox to be not empty
        while (mmio_read(MBOX_STATUS) & MBOX_EMPTY) != 0 {
            core::hint::spin_loop();
        }

        // Read response
        let response = mmio_read(MBOX_READ);

        // Check if this is our response (matching channel)
        if (response & 0xF) == channel as u32 {
            // Check if successful
            return buffer.data[1] == MBOX_RESPONSE_SUCCESS;
        }
    }
}

// ============================================================================
// Property Tags
// ============================================================================

/// Mailbox property tags
pub mod tags {
    // VideoCore
    pub const GET_FIRMWARE_REV: u32 = 0x0000_0001;

    // Hardware
    pub const GET_BOARD_MODEL: u32 = 0x0001_0001;
    pub const GET_BOARD_REV: u32 = 0x0001_0002;
    pub const GET_BOARD_MAC: u32 = 0x0001_0003;
    pub const GET_BOARD_SERIAL: u32 = 0x0001_0004;
    pub const GET_ARM_MEMORY: u32 = 0x0001_0005;
    pub const GET_VC_MEMORY: u32 = 0x0001_0006;

    // Power
    pub const GET_POWER_STATE: u32 = 0x0002_0001;
    pub const SET_POWER_STATE: u32 = 0x0002_8001;
    pub const WAIT_FOR_VSYNC: u32 = 0x0002_0014;

    // Clocks
    pub const GET_CLOCK_STATE: u32 = 0x0003_0001;
    pub const SET_CLOCK_STATE: u32 = 0x0003_8001;
    pub const GET_CLOCK_RATE: u32 = 0x0003_0002;
    pub const SET_CLOCK_RATE: u32 = 0x0003_8002;

    // Framebuffer
    pub const ALLOCATE_BUFFER: u32 = 0x0004_0001;
    pub const RELEASE_BUFFER: u32 = 0x0004_8001;
    pub const BLANK_SCREEN: u32 = 0x0004_0002;
    pub const GET_PHYSICAL_SIZE: u32 = 0x0004_0003;
    pub const SET_PHYSICAL_SIZE: u32 = 0x0004_8003;
    pub const GET_VIRTUAL_SIZE: u32 = 0x0004_0004;
    pub const SET_VIRTUAL_SIZE: u32 = 0x0004_8004;
    pub const GET_DEPTH: u32 = 0x0004_0005;
    pub const SET_DEPTH: u32 = 0x0004_8005;
    pub const GET_PIXEL_ORDER: u32 = 0x0004_0006;
    pub const SET_PIXEL_ORDER: u32 = 0x0004_8006;
    pub const GET_PITCH: u32 = 0x0004_0008;
    pub const SET_VIRTUAL_OFFSET: u32 = 0x0004_8009;
}

/// Device IDs for power management
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
// Power Management
// ============================================================================

/// Set power state for a device
///
/// Returns true if the device is now in the requested state
pub fn set_power_state(device_id: u32, on: bool) -> bool {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;           // Total size
    mbox.data[1] = 0;               // Request code
    mbox.data[2] = tags::SET_POWER_STATE;
    mbox.data[3] = 8;               // Value buffer size
    mbox.data[4] = 8;               // Request size
    mbox.data[5] = device_id;       // Device ID
    mbox.data[6] = if on { 3 } else { 0 }; // State: on + wait
    mbox.data[7] = 0;               // End tag

    mailbox_call(&mut mbox, 8) && (mbox.data[6] & 1) != 0
}

/// Get power state for a device
///
/// Returns (exists, is_on)
pub fn get_power_state(device_id: u32) -> (bool, bool) {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = tags::GET_POWER_STATE;
    mbox.data[3] = 8;
    mbox.data[4] = 0;
    mbox.data[5] = device_id;
    mbox.data[6] = 0;
    mbox.data[7] = 0;

    if mailbox_call(&mut mbox, 8) {
        let exists = (mbox.data[6] & 2) == 0;
        let is_on = (mbox.data[6] & 1) != 0;
        (exists, is_on)
    } else {
        (false, false)
    }
}

// ============================================================================
// Hardware Queries
// ============================================================================

/// Get ARM memory base and size
pub fn get_arm_memory() -> Option<(u32, u32)> {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = tags::GET_ARM_MEMORY;
    mbox.data[3] = 8;
    mbox.data[4] = 0;
    mbox.data[5] = 0;
    mbox.data[6] = 0;
    mbox.data[7] = 0;

    if mailbox_call(&mut mbox, 8) {
        Some((mbox.data[5], mbox.data[6]))
    } else {
        None
    }
}

/// Get VideoCore memory base and size
pub fn get_vc_memory() -> Option<(u32, u32)> {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = tags::GET_VC_MEMORY;
    mbox.data[3] = 8;
    mbox.data[4] = 0;
    mbox.data[5] = 0;
    mbox.data[6] = 0;
    mbox.data[7] = 0;

    if mailbox_call(&mut mbox, 8) {
        Some((mbox.data[5], mbox.data[6]))
    } else {
        None
    }
}

/// Get board serial number
pub fn get_board_serial() -> Option<u64> {
    let mut mbox = MailboxBuffer::new();

    mbox.data[0] = 8 * 4;
    mbox.data[1] = 0;
    mbox.data[2] = tags::GET_BOARD_SERIAL;
    mbox.data[3] = 8;
    mbox.data[4] = 0;
    mbox.data[5] = 0;
    mbox.data[6] = 0;
    mbox.data[7] = 0;

    if mailbox_call(&mut mbox, 8) {
        Some(((mbox.data[6] as u64) << 32) | (mbox.data[5] as u64))
    } else {
        None
    }
}
