//! DWC2 USB Host Controller Driver
//!
//! Driver for the DesignWare Core USB 2.0 controller found in
//! Raspberry Pi Zero 2 W (BCM2710).
//!
//! # Features
//!
//! - USB Host mode operation
//! - Control, Bulk, and Interrupt transfers
//! - Low-speed and Full-speed device support
//! - Basic hub support (single level)
//!
//! # Usage
//!
//! ```rust
//! let mut usb = UsbHost::new();
//! usb.init()?;
//! usb.wait_for_connection(3000);
//! usb.reset_port()?;
//! usb.enumerate()?;
//! ```

use crate::mmio::{self, PERIPHERAL_BASE};
use crate::timer;
use crate::mailbox;

// ============================================================================
// Register Addresses
// ============================================================================

const USB_BASE: usize = PERIPHERAL_BASE + 0x0098_0000;

// Core Global Registers
const USB_GOTGCTL: usize = USB_BASE + 0x000;  // OTG Control
const USB_GOTGINT: usize = USB_BASE + 0x004;  // OTG Interrupt
const USB_GAHBCFG: usize = USB_BASE + 0x008;  // AHB Configuration
const USB_GUSBCFG: usize = USB_BASE + 0x00C;  // USB Configuration
const USB_GRSTCTL: usize = USB_BASE + 0x010;  // Reset Control
const USB_GINTSTS: usize = USB_BASE + 0x014;  // Interrupt Status
const USB_GINTMSK: usize = USB_BASE + 0x018;  // Interrupt Mask
const USB_GRXSTSR: usize = USB_BASE + 0x01C;  // RX Status Read
const USB_GRXSTSP: usize = USB_BASE + 0x020;  // RX Status Pop
const USB_GRXFSIZ: usize = USB_BASE + 0x024;  // RX FIFO Size
const USB_GNPTXFSIZ: usize = USB_BASE + 0x028; // Non-Periodic TX FIFO Size
const USB_GNPTXSTS: usize = USB_BASE + 0x02C;  // Non-Periodic TX FIFO Status
const USB_GSNPSID: usize = USB_BASE + 0x040;   // Synopsys ID
const USB_GHWCFG1: usize = USB_BASE + 0x044;   // Hardware Config 1
const USB_GHWCFG2: usize = USB_BASE + 0x048;   // Hardware Config 2
const USB_GHWCFG3: usize = USB_BASE + 0x04C;   // Hardware Config 3
const USB_GHWCFG4: usize = USB_BASE + 0x050;   // Hardware Config 4
const USB_HPTXFSIZ: usize = USB_BASE + 0x100;  // Periodic TX FIFO Size

// Host Mode Registers
const USB_HCFG: usize = USB_BASE + 0x400;      // Host Configuration
const USB_HFIR: usize = USB_BASE + 0x404;      // Host Frame Interval
const USB_HFNUM: usize = USB_BASE + 0x408;     // Host Frame Number
const USB_HPTXSTS: usize = USB_BASE + 0x410;   // Periodic TX FIFO Status
const USB_HAINT: usize = USB_BASE + 0x414;     // Host All Channels Interrupt
const USB_HAINTMSK: usize = USB_BASE + 0x418;  // Host All Channels Interrupt Mask
const USB_HPRT: usize = USB_BASE + 0x440;      // Host Port Control

// Host Channel Registers (8 channels, 0x20 bytes each)
const USB_HCCHAR0: usize = USB_BASE + 0x500;   // Channel Characteristics
const USB_HCSPLT0: usize = USB_BASE + 0x504;   // Channel Split Control
const USB_HCINT0: usize = USB_BASE + 0x508;    // Channel Interrupt
const USB_HCINTMSK0: usize = USB_BASE + 0x50C; // Channel Interrupt Mask
const USB_HCTSIZ0: usize = USB_BASE + 0x510;   // Channel Transfer Size
const USB_HCDMA0: usize = USB_BASE + 0x514;    // Channel DMA Address
const USB_HC_STRIDE: usize = 0x20;             // Bytes between channel register sets

// Power and Clock Gating
const USB_PCGCCTL: usize = USB_BASE + 0xE00;

// FIFO Access (one per channel)
const USB_FIFO0: usize = USB_BASE + 0x1000;
const USB_FIFO_STRIDE: usize = 0x1000;

// ============================================================================
// Register Bit Definitions
// ============================================================================

mod bits {
    // GAHBCFG bits
    pub const GAHBCFG_GLBL_INTR_EN: u32 = 1 << 0;
    pub const GAHBCFG_DMA_EN: u32 = 1 << 5;

    // GUSBCFG bits
    pub const GUSBCFG_PHYSEL: u32 = 1 << 6;
    pub const GUSBCFG_FORCE_HOST: u32 = 1 << 29;
    pub const GUSBCFG_FORCE_DEV: u32 = 1 << 30;

    // GRSTCTL bits
    pub const GRSTCTL_CSRST: u32 = 1 << 0;
    pub const GRSTCTL_RXFFLSH: u32 = 1 << 4;
    pub const GRSTCTL_TXFFLSH: u32 = 1 << 5;
    pub const GRSTCTL_TXFNUM_ALL: u32 = 0x10 << 6;
    pub const GRSTCTL_AHB_IDLE: u32 = 1 << 31;

    // GINTSTS/GINTMSK bits
    pub const GINTSTS_CURMOD: u32 = 1 << 0;  // 1 = Host mode
    pub const GINTSTS_SOF: u32 = 1 << 3;
    pub const GINTSTS_RXFLVL: u32 = 1 << 4;
    pub const GINTSTS_NPTXFE: u32 = 1 << 5;
    pub const GINTSTS_HPRTINT: u32 = 1 << 24;
    pub const GINTSTS_HCINT: u32 = 1 << 25;
    pub const GINTSTS_PTXFE: u32 = 1 << 26;

    // HPRT bits
    pub const HPRT_CONN_STS: u32 = 1 << 0;
    pub const HPRT_CONN_DET: u32 = 1 << 1;
    pub const HPRT_ENA: u32 = 1 << 2;
    pub const HPRT_ENA_CHNG: u32 = 1 << 3;
    pub const HPRT_OVRCUR_ACT: u32 = 1 << 4;
    pub const HPRT_OVRCUR_CHNG: u32 = 1 << 5;
    pub const HPRT_RES: u32 = 1 << 6;
    pub const HPRT_SUSP: u32 = 1 << 7;
    pub const HPRT_RST: u32 = 1 << 8;
    pub const HPRT_PWR: u32 = 1 << 12;
    pub const HPRT_TST_CTL_MASK: u32 = 0xF << 13;
    pub const HPRT_SPD_MASK: u32 = 0x3 << 17;
    pub const HPRT_SPD_SHIFT: u32 = 17;
    // Write-1-to-clear bits (must be preserved when modifying other bits)
    pub const HPRT_W1C_MASK: u32 =
        HPRT_CONN_DET | HPRT_ENA | HPRT_ENA_CHNG | HPRT_OVRCUR_CHNG;

    // HCCHAR bits
    pub const HCCHAR_MPS_MASK: u32 = 0x7FF;
    pub const HCCHAR_EPNUM_SHIFT: u32 = 11;
    pub const HCCHAR_EPDIR_IN: u32 = 1 << 15;
    pub const HCCHAR_LSDEV: u32 = 1 << 17;
    pub const HCCHAR_EPTYPE_SHIFT: u32 = 18;
    pub const HCCHAR_EPTYPE_CTRL: u32 = 0 << 18;
    pub const HCCHAR_EPTYPE_ISOC: u32 = 1 << 18;
    pub const HCCHAR_EPTYPE_BULK: u32 = 2 << 18;
    pub const HCCHAR_EPTYPE_INTR: u32 = 3 << 18;
    pub const HCCHAR_MC_SHIFT: u32 = 20;
    pub const HCCHAR_DEVADDR_SHIFT: u32 = 22;
    pub const HCCHAR_ODDFRM: u32 = 1 << 29;
    pub const HCCHAR_CHDIS: u32 = 1 << 30;
    pub const HCCHAR_CHEN: u32 = 1 << 31;

    // HCTSIZ bits
    pub const HCTSIZ_XFERSIZE_MASK: u32 = 0x7FFFF;
    pub const HCTSIZ_PKTCNT_SHIFT: u32 = 19;
    pub const HCTSIZ_PID_DATA0: u32 = 0 << 29;
    pub const HCTSIZ_PID_DATA2: u32 = 1 << 29;
    pub const HCTSIZ_PID_DATA1: u32 = 2 << 29;
    pub const HCTSIZ_PID_SETUP: u32 = 3 << 29;

    // HCINT bits
    pub const HCINT_XFERCOMP: u32 = 1 << 0;
    pub const HCINT_CHHLT: u32 = 1 << 1;
    pub const HCINT_AHBERR: u32 = 1 << 2;
    pub const HCINT_STALL: u32 = 1 << 3;
    pub const HCINT_NAK: u32 = 1 << 4;
    pub const HCINT_ACK: u32 = 1 << 5;
    pub const HCINT_NYET: u32 = 1 << 6;
    pub const HCINT_XACTERR: u32 = 1 << 7;
    pub const HCINT_BBLERR: u32 = 1 << 8;
    pub const HCINT_FRMOVRUN: u32 = 1 << 9;
    pub const HCINT_DATATGLERR: u32 = 1 << 10;
    pub const HCINT_ERROR_MASK: u32 =
        HCINT_AHBERR | HCINT_STALL | HCINT_XACTERR | HCINT_BBLERR;
}

use bits::*;

// ============================================================================
// USB Protocol Constants
// ============================================================================

/// USB device speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbSpeed {
    High = 0, // 480 Mbps (not supported on Pi)
    Full = 1, // 12 Mbps
    Low = 2,  // 1.5 Mbps
}

impl UsbSpeed {
    fn from_hprt(hprt: u32) -> Self {
        match (hprt & HPRT_SPD_MASK) >> HPRT_SPD_SHIFT {
            0 => UsbSpeed::High,
            1 => UsbSpeed::Full,
            2 => UsbSpeed::Low,
            _ => UsbSpeed::Full,
        }
    }
}

/// USB endpoint type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum EndpointType {
    Control = 0,
    Isochronous = 1,
    Bulk = 2,
    Interrupt = 3,
}

/// Result of a USB transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferResult {
    /// Transfer completed successfully, contains bytes transferred.
    Success(usize),
    /// Device returned NAK (not ready, try again).
    Nak,
    /// Device returned STALL (error, endpoint halted).
    Stall,
    /// Transfer error (CRC, timeout, etc.).
    Error,
    /// Transfer timed out.
    Timeout,
}

impl TransferResult {
    /// Returns `true` if the transfer succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, TransferResult::Success(_))
    }

    /// Returns the number of bytes transferred, or 0 if failed.
    pub fn bytes(&self) -> usize {
        match self {
            TransferResult::Success(n) => *n,
            _ => 0,
        }
    }
}

// ============================================================================
// USB Setup Packet
// ============================================================================

/// USB SETUP packet for control transfers.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct SetupPacket {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
}

impl SetupPacket {
    /// Create a GET_DESCRIPTOR request.
    pub const fn get_descriptor(desc_type: u8, desc_index: u8, length: u16) -> Self {
        Self {
            bm_request_type: 0x80, // Device-to-host, Standard, Device
            b_request: 0x06,       // GET_DESCRIPTOR
            w_value: ((desc_type as u16) << 8) | (desc_index as u16),
            w_index: 0,
            w_length: length,
        }
    }

    /// Create a SET_ADDRESS request.
    pub const fn set_address(addr: u8) -> Self {
        Self {
            bm_request_type: 0x00, // Host-to-device, Standard, Device
            b_request: 0x05,       // SET_ADDRESS
            w_value: addr as u16,
            w_index: 0,
            w_length: 0,
        }
    }

    /// Create a SET_CONFIGURATION request.
    pub const fn set_configuration(config: u8) -> Self {
        Self {
            bm_request_type: 0x00,
            b_request: 0x09, // SET_CONFIGURATION
            w_value: config as u16,
            w_index: 0,
            w_length: 0,
        }
    }

    /// Create a SET_INTERFACE request.
    pub const fn set_interface(interface: u16, alt_setting: u16) -> Self {
        Self {
            bm_request_type: 0x01, // Host-to-device, Standard, Interface
            b_request: 0x0B,       // SET_INTERFACE
            w_value: alt_setting,
            w_index: interface,
            w_length: 0,
        }
    }

    /// Create a class-specific SET_IDLE request (for HID).
    pub const fn hid_set_idle(interface: u16) -> Self {
        Self {
            bm_request_type: 0x21, // Host-to-device, Class, Interface
            b_request: 0x0A,       // SET_IDLE
            w_value: 0,
            w_index: interface,
            w_length: 0,
        }
    }

    /// Convert to byte array for transmission.
    pub fn as_bytes(&self) -> [u8; 8] {
        let w_value = self.w_value.to_le_bytes();
        let w_index = self.w_index.to_le_bytes();
        let w_length = self.w_length.to_le_bytes();
        [
            self.bm_request_type,
            self.b_request,
            w_value[0],
            w_value[1],
            w_index[0],
            w_index[1],
            w_length[0],
            w_length[1],
        ]
    }
}

// ============================================================================
// USB Host Controller
// ============================================================================

/// Number of hardware channels available.
const NUM_CHANNELS: usize = 8;

/// Maximum retries for NAK responses.
const MAX_NAK_RETRIES: u32 = 50;

/// DWC2 USB Host Controller driver.
pub struct UsbHost {
    /// Current device address (0 = default, 1-127 = assigned)
    device_address: u8,
    /// Maximum packet size for endpoint 0
    ep0_max_packet: u16,
    /// Port speed (after enumeration)
    port_speed: UsbSpeed,
    /// Controller initialized flag
    initialized: bool,
    /// Device enumerated flag
    enumerated: bool,
}

impl UsbHost {
    /// Create a new USB host controller instance.
    pub const fn new() -> Self {
        Self {
            device_address: 0,
            ep0_max_packet: 8,
            port_speed: UsbSpeed::Full,
            initialized: false,
            enumerated: false,
        }
    }

    /// Power on USB via mailbox.
    fn power_on(&self) -> bool {
        mailbox::set_power_state(mailbox::device::USB_HCD, true)
    }

    /// Wait for AHB master idle.
    fn wait_ahb_idle(&self, timeout_us: u32) -> bool {
        let start = timer::micros();
        while timer::elapsed_since(start) < timeout_us {
            if mmio::read(USB_GRSTCTL) & GRSTCTL_AHB_IDLE != 0 {
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }

    /// Perform a core soft reset.
    fn core_reset(&self) -> bool {
        // Wait for AHB idle
        if !self.wait_ahb_idle(100_000) {
            return false;
        }

        // Trigger reset
        mmio::write(USB_GRSTCTL, GRSTCTL_CSRST);

        // Wait for reset to complete
        let start = timer::micros();
        while timer::elapsed_since(start) < 100_000 {
            if mmio::read(USB_GRSTCTL) & GRSTCTL_CSRST == 0 {
                timer::delay_ms(100);
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }

    /// Flush TX FIFO.
    fn flush_tx_fifo(&self) {
        mmio::write(USB_GRSTCTL, GRSTCTL_TXFFLSH | GRSTCTL_TXFNUM_ALL);
        let start = timer::micros();
        while timer::elapsed_since(start) < 10_000 {
            if mmio::read(USB_GRSTCTL) & GRSTCTL_TXFFLSH == 0 {
                return;
            }
            core::hint::spin_loop();
        }
    }

    /// Flush RX FIFO.
    fn flush_rx_fifo(&self) {
        mmio::write(USB_GRSTCTL, GRSTCTL_RXFFLSH);
        let start = timer::micros();
        while timer::elapsed_since(start) < 10_000 {
            if mmio::read(USB_GRSTCTL) & GRSTCTL_RXFFLSH == 0 {
                return;
            }
            core::hint::spin_loop();
        }
    }

    /// Disable a host channel.
    fn disable_channel(&self, ch: usize) {
        let hcchar_addr = USB_HCCHAR0 + ch * USB_HC_STRIDE;
        let hcint_addr = USB_HCINT0 + ch * USB_HC_STRIDE;

        let hcchar = mmio::read(hcchar_addr);
        if hcchar & HCCHAR_CHEN != 0 {
            mmio::write(hcchar_addr, hcchar | HCCHAR_CHDIS);
            // Wait for channel to halt
            let start = timer::micros();
            while timer::elapsed_since(start) < 10_000 {
                if mmio::read(hcint_addr) & HCINT_CHHLT != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
        }
        mmio::write(hcint_addr, 0xFFFF_FFFF);
    }

    /// Wait for start of frame.
    fn wait_for_sof(&self) {
        mmio::write(USB_GINTSTS, GINTSTS_SOF);
        let start = timer::micros();
        while timer::elapsed_since(start) < 3000 {
            if mmio::read(USB_GINTSTS) & GINTSTS_SOF != 0 {
                mmio::write(USB_GINTSTS, GINTSTS_SOF);
                return;
            }
            core::hint::spin_loop();
        }
    }

    /// Wait for TX FIFO space.
    fn wait_tx_fifo(&self, words: u32) -> bool {
        let start = timer::micros();
        while timer::elapsed_since(start) < 10_000 {
            let txsts = mmio::read(USB_GNPTXSTS);
            if (txsts & 0xFFFF) >= words {
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }

    /// Initialize the USB host controller.
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Power on USB
        if !self.power_on() {
            return Err("USB power on failed");
        }
        timer::delay_ms(50);

        // Verify Synopsys ID
        let snpsid = mmio::read(USB_GSNPSID);
        if (snpsid & 0xFFFF_F000) != 0x4F54_2000 {
            return Err("Invalid DWC2 ID");
        }

        // Disable interrupts during init
        mmio::write(USB_GINTMSK, 0);
        mmio::write(USB_GAHBCFG, 0);

        // Core reset
        if !self.core_reset() {
            return Err("Core reset failed");
        }

        // Disable power gating
        mmio::write(USB_PCGCCTL, 0);
        timer::delay_ms(10);

        // Force host mode
        let gusbcfg = mmio::read(USB_GUSBCFG);
        mmio::write(
            USB_GUSBCFG,
            (gusbcfg & !GUSBCFG_FORCE_DEV) | GUSBCFG_FORCE_HOST | GUSBCFG_PHYSEL,
        );
        timer::delay_ms(50);

        // Wait for host mode
        let start = timer::micros();
        while timer::elapsed_since(start) < 100_000 {
            if mmio::read(USB_GINTSTS) & GINTSTS_CURMOD != 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Configure FIFOs
        // RX FIFO: 512 words
        // Non-periodic TX: 256 words starting at 512
        // Periodic TX: 256 words starting at 768
        mmio::write(USB_GRXFSIZ, 512);
        mmio::write(USB_GNPTXFSIZ, (256 << 16) | 512);
        mmio::write(USB_HPTXFSIZ, (256 << 16) | 768);

        // Flush FIFOs
        self.flush_tx_fifo();
        self.flush_rx_fifo();

        // Configure host
        mmio::write(USB_HCFG, 1); // Full-speed PHY clock
        mmio::write(USB_HFIR, 48000); // Frame interval for full-speed

        // Initialize all channels
        for ch in 0..NUM_CHANNELS {
            self.disable_channel(ch);
            let hcintmsk_addr = USB_HCINTMSK0 + ch * USB_HC_STRIDE;
            mmio::write(
                hcintmsk_addr,
                HCINT_XFERCOMP
                    | HCINT_CHHLT
                    | HCINT_STALL
                    | HCINT_NAK
                    | HCINT_ACK
                    | HCINT_XACTERR
                    | HCINT_BBLERR
                    | HCINT_DATATGLERR,
            );
        }

        // Enable channel interrupts
        mmio::write(USB_HAINTMSK, 0xFF);

        // Clear and enable global interrupts
        mmio::write(USB_GINTSTS, 0xFFFF_FFFF);
        mmio::write(
            USB_GINTMSK,
            GINTSTS_SOF | GINTSTS_RXFLVL | GINTSTS_HPRTINT | GINTSTS_HCINT,
        );
        mmio::write(USB_GAHBCFG, GAHBCFG_GLBL_INTR_EN);

        // Power on port
        let hprt = mmio::read(USB_HPRT);
        mmio::write(USB_HPRT, (hprt & !HPRT_W1C_MASK) | HPRT_PWR);
        timer::delay_ms(100);

        self.initialized = true;
        Ok(())
    }

    /// Check if a device is connected.
    #[inline]
    pub fn is_connected(&self) -> bool {
        mmio::read(USB_HPRT) & HPRT_CONN_STS != 0
    }

    /// Wait for device connection.
    ///
    /// # Arguments
    /// * `timeout_ms` - Maximum time to wait in milliseconds
    ///
    /// # Returns
    /// `true` if device connected, `false` on timeout.
    pub fn wait_for_connection(&self, timeout_ms: u32) -> bool {
        let start = timer::micros();
        while timer::elapsed_since(start) < timeout_ms * 1000 {
            if self.is_connected() {
                return true;
            }
            timer::delay_ms(10);
        }
        false
    }

    /// Reset the USB port and enable it.
    pub fn reset_port(&mut self) -> Result<(), &'static str> {
        if !self.is_connected() {
            return Err("No device connected");
        }

        // Clear status bits
        let hprt = mmio::read(USB_HPRT);
        mmio::write(
            USB_HPRT,
            (hprt & !HPRT_ENA) | HPRT_CONN_DET | HPRT_ENA_CHNG | HPRT_OVRCUR_CHNG,
        );
        timer::delay_ms(10);

        // Start reset
        let hprt = mmio::read(USB_HPRT);
        mmio::write(USB_HPRT, (hprt & !HPRT_W1C_MASK) | HPRT_RST);
        timer::delay_ms(60); // USB spec requires 10-20ms, but longer is safer

        // End reset
        let hprt = mmio::read(USB_HPRT);
        mmio::write(USB_HPRT, hprt & !HPRT_W1C_MASK & !HPRT_RST);
        timer::delay_ms(20);

        // Wait for port enable
        let start = timer::micros();
        while timer::elapsed_since(start) < 500_000 {
            let hprt = mmio::read(USB_HPRT);
            if hprt & HPRT_ENA_CHNG != 0 {
                mmio::write(USB_HPRT, (hprt & !HPRT_ENA) | HPRT_ENA_CHNG);
            }
            if hprt & HPRT_ENA != 0 {
                self.port_speed = UsbSpeed::from_hprt(hprt);
                self.device_address = 0;
                self.ep0_max_packet = 8;
                self.enumerated = false;
                return Ok(());
            }
            timer::delay_ms(10);
        }

        Err("Port enable timeout")
    }

    /// Perform a transfer on a host channel.
    ///
    /// This is the low-level transfer function used by control, bulk, and
    /// interrupt transfer methods.
    pub fn do_transfer(
        &self,
        ch: usize,
        ep: u8,
        is_in: bool,
        ep_type: EndpointType,
        pid: u32,
        buf: &mut [u8],
        len: usize,
    ) -> TransferResult {
        self.disable_channel(ch);

        // Sync with SOF for control transfers
        if ep_type == EndpointType::Control {
            self.wait_for_sof();
        }

        let hcchar_addr = USB_HCCHAR0 + ch * USB_HC_STRIDE;
        let hctsiz_addr = USB_HCTSIZ0 + ch * USB_HC_STRIDE;
        let hcint_addr = USB_HCINT0 + ch * USB_HC_STRIDE;
        let hcsplt_addr = USB_HCSPLT0 + ch * USB_HC_STRIDE;
        let fifo_addr = USB_FIFO0 + ch * USB_FIFO_STRIDE;

        // No split transactions
        mmio::write(hcsplt_addr, 0);

        // Build channel characteristics
        let max_pkt = if ep == 0 {
            self.ep0_max_packet
        } else {
            len.min(64) as u16
        };
        let dir_bit = if is_in { HCCHAR_EPDIR_IN } else { 0 };
        let ls_bit = if self.port_speed == UsbSpeed::Low {
            HCCHAR_LSDEV
        } else {
            0
        };
        let frame = mmio::read(USB_HFNUM) & 1;
        let odd_frame = if frame != 0 { HCCHAR_ODDFRM } else { 0 };
        let ep_type_bits = (ep_type as u32) << HCCHAR_EPTYPE_SHIFT;

        let hcchar = (max_pkt as u32 & HCCHAR_MPS_MASK)
            | ((ep as u32) << HCCHAR_EPNUM_SHIFT)
            | dir_bit
            | ls_bit
            | ep_type_bits
            | (1 << HCCHAR_MC_SHIFT)
            | ((self.device_address as u32) << HCCHAR_DEVADDR_SHIFT)
            | odd_frame;

        // Transfer size
        let request_len = if is_in {
            max_pkt as usize
        } else {
            len.min(max_pkt as usize)
        };
        let hctsiz = (request_len as u32) | (1 << HCTSIZ_PKTCNT_SHIFT) | pid;

        // Clear interrupts
        mmio::write(hcint_addr, 0xFFFF_FFFF);

        // For OUT transfers, prepare TX FIFO
        if !is_in && request_len > 0 {
            if !self.wait_tx_fifo(((request_len + 3) / 4) as u32) {
                return TransferResult::Error;
            }
        }

        // Program transfer size and enable channel
        mmio::write(hctsiz_addr, hctsiz);
        mmio::dmb();
        mmio::write(hcchar_addr, hcchar | HCCHAR_CHEN);
        mmio::dmb();

        // Write data to FIFO for OUT transfers
        if !is_in && request_len > 0 {
            let words = (request_len + 3) / 4;
            for i in 0..words {
                let start = i * 4;
                let mut word = 0u32;
                for j in 0..4 {
                    if start + j < len {
                        word |= (buf[start + j] as u32) << (j * 8);
                    }
                }
                mmio::write(fifo_addr, word);
            }
            mmio::dmb();
        }

        // Wait for completion
        let mut received = 0usize;
        let timeout_us = if ep_type == EndpointType::Control {
            500_000
        } else {
            2_000
        };
        let start = timer::micros();

        loop {
            // Handle RX FIFO for IN transfers
            if is_in {
                while mmio::read(USB_GINTSTS) & GINTSTS_RXFLVL != 0 {
                    let rxsts = mmio::read(USB_GRXSTSR);
                    let rx_ch = (rxsts & 0xF) as usize;
                    if rx_ch != ch {
                        let _ = mmio::read(USB_GRXSTSP);
                        continue;
                    }

                    let rxsts = mmio::read(USB_GRXSTSP);
                    let byte_count = ((rxsts >> 4) & 0x7FF) as usize;
                    let pkt_status = ((rxsts >> 17) & 0xF) as u8;

                    if pkt_status == 2 && byte_count > 0 {
                        // Data received
                        let words = (byte_count + 3) / 4;
                        for i in 0..words {
                            let word = mmio::read(fifo_addr);
                            for j in 0..4 {
                                let idx = received + i * 4 + j;
                                if idx < buf.len() && (i * 4 + j) < byte_count {
                                    buf[idx] = ((word >> (j * 8)) & 0xFF) as u8;
                                }
                            }
                        }
                        received += byte_count;
                    }
                    if pkt_status == 3 || pkt_status == 7 {
                        // Transfer complete or channel halted
                        break;
                    }
                }
            }

            // Check channel interrupt
            let hcint = mmio::read(hcint_addr);

            if hcint & HCINT_XFERCOMP != 0 {
                mmio::write(hcint_addr, 0xFFFF_FFFF);
                return TransferResult::Success(if is_in { received } else { request_len });
            }

            if hcint & HCINT_CHHLT != 0 {
                mmio::write(hcint_addr, 0xFFFF_FFFF);
                if is_in && received > 0 && (hcint & HCINT_ERROR_MASK) == 0 {
                    return TransferResult::Success(received);
                }
                if hcint & HCINT_STALL != 0 {
                    return TransferResult::Stall;
                }
                if hcint & HCINT_NAK != 0 {
                    return TransferResult::Nak;
                }
                if (hcint & HCINT_ACK != 0) && is_in && received > 0 {
                    return TransferResult::Success(received);
                }
                return TransferResult::Error;
            }

            if timer::elapsed_since(start) > timeout_us {
                self.disable_channel(ch);
                if is_in && received > 0 {
                    return TransferResult::Success(received);
                }
                return TransferResult::Timeout;
            }

            core::hint::spin_loop();
        }
    }

    /// Perform a control transfer.
    pub fn control_transfer(
        &mut self,
        setup: &SetupPacket,
        data: Option<&mut [u8]>,
    ) -> Result<usize, &'static str> {
        const CH: usize = 0;

        let setup_bytes = setup.as_bytes();
        let mut setup_buf = setup_bytes;

        // SETUP stage
        for _ in 0..MAX_NAK_RETRIES {
            match self.do_transfer(
                CH,
                0,
                false,
                EndpointType::Control,
                HCTSIZ_PID_SETUP,
                &mut setup_buf,
                8,
            ) {
                TransferResult::Success(_) => break,
                TransferResult::Nak => {
                    timer::delay_ms(1);
                    continue;
                }
                _ => return Err("SETUP stage failed"),
            }
        }

        // DATA stage (if any)
        let mut transferred = 0usize;
        if let Some(buf) = data {
            if !buf.is_empty() && setup.w_length > 0 {
                let is_in = (setup.bm_request_type & 0x80) != 0;
                let mut data_toggle = HCTSIZ_PID_DATA1;
                let mut offset = 0usize;
                let total_len = (setup.w_length as usize).min(buf.len());

                while offset < total_len {
                    let chunk_len = (total_len - offset).min(self.ep0_max_packet as usize);

                    for _ in 0..MAX_NAK_RETRIES {
                        let result = self.do_transfer(
                            CH,
                            0,
                            is_in,
                            EndpointType::Control,
                            data_toggle,
                            &mut buf[offset..offset + chunk_len],
                            chunk_len,
                        );

                        match result {
                            TransferResult::Success(n) => {
                                offset += n;
                                transferred = offset;
                                data_toggle = if data_toggle == HCTSIZ_PID_DATA1 {
                                    HCTSIZ_PID_DATA0
                                } else {
                                    HCTSIZ_PID_DATA1
                                };
                                if n < self.ep0_max_packet as usize {
                                    offset = total_len; // Short packet = done
                                }
                                break;
                            }
                            TransferResult::Nak => {
                                timer::delay_ms(1);
                                continue;
                            }
                            _ => return Err("DATA stage failed"),
                        }
                    }
                }
            }
        }

        // STATUS stage
        let status_in = setup.w_length == 0 || (setup.bm_request_type & 0x80) == 0;
        let mut status_buf = [0u8; 8];

        for _ in 0..MAX_NAK_RETRIES {
            match self.do_transfer(
                CH,
                0,
                status_in,
                EndpointType::Control,
                HCTSIZ_PID_DATA1,
                &mut status_buf,
                0,
            ) {
                TransferResult::Success(_) => return Ok(transferred),
                TransferResult::Nak => {
                    timer::delay_ms(1);
                    continue;
                }
                _ => return Err("STATUS stage failed"),
            }
        }

        Err("STATUS stage timeout")
    }

    /// Enumerate the connected device.
    ///
    /// This reads device descriptors, assigns an address, and configures
    /// the device.
    pub fn enumerate(&mut self) -> Result<(), &'static str> {
        // Get first 8 bytes of device descriptor to learn EP0 max packet size
        let mut desc_buf = [0u8; 18];
        let setup = SetupPacket::get_descriptor(1, 0, 8); // Device descriptor
        self.control_transfer(&setup, Some(&mut desc_buf[..8]))?;

        // Update EP0 max packet size
        self.ep0_max_packet = desc_buf[7] as u16;
        if self.ep0_max_packet == 0 || self.ep0_max_packet > 64 {
            self.ep0_max_packet = 8;
        }

        // Reset port again with correct packet size
        self.reset_port()?;
        timer::delay_ms(20);

        // Set device address
        let setup = SetupPacket::set_address(1);
        self.control_transfer(&setup, None)?;
        self.device_address = 1;
        timer::delay_ms(10);

        // Get full device descriptor
        let setup = SetupPacket::get_descriptor(1, 0, 18);
        self.control_transfer(&setup, Some(&mut desc_buf))?;

        // Get configuration descriptor
        let mut config_buf = [0u8; 64];
        let setup = SetupPacket::get_descriptor(2, 0, 64); // Configuration descriptor
        let config_len = self.control_transfer(&setup, Some(&mut config_buf))?;

        // Set configuration (use first configuration)
        let config_val = if config_len >= 6 { config_buf[5] } else { 1 };
        let setup = SetupPacket::set_configuration(config_val);
        self.control_transfer(&setup, None)?;

        self.enumerated = true;
        Ok(())
    }

    /// Check if the controller is initialized.
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Check if a device is enumerated.
    #[inline]
    pub fn is_enumerated(&self) -> bool {
        self.enumerated
    }

    /// Get the current device address.
    #[inline]
    pub fn device_address(&self) -> u8 {
        self.device_address
    }

    /// Get the port speed.
    #[inline]
    pub fn port_speed(&self) -> UsbSpeed {
        self.port_speed
    }

    /// Get EP0 max packet size.
    #[inline]
    pub fn ep0_max_packet(&self) -> u16 {
        self.ep0_max_packet
    }

    /// Read the hardware version.
    pub fn version(&self) -> u32 {
        mmio::read(USB_GSNPSID)
    }
}

impl Default for UsbHost {
    fn default() -> Self {
        Self::new()
    }
}
