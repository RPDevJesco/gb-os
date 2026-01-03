//! VideoCore Mailbox Interface
//!
//! The mailbox is the communication channel between ARM and VideoCore GPU.
//! Used for:
//!   - Querying memory configuration
//!   - Setting up framebuffer
//!   - Configuring clocks
//!   - Power management
//!
//! # Protocol
//!
//! 1. Build property buffer with tags
//! 2. Write buffer address (bus address) to mailbox
//! 3. Wait for response
//! 4. Read results from buffer
//!
//! # Buffer Format
//!
//! ```text
//! Offset  Size  Description
//! ──────────────────────────────────
//! 0       4     Buffer size (bytes)
//! 4       4     Request/Response code
//! 8       N     Tags (variable)
//! 8+N     4     End tag (0x0)
//! ```
//!
//! # Tag Format
//!
//! ```text

#![allow(dead_code)]
//! Offset  Size  Description
//! ──────────────────────────────────
//! 0       4     Tag ID
//! 4       4     Value buffer size
//! 8       4     Request/Response size
//! 12      N     Value buffer
//! ```

use crate::mmio;
use crate::memory_map::{self, phys_to_bus};

// ============================================================================
// Mailbox Registers
// ============================================================================

/// Mailbox base address (BCM2710/BCM2837)
const MAILBOX_BASE: usize = memory_map::PERIPHERAL_BASE + 0x0000_B880;

/// Mailbox 0 read register
const MBOX_READ: usize = MAILBOX_BASE + 0x00;

/// Mailbox 0 poll register (unused)
const MBOX_POLL: usize = MAILBOX_BASE + 0x10;

/// Mailbox 0 sender register (unused)
const MBOX_SENDER: usize = MAILBOX_BASE + 0x14;

/// Mailbox 0 status register
const MBOX_STATUS: usize = MAILBOX_BASE + 0x18;

/// Mailbox 0 configuration register (unused)
const MBOX_CONFIG: usize = MAILBOX_BASE + 0x1C;

/// Mailbox 1 write register
const MBOX_WRITE: usize = MAILBOX_BASE + 0x20;

/// Status register bits
mod status {
    /// Mailbox is full (cannot write)
    pub const FULL: u32 = 0x8000_0000;
    /// Mailbox is empty (cannot read)
    pub const EMPTY: u32 = 0x4000_0000;
}

/// Mailbox channels
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Channel {
    /// Power management
    Power = 0,
    /// Framebuffer
    Framebuffer = 1,
    /// Virtual UART
    VirtualUart = 2,
    /// VCHIQ
    Vchiq = 3,
    /// LEDs
    Leds = 4,
    /// Buttons
    Buttons = 5,
    /// Touchscreen
    Touchscreen = 6,
    /// Property tags (ARM → VC)
    PropertyArmToVc = 8,
    /// Property tags (VC → ARM)
    PropertyVcToArm = 9,
}

// ============================================================================
// Request/Response Codes
// ============================================================================

/// Request code (in buffer header)
const REQUEST_CODE: u32 = 0x0000_0000;

/// Response success
const RESPONSE_SUCCESS: u32 = 0x8000_0000;

/// Response error (parsing error)
const RESPONSE_ERROR: u32 = 0x8000_0001;

// ============================================================================
// Property Tags
// ============================================================================

/// Property tag IDs
pub mod tag {
    // VideoCore info
    pub const GET_FIRMWARE_REV: u32 = 0x0000_0001;

    // Hardware info
    pub const GET_BOARD_MODEL: u32 = 0x0001_0001;
    pub const GET_BOARD_REV: u32 = 0x0001_0002;
    pub const GET_BOARD_MAC: u32 = 0x0001_0003;
    pub const GET_BOARD_SERIAL: u32 = 0x0001_0004;
    pub const GET_ARM_MEMORY: u32 = 0x0001_0005;
    pub const GET_VC_MEMORY: u32 = 0x0001_0006;

    // Clocks
    pub const GET_CLOCK_RATE: u32 = 0x0003_0002;
    pub const GET_MAX_CLOCK_RATE: u32 = 0x0003_0004;
    pub const GET_MIN_CLOCK_RATE: u32 = 0x0003_0007;
    pub const SET_CLOCK_RATE: u32 = 0x0003_8002;

    // Framebuffer
    pub const ALLOCATE_BUFFER: u32 = 0x0004_0001;
    pub const RELEASE_BUFFER: u32 = 0x0004_8001;
    pub const BLANK_SCREEN: u32 = 0x0004_0002;
    pub const GET_PHYSICAL_SIZE: u32 = 0x0004_0003;
    pub const TEST_PHYSICAL_SIZE: u32 = 0x0004_4003;
    pub const SET_PHYSICAL_SIZE: u32 = 0x0004_8003;
    pub const GET_VIRTUAL_SIZE: u32 = 0x0004_0004;
    pub const TEST_VIRTUAL_SIZE: u32 = 0x0004_4004;
    pub const SET_VIRTUAL_SIZE: u32 = 0x0004_8004;
    pub const GET_DEPTH: u32 = 0x0004_0005;
    pub const TEST_DEPTH: u32 = 0x0004_4005;
    pub const SET_DEPTH: u32 = 0x0004_8005;
    pub const GET_PIXEL_ORDER: u32 = 0x0004_0006;
    pub const TEST_PIXEL_ORDER: u32 = 0x0004_4006;
    pub const SET_PIXEL_ORDER: u32 = 0x0004_8006;
    pub const GET_ALPHA_MODE: u32 = 0x0004_0007;
    pub const TEST_ALPHA_MODE: u32 = 0x0004_4007;
    pub const SET_ALPHA_MODE: u32 = 0x0004_8007;
    pub const GET_PITCH: u32 = 0x0004_0008;
    pub const GET_VIRTUAL_OFFSET: u32 = 0x0004_0009;
    pub const SET_VIRTUAL_OFFSET: u32 = 0x0004_8009;

    /// End tag (terminates tag list)
    pub const END: u32 = 0x0000_0000;
}

/// Clock IDs for clock rate tags
pub mod clock {
    pub const EMMC: u32 = 0x1;
    pub const UART: u32 = 0x2;
    pub const ARM: u32 = 0x3;
    pub const CORE: u32 = 0x4;
    pub const V3D: u32 = 0x5;
    pub const H264: u32 = 0x6;
    pub const ISP: u32 = 0x7;
    pub const SDRAM: u32 = 0x8;
    pub const PIXEL: u32 = 0x9;
    pub const PWM: u32 = 0xA;
    pub const HEVC: u32 = 0xB;
    pub const EMMC2: u32 = 0xC;
    pub const M2MC: u32 = 0xD;
    pub const PIXEL_BVB: u32 = 0xE;
}

// ============================================================================
// Mailbox Buffer
// ============================================================================

/// Maximum buffer size (must fit in our allocated region)
const MAX_BUFFER_SIZE: usize = 256;

/// Property buffer (16-byte aligned for DMA)
#[repr(C, align(16))]
pub struct PropertyBuffer {
    data: [u32; MAX_BUFFER_SIZE / 4],
}

impl PropertyBuffer {
    /// Create empty buffer
    pub const fn new() -> Self {
        Self {
            data: [0; MAX_BUFFER_SIZE / 4],
        }
    }

    /// Get buffer address
    pub fn as_ptr(&self) -> *const u32 {
        self.data.as_ptr()
    }

    /// Get mutable buffer address
    pub fn as_mut_ptr(&mut self) -> *mut u32 {
        self.data.as_mut_ptr()
    }

    /// Get physical address
    pub fn phys_addr(&self) -> usize {
        self.as_ptr() as usize
    }

    /// Get bus address (for VideoCore DMA)
    pub fn bus_addr(&self) -> u32 {
        phys_to_bus(self.phys_addr())
    }
}

// ============================================================================
// Mailbox Driver
// ============================================================================

/// Mailbox interface
pub struct Mailbox {
    buffer: PropertyBuffer,
}

impl Mailbox {
    /// Create new mailbox interface
    pub const fn new() -> Self {
        Self {
            buffer: PropertyBuffer::new(),
        }
    }

    /// Wait until mailbox is not full
    fn wait_write_ready(&self) {
        loop {
            if (mmio::read(MBOX_STATUS) & status::FULL) == 0 {
                break;
            }
            core::hint::spin_loop();
        }
    }

    /// Wait until mailbox has data
    fn wait_read_ready(&self) {
        loop {
            if (mmio::read(MBOX_STATUS) & status::EMPTY) == 0 {
                break;
            }
            core::hint::spin_loop();
        }
    }

    /// Send buffer to mailbox and wait for response
    fn call(&mut self, channel: Channel) -> Result<(), MailboxError> {
        // Ensure 16-byte alignment
        let addr = self.buffer.phys_addr();
        if addr & 0xF != 0 {
            return Err(MailboxError::NotAligned);
        }

        // Combine address with channel (lower 4 bits)
        let value = (self.buffer.bus_addr() & !0xF) | (channel as u32);

        // Write to mailbox
        self.wait_write_ready();
        mmio::write(MBOX_WRITE, value);

        // Wait for response
        loop {
            self.wait_read_ready();
            let response = mmio::read(MBOX_READ);

            // Check if this response is for our channel
            if (response & 0xF) == (channel as u32) {
                break;
            }
            // Otherwise, keep waiting (response was for different channel)
        }

        // Check response code in buffer
        let response_code = self.buffer.data[1];
        if response_code == RESPONSE_SUCCESS {
            Ok(())
        } else if response_code == RESPONSE_ERROR {
            Err(MailboxError::ResponseError)
        } else {
            Err(MailboxError::InvalidResponse(response_code))
        }
    }

    /// Build and send a single-tag request
    fn single_tag_request(
        &mut self,
        tag_id: u32,
        request: &[u32],
        response_words: usize,
    ) -> Result<(), MailboxError> {
        // Calculate sizes
        let value_size = response_words.max(request.len()) * 4;
        let total_size = 12 + 12 + value_size + 4; // header + tag header + value + end

        // Build buffer
        self.buffer.data[0] = total_size as u32; // Buffer size
        self.buffer.data[1] = REQUEST_CODE;       // Request code

        // Tag
        self.buffer.data[2] = tag_id;             // Tag ID
        self.buffer.data[3] = value_size as u32;  // Value buffer size
        self.buffer.data[4] = 0;                  // Request size (0 = request)

        // Copy request data
        for (i, &val) in request.iter().enumerate() {
            self.buffer.data[5 + i] = val;
        }

        // End tag
        let end_offset = 5 + (value_size / 4);
        self.buffer.data[end_offset] = tag::END;

        // Send request
        self.call(Channel::PropertyArmToVc)
    }

    // ========================================================================
    // High-Level API
    // ========================================================================

    /// Get ARM memory base and size
    pub fn get_arm_memory(&mut self) -> Result<(u32, u32), MailboxError> {
        self.single_tag_request(tag::GET_ARM_MEMORY, &[], 2)?;
        Ok((self.buffer.data[5], self.buffer.data[6]))
    }

    /// Get VideoCore memory base and size
    pub fn get_vc_memory(&mut self) -> Result<(u32, u32), MailboxError> {
        self.single_tag_request(tag::GET_VC_MEMORY, &[], 2)?;
        Ok((self.buffer.data[5], self.buffer.data[6]))
    }

    /// Get board revision
    pub fn get_board_revision(&mut self) -> Result<u32, MailboxError> {
        self.single_tag_request(tag::GET_BOARD_REV, &[], 1)?;
        Ok(self.buffer.data[5])
    }

    /// Get board serial number
    pub fn get_board_serial(&mut self) -> Result<u64, MailboxError> {
        self.single_tag_request(tag::GET_BOARD_SERIAL, &[], 2)?;
        let low = self.buffer.data[5] as u64;
        let high = self.buffer.data[6] as u64;
        Ok((high << 32) | low)
    }

    /// Get clock rate in Hz
    pub fn get_clock_rate(&mut self, clock_id: u32) -> Result<u32, MailboxError> {
        self.single_tag_request(tag::GET_CLOCK_RATE, &[clock_id], 2)?;
        Ok(self.buffer.data[6])
    }

    /// Set clock rate in Hz, returns actual rate
    pub fn set_clock_rate(&mut self, clock_id: u32, rate_hz: u32) -> Result<u32, MailboxError> {
        // Request: clock_id, rate, skip_turbo (0 = don't skip)
        self.single_tag_request(tag::SET_CLOCK_RATE, &[clock_id, rate_hz, 0], 2)?;
        Ok(self.buffer.data[6])
    }

    /// Allocate framebuffer
    /// Returns (base_address, size_bytes)
    pub fn allocate_framebuffer(&mut self, alignment: u32) -> Result<(u32, u32), MailboxError> {
        self.single_tag_request(tag::ALLOCATE_BUFFER, &[alignment], 2)?;
        // Response is bus address - convert to ARM physical
        let bus_addr = self.buffer.data[5];
        let size = self.buffer.data[6];
        Ok((bus_addr & 0x3FFF_FFFF, size))
    }

    /// Set physical (display) size
    pub fn set_physical_size(&mut self, width: u32, height: u32) -> Result<(u32, u32), MailboxError> {
        self.single_tag_request(tag::SET_PHYSICAL_SIZE, &[width, height], 2)?;
        Ok((self.buffer.data[5], self.buffer.data[6]))
    }

    /// Set virtual (framebuffer) size
    pub fn set_virtual_size(&mut self, width: u32, height: u32) -> Result<(u32, u32), MailboxError> {
        self.single_tag_request(tag::SET_VIRTUAL_SIZE, &[width, height], 2)?;
        Ok((self.buffer.data[5], self.buffer.data[6]))
    }

    /// Set color depth in bits per pixel
    pub fn set_depth(&mut self, bits: u32) -> Result<u32, MailboxError> {
        self.single_tag_request(tag::SET_DEPTH, &[bits], 1)?;
        Ok(self.buffer.data[5])
    }

    /// Set pixel order (0 = BGR, 1 = RGB)
    pub fn set_pixel_order(&mut self, rgb: bool) -> Result<u32, MailboxError> {
        self.single_tag_request(tag::SET_PIXEL_ORDER, &[rgb as u32], 1)?;
        Ok(self.buffer.data[5])
    }

    /// Get pitch (bytes per row)
    pub fn get_pitch(&mut self) -> Result<u32, MailboxError> {
        self.single_tag_request(tag::GET_PITCH, &[], 1)?;
        Ok(self.buffer.data[5])
    }

    /// Set virtual offset (for double buffering)
    pub fn set_virtual_offset(&mut self, x: u32, y: u32) -> Result<(u32, u32), MailboxError> {
        self.single_tag_request(tag::SET_VIRTUAL_OFFSET, &[x, y], 2)?;
        Ok((self.buffer.data[5], self.buffer.data[6]))
    }

    // ========================================================================
    // Framebuffer Setup (convenience function)
    // ========================================================================

    /// Initialize framebuffer with given parameters
    pub fn init_framebuffer(
        &mut self,
        width: u32,
        height: u32,
        depth: u32,
    ) -> Result<FramebufferInfo, MailboxError> {
        // Set physical size
        let (phys_w, phys_h) = self.set_physical_size(width, height)?;

        // Set virtual size (same as physical for single buffer)
        let (virt_w, virt_h) = self.set_virtual_size(width, height)?;

        // Set depth
        let actual_depth = self.set_depth(depth)?;

        // Set RGB order
        self.set_pixel_order(true)?; // RGB

        // Set offset to 0,0
        self.set_virtual_offset(0, 0)?;

        // Allocate framebuffer (16-byte aligned)
        let (fb_addr, fb_size) = self.allocate_framebuffer(16)?;

        // Get pitch
        let pitch = self.get_pitch()?;

        Ok(FramebufferInfo {
            width: phys_w,
            height: phys_h,
            virtual_width: virt_w,
            virtual_height: virt_h,
            depth: actual_depth,
            pitch,
            address: fb_addr,
            size: fb_size,
        })
    }
}

// ============================================================================
// Types
// ============================================================================

/// Mailbox errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxError {
    /// Buffer not 16-byte aligned
    NotAligned,
    /// Response indicated error
    ResponseError,
    /// Invalid response code
    InvalidResponse(u32),
    /// Timeout waiting for response
    Timeout,
}

/// Framebuffer information
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    /// Physical width
    pub width: u32,
    /// Physical height
    pub height: u32,
    /// Virtual width
    pub virtual_width: u32,
    /// Virtual height
    pub virtual_height: u32,
    /// Bits per pixel
    pub depth: u32,
    /// Bytes per row
    pub pitch: u32,
    /// Framebuffer physical address
    pub address: u32,
    /// Framebuffer size in bytes
    pub size: u32,
}

impl FramebufferInfo {
    /// Get framebuffer as mutable slice
    ///
    /// # Safety
    /// Caller must ensure exclusive access to framebuffer memory
    pub unsafe fn as_slice(&self) -> &'static mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.address as *mut u8,
                self.size as usize,
            )
        }
    }

    /// Get pointer to pixel at (x, y)
    pub fn pixel_ptr(&self, x: u32, y: u32) -> *mut u8 {
        let bytes_per_pixel = self.depth / 8;
        let offset = (y * self.pitch) + (x * bytes_per_pixel);
        (self.address + offset) as *mut u8
    }
}

// ============================================================================
// Global Instance
// ============================================================================

/// Global mailbox instance
static mut MAILBOX: Mailbox = Mailbox::new();

/// Get mailbox instance
///
/// # Safety
/// Not thread-safe. Only call from single core during init.
pub fn get_mailbox() -> &'static mut Mailbox {
    // SAFETY: We only access this from core 0 during single-threaded init
    unsafe { &mut *core::ptr::addr_of_mut!(MAILBOX) }
}
