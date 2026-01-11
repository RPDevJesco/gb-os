//! DWC2 USB Host Controller Driver
//!
//! This module provides USB host functionality using the DesignWare Core 2
//! (DWC2) USB controller found in BCM283x SoCs.
//!
//! Features:
//! - USB host mode initialization
//! - Device enumeration
//! - Control transfers
//! - Interrupt transfers (for HID devices)
//! - Xbox 360 controller support (GPi Case 2W gamepad)
//!
//! Note: This is a minimal implementation focused on HID input devices.

use crate::platform_core::mmio::{mmio_read, mmio_write, dmb, delay_us, delay_ms, micros, PERIPHERAL_BASE};
use core::ptr::addr_of_mut;
// ============================================================================
// DWC2 Register Addresses
// ============================================================================

const USB_BASE: usize = PERIPHERAL_BASE + 0x0098_0000;

// Core Global Registers
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

// Host Mode Registers
const USB_HCFG: usize = USB_BASE + 0x400;
const USB_HFIR: usize = USB_BASE + 0x404;
const USB_HFNUM: usize = USB_BASE + 0x408;
const USB_HAINT: usize = USB_BASE + 0x414;
const USB_HAINTMSK: usize = USB_BASE + 0x418;
const USB_HPRT: usize = USB_BASE + 0x440;

// Host Channel Registers (channel 0)
const USB_HCCHAR0: usize = USB_BASE + 0x500;
const USB_HCSPLT0: usize = USB_BASE + 0x504;
const USB_HCINT0: usize = USB_BASE + 0x508;
const USB_HCINTMSK0: usize = USB_BASE + 0x50C;
const USB_HCTSIZ0: usize = USB_BASE + 0x510;

/// Stride between channel register sets
const USB_HC_STRIDE: usize = 0x20;

// Power and Clock Gating
const USB_PCGCCTL: usize = USB_BASE + 0xE00;

// FIFO base
const USB_FIFO0: usize = USB_BASE + 0x1000;

// ============================================================================
// Register Bit Definitions
// ============================================================================

// GAHBCFG bits
const GAHBCFG_GLBL_INTR_EN: u32 = 1 << 0;

// GUSBCFG bits
const GUSBCFG_PHYSEL: u32 = 1 << 6;
const GUSBCFG_FORCE_HOST: u32 = 1 << 29;
const GUSBCFG_FORCE_DEV: u32 = 1 << 30;

// GRSTCTL bits
const GRSTCTL_CSRST: u32 = 1 << 0;
const GRSTCTL_RXFFLSH: u32 = 1 << 4;
const GRSTCTL_TXFFLSH: u32 = 1 << 5;
const GRSTCTL_TXFNUM_ALL: u32 = 0x10 << 6;
const GRSTCTL_AHB_IDLE: u32 = 1 << 31;

// GINTSTS bits
const GINTSTS_CURMOD: u32 = 1 << 0;
const GINTSTS_SOF: u32 = 1 << 3;
const GINTSTS_RXFLVL: u32 = 1 << 4;
const GINTSTS_HPRTINT: u32 = 1 << 24;
const GINTSTS_HCINT: u32 = 1 << 25;

// HPRT bits
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

// HCCHAR bits
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

// HCTSIZ bits
const HCTSIZ_XFERSIZE_SHIFT: u32 = 0;
const HCTSIZ_PKTCNT_SHIFT: u32 = 19;
const HCTSIZ_PID_DATA0: u32 = 0 << 29;
const HCTSIZ_PID_DATA1: u32 = 2 << 29;
const HCTSIZ_PID_SETUP: u32 = 3 << 29;

// HCINT bits
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

// ============================================================================
// USB Protocol Constants
// ============================================================================

const USB_REQ_SET_ADDRESS: u8 = 0x05;
const USB_REQ_GET_DESCRIPTOR: u8 = 0x06;
const USB_REQ_SET_CONFIGURATION: u8 = 0x09;

const USB_DESC_DEVICE: u8 = 0x01;
const USB_DESC_CONFIGURATION: u8 = 0x02;
const USB_DESC_ENDPOINT: u8 = 0x05;

const USB_REQTYPE_DIR_IN: u8 = 0x80;
const USB_REQTYPE_TYPE_STANDARD: u8 = 0x00;
const USB_REQTYPE_RECIP_DEVICE: u8 = 0x00;

// Mailbox constants for power control
const MBOX_BASE: usize = PERIPHERAL_BASE + 0x0000_B880;
const MBOX_STATUS: usize = MBOX_BASE + 0x18;
const MBOX_WRITE: usize = MBOX_BASE + 0x20;
const MBOX_READ: usize = MBOX_BASE + 0x00;
const MBOX_FULL: u32 = 0x8000_0000;
const MBOX_EMPTY: u32 = 0x4000_0000;

// ============================================================================
// USB Setup Packet
// ============================================================================

/// USB Setup Packet (8 bytes)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct UsbSetupPacket {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
}

impl UsbSetupPacket {
    /// Create GET_DESCRIPTOR request
    pub const fn get_descriptor(desc_type: u8, desc_index: u8, length: u16) -> Self {
        Self {
            bm_request_type: USB_REQTYPE_DIR_IN | USB_REQTYPE_TYPE_STANDARD | USB_REQTYPE_RECIP_DEVICE,
            b_request: USB_REQ_GET_DESCRIPTOR,
            w_value: ((desc_type as u16) << 8) | (desc_index as u16),
            w_index: 0,
            w_length: length,
        }
    }

    /// Create SET_ADDRESS request
    pub const fn set_address(addr: u8) -> Self {
        Self {
            bm_request_type: USB_REQTYPE_TYPE_STANDARD | USB_REQTYPE_RECIP_DEVICE,
            b_request: USB_REQ_SET_ADDRESS,
            w_value: addr as u16,
            w_index: 0,
            w_length: 0,
        }
    }

    /// Create SET_CONFIGURATION request
    pub const fn set_configuration(config: u8) -> Self {
        Self {
            bm_request_type: USB_REQTYPE_TYPE_STANDARD | USB_REQTYPE_RECIP_DEVICE,
            b_request: USB_REQ_SET_CONFIGURATION,
            w_value: config as u16,
            w_index: 0,
            w_length: 0,
        }
    }
}

// ============================================================================
// Xbox 360 Controller Input Report
// ============================================================================

/// Xbox 360 Controller Input Report (20 bytes)
///
/// This is the format used by the GPi Case 2W controller
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct Xbox360InputReport {
    pub report_id: u8,
    pub report_length: u8,
    pub buttons_low: u8,
    pub buttons_high: u8,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub left_stick_x: i16,
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub _reserved: [u8; 6],
}

impl Xbox360InputReport {
    // buttons_low bits
    pub const DPAD_UP: u8 = 1 << 0;
    pub const DPAD_DOWN: u8 = 1 << 1;
    pub const DPAD_LEFT: u8 = 1 << 2;
    pub const DPAD_RIGHT: u8 = 1 << 3;
    pub const START: u8 = 1 << 4;
    pub const BACK: u8 = 1 << 5;

    // buttons_high bits
    pub const LB: u8 = 1 << 0;
    pub const RB: u8 = 1 << 1;
    pub const GUIDE: u8 = 1 << 2;
    pub const A: u8 = 1 << 4;
    pub const B: u8 = 1 << 5;
    pub const X: u8 = 1 << 6;
    pub const Y: u8 = 1 << 7;
}

// ============================================================================
// Transfer Result
// ============================================================================

/// Result of a USB transfer
#[derive(Clone, Copy, Debug)]
pub enum TransferResult {
    /// Transfer completed successfully with N bytes
    Success(usize),
    /// Device returned NAK (no data available)
    Nak,
    /// Device returned STALL
    Stall,
    /// Transfer error
    Error,
    /// Transfer timed out
    Timeout,
}

// ============================================================================
// USB Host Controller
// ============================================================================

/// DWC2 USB Host Controller
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
    /// Create a new USB host controller instance
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

    /// Check if a device has been enumerated
    pub fn is_enumerated(&self) -> bool {
        self.enumerated
    }

    /// Power on USB via mailbox
    fn power_on(&self) -> bool {
        #[repr(C, align(16))]
        struct UsbMbox {
            data: [u32; 8],
        }
        static mut USB_MBOX: UsbMbox = UsbMbox { data: [0; 8] };

        let mbox = unsafe { &mut *addr_of_mut!(USB_MBOX.data) };
        mbox[0] = 8 * 4;
        mbox[1] = 0;
        mbox[2] = 0x28001; // SET_POWER_STATE
        mbox[3] = 8;
        mbox[4] = 8;
        mbox[5] = 3; // USB HCD device ID
        mbox[6] = 3; // On + Wait
        mbox[7] = 0;

        dmb();
        let mbox_addr = mbox.as_ptr() as u32;
        let mbox_msg = (mbox_addr & !0xF) | 8;

        // Wait for mailbox not full
        for _ in 0..10000 {
            if mmio_read(MBOX_STATUS) & MBOX_FULL == 0 {
                break;
            }
            delay_us(1);
        }
        mmio_write(MBOX_WRITE, mbox_msg);

        // Wait for response
        for _ in 0..100000 {
            if mmio_read(MBOX_STATUS) & MBOX_EMPTY == 0 {
                let response = mmio_read(MBOX_READ);
                if response == mbox_msg {
                    return mbox[6] & 1 == 1;
                }
            }
            delay_us(10);
        }
        false
    }

    /// Wait for Start of Frame
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

    /// Wait for TX FIFO space
    fn wait_tx_fifo(&self, words: u32) -> bool {
        for _ in 0..10000 {
            let txsts = mmio_read(USB_GNPTXSTS);
            if (txsts & 0xFFFF) >= words {
                return true;
            }
            delay_us(1);
        }
        false
    }

    /// Disable a host channel
    fn disable_channel(&self, ch: usize) {
        let hcchar_addr = USB_HCCHAR0 + ch * USB_HC_STRIDE;
        let hcint_addr = USB_HCINT0 + ch * USB_HC_STRIDE;

        let hcchar = mmio_read(hcchar_addr);
        if hcchar & HCCHAR_CHEN != 0 {
            mmio_write(hcchar_addr, hcchar | HCCHAR_CHDIS);
            for _ in 0..10000 {
                if mmio_read(hcint_addr) & HCINT_CHHLT != 0 {
                    break;
                }
                delay_us(1);
            }
        }
        mmio_write(hcint_addr, 0xFFFF_FFFF);
    }

    /// Initialize the USB host controller
    pub fn init(&mut self) -> Result<(), &'static str> {
        self.power_on();
        delay_ms(50);

        // Verify DWC2 is present
        let snpsid = mmio_read(USB_GSNPSID);
        if (snpsid & 0xFFFF_F000) != 0x4F54_2000 {
            return Err("DWC2 not found");
        }

        // Disable interrupts and DMA
        mmio_write(USB_GINTMSK, 0);
        mmio_write(USB_GAHBCFG, 0);

        // Wait for AHB idle
        for _ in 0..100_000 {
            if mmio_read(USB_GRSTCTL) & GRSTCTL_AHB_IDLE != 0 {
                break;
            }
            delay_us(1);
        }

        // Core soft reset
        mmio_write(USB_GRSTCTL, GRSTCTL_CSRST);
        for _ in 0..100_000 {
            if mmio_read(USB_GRSTCTL) & GRSTCTL_CSRST == 0 {
                break;
            }
            delay_us(1);
        }
        delay_ms(100);

        // Disable power gating
        mmio_write(USB_PCGCCTL, 0);
        delay_ms(10);

        // Force host mode
        let gusbcfg = mmio_read(USB_GUSBCFG);
        mmio_write(
            USB_GUSBCFG,
            (gusbcfg & !GUSBCFG_FORCE_DEV) | GUSBCFG_FORCE_HOST | GUSBCFG_PHYSEL,
        );
        delay_ms(50);

        // Wait for host mode
        for _ in 0..100_000 {
            if mmio_read(USB_GINTSTS) & GINTSTS_CURMOD != 0 {
                break;
            }
            delay_us(1);
        }

        // Configure FIFOs
        mmio_write(USB_GRXFSIZ, 512);
        mmio_write(USB_GNPTXFSIZ, (256 << 16) | 512);
        mmio_write(USB_HPTXFSIZ, (256 << 16) | 768);

        // Flush FIFOs
        mmio_write(USB_GRSTCTL, GRSTCTL_TXFFLSH | GRSTCTL_TXFNUM_ALL);
        for _ in 0..10000 {
            if mmio_read(USB_GRSTCTL) & GRSTCTL_TXFFLSH == 0 {
                break;
            }
            delay_us(1);
        }
        mmio_write(USB_GRSTCTL, GRSTCTL_RXFFLSH);
        for _ in 0..10000 {
            if mmio_read(USB_GRSTCTL) & GRSTCTL_RXFFLSH == 0 {
                break;
            }
            delay_us(1);
        }

        // Configure host
        mmio_write(USB_HCFG, 1); // Full speed
        mmio_write(USB_HFIR, 48000);

        // Initialize all channels
        for ch in 0..8 {
            self.disable_channel(ch);
            mmio_write(
                USB_HCINTMSK0 + ch * USB_HC_STRIDE,
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

        // Enable interrupts
        mmio_write(USB_HAINTMSK, 0xFF);
        mmio_write(USB_GINTSTS, 0xFFFF_FFFF);
        mmio_write(
            USB_GINTMSK,
            GINTSTS_SOF | GINTSTS_RXFLVL | GINTSTS_HPRTINT | GINTSTS_HCINT,
        );
        mmio_write(USB_GAHBCFG, GAHBCFG_GLBL_INTR_EN);

        // Power on port
        let hprt = mmio_read(USB_HPRT);
        mmio_write(USB_HPRT, (hprt & !HPRT_W1C_MASK) | HPRT_PWR);
        delay_ms(100);

        Ok(())
    }

    /// Wait for device connection
    pub fn wait_for_connection(&self, timeout_ms: u32) -> bool {
        let start = micros();
        loop {
            if mmio_read(USB_HPRT) & HPRT_CONN_STS != 0 {
                return true;
            }
            if micros().wrapping_sub(start) > timeout_ms * 1000 {
                return false;
            }
            delay_ms(10);
        }
    }

    /// Reset the USB port
    pub fn reset_port(&mut self) -> Result<(), &'static str> {
        let hprt = mmio_read(USB_HPRT);
        if hprt & HPRT_CONN_STS == 0 {
            return Err("No device connected");
        }

        // Clear status bits
        mmio_write(
            USB_HPRT,
            (hprt & !HPRT_ENA) | HPRT_CONN_DET | HPRT_ENA_CHNG | HPRT_OVRCUR_CHNG,
        );
        delay_ms(10);

        // Start reset
        let hprt = mmio_read(USB_HPRT);
        mmio_write(USB_HPRT, (hprt & !HPRT_W1C_MASK) | HPRT_RST);
        delay_ms(60);

        // End reset
        let hprt = mmio_read(USB_HPRT);
        mmio_write(USB_HPRT, hprt & !HPRT_W1C_MASK & !HPRT_RST);
        delay_ms(20);

        // Wait for port enable
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

    /// Perform a transfer on a channel
    fn do_transfer(
        &mut self,
        ch: usize,
        ep: u8,
        is_in: bool,
        ep_type: u32,
        pid: u32,
        buf: &mut [u8],
        len: usize,
    ) -> TransferResult {
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

        let max_pkt = if ep == 0 {
            self.ep0_max_packet
        } else {
            self.hid_max_packet
        };
        let dir_bit = if is_in { HCCHAR_EPDIR_IN } else { 0 };
        let ls_bit = if self.port_speed == 2 {
            HCCHAR_LSDEV
        } else {
            0
        };
        let frame = mmio_read(USB_HFNUM) & 1;
        let odd_frame = if frame != 0 { HCCHAR_ODDFRM } else { 0 };

        let hcchar = (max_pkt as u32 & HCCHAR_MPS_MASK)
            | ((ep as u32) << HCCHAR_EPNUM_SHIFT)
            | dir_bit
            | ls_bit
            | ep_type
            | (1 << HCCHAR_MC_SHIFT)
            | ((self.device_address as u32) << HCCHAR_DEVADDR_SHIFT)
            | odd_frame;

        let request_len = if is_in {
            max_pkt as usize
        } else {
            len.min(max_pkt as usize)
        };
        let hctsiz =
            ((request_len as u32) << HCTSIZ_XFERSIZE_SHIFT) | (1 << HCTSIZ_PKTCNT_SHIFT) | pid;

        mmio_write(hcint_addr, 0xFFFF_FFFF);

        if !is_in && request_len > 0 {
            if !self.wait_tx_fifo(((request_len + 3) / 4) as u32) {
                return TransferResult::Error;
            }
        }

        mmio_write(hctsiz_addr, hctsiz);
        dmb();
        mmio_write(hcchar_addr, hcchar | HCCHAR_CHEN);
        dmb();

        // Write OUT data
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
                mmio_write(fifo_addr, word);
            }
            dmb();
        }

        let mut received = 0usize;
        let timeout_us = if ep_type == HCCHAR_EPTYPE_CTRL {
            500_000
        } else {
            500
        };
        let start = micros();

        loop {
            // Read IN data from RX FIFO
            if is_in {
                while mmio_read(USB_GINTSTS) & GINTSTS_RXFLVL != 0 {
                    let rxsts = mmio_read(USB_GRXSTSR);
                    let rx_ch = (rxsts & 0xF) as usize;
                    if rx_ch != ch {
                        let _ = mmio_read(USB_GRXSTSP);
                        continue;
                    }

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
                    if pkt_status == 3 || pkt_status == 7 {
                        break;
                    }
                }
            }

            let hcint = mmio_read(hcint_addr);

            if hcint & HCINT_XFERCOMP != 0 {
                mmio_write(hcint_addr, 0xFFFF_FFFF);
                return TransferResult::Success(if is_in { received } else { request_len });
            }

            if hcint & HCINT_CHHLT != 0 {
                mmio_write(hcint_addr, 0xFFFF_FFFF);
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

            if micros().wrapping_sub(start) > timeout_us {
                self.disable_channel(ch);
                if is_in && received > 0 {
                    return TransferResult::Success(received);
                }
                return TransferResult::Timeout;
            }

            delay_us(1);
        }
    }

    /// Perform a control transfer
    fn control_transfer(
        &mut self,
        setup: &UsbSetupPacket,
        data: Option<&mut [u8]>,
    ) -> Result<usize, &'static str> {
        const CH: usize = 0;
        const MAX_RETRIES: u32 = 50;

        let setup_bytes =
            unsafe { core::slice::from_raw_parts(setup as *const _ as *const u8, 8) };
        let mut setup_buf = [0u8; 8];
        setup_buf.copy_from_slice(setup_bytes);

        // SETUP stage
        for _ in 0..MAX_RETRIES {
            match self.do_transfer(
                CH,
                0,
                false,
                HCCHAR_EPTYPE_CTRL,
                HCTSIZ_PID_SETUP,
                &mut setup_buf,
                8,
            ) {
                TransferResult::Success(_) => break,
                TransferResult::Nak => {
                    delay_ms(1);
                    continue;
                }
                _ => return Err("SETUP failed"),
            }
        }

        let mut transferred = 0usize;

        // DATA stage
        if let Some(buf) = data {
            if !buf.is_empty() && setup.w_length > 0 {
                let is_in = (setup.bm_request_type & USB_REQTYPE_DIR_IN) != 0;
                let mut data_toggle = HCTSIZ_PID_DATA1;
                let mut offset = 0usize;
                let total_len = (setup.w_length as usize).min(buf.len());

                while offset < total_len {
                    let chunk_len = (total_len - offset).min(self.ep0_max_packet as usize);

                    for _ in 0..MAX_RETRIES {
                        let result = self.do_transfer(
                            CH,
                            0,
                            is_in,
                            HCCHAR_EPTYPE_CTRL,
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
                                    offset = total_len;
                                }
                                break;
                            }
                            TransferResult::Nak => {
                                delay_ms(1);
                                continue;
                            }
                            _ => return Err("DATA failed"),
                        }
                    }
                }
            }
        }

        // STATUS stage
        let status_in = setup.w_length == 0 || (setup.bm_request_type & USB_REQTYPE_DIR_IN) == 0;
        let mut status_buf = [0u8; 8];

        for _ in 0..MAX_RETRIES {
            match self.do_transfer(
                CH,
                0,
                status_in,
                HCCHAR_EPTYPE_CTRL,
                HCTSIZ_PID_DATA1,
                &mut status_buf,
                0,
            ) {
                TransferResult::Success(_) => return Ok(transferred),
                TransferResult::Nak => {
                    delay_ms(1);
                    continue;
                }
                _ => return Err("STATUS failed"),
            }
        }

        Err("STATUS timeout")
    }

    /// Enumerate the connected device
    pub fn enumerate(&mut self) -> Result<(), &'static str> {
        let mut desc_buf = [0u8; 18];

        // Get first 8 bytes of device descriptor
        let setup = UsbSetupPacket::get_descriptor(USB_DESC_DEVICE, 0, 8);
        self.control_transfer(&setup, Some(&mut desc_buf[..8]))?;

        // Get max packet size
        self.ep0_max_packet = desc_buf[7] as u16;
        if self.ep0_max_packet == 0 || self.ep0_max_packet > 64 {
            self.ep0_max_packet = 8;
        }

        // Reset port again
        self.reset_port()?;
        delay_ms(20);

        // Set address
        let setup = UsbSetupPacket::set_address(1);
        self.control_transfer(&setup, None)?;
        self.device_address = 1;
        delay_ms(10);

        // Get full device descriptor
        let setup = UsbSetupPacket::get_descriptor(USB_DESC_DEVICE, 0, 18);
        self.control_transfer(&setup, Some(&mut desc_buf))?;

        // Get configuration descriptor
        let mut config_buf = [0u8; 64];
        let setup = UsbSetupPacket::get_descriptor(USB_DESC_CONFIGURATION, 0, 64);
        let len = self.control_transfer(&setup, Some(&mut config_buf))?;

        // Parse configuration to find HID endpoint
        self.parse_config_descriptor(&config_buf[..len])?;

        // Set configuration
        let config_val = if len >= 6 { config_buf[5] } else { 1 };
        let setup = UsbSetupPacket::set_configuration(config_val);
        self.control_transfer(&setup, None)?;

        self.enumerated = true;
        Ok(())
    }

    /// Parse configuration descriptor to find HID interrupt IN endpoint
    fn parse_config_descriptor(&mut self, data: &[u8]) -> Result<(), &'static str> {
        let mut pos = 0;
        while pos + 2 <= data.len() {
            let len = data[pos] as usize;
            let desc_type = data[pos + 1];
            if len == 0 || pos + len > data.len() {
                break;
            }

            if desc_type == USB_DESC_ENDPOINT && len >= 7 {
                let ep_addr = data[pos + 2];
                let ep_attr = data[pos + 3];
                let ep_max_pkt = u16::from_le_bytes([data[pos + 4], data[pos + 5]]);
                let is_in = (ep_addr & 0x80) != 0;
                let ep_type = ep_attr & 0x03;

                // Interrupt IN endpoint
                if is_in && ep_type == 3 {
                    self.hid_endpoint = ep_addr & 0x0F;
                    self.hid_max_packet = ep_max_pkt;
                    return Ok(());
                }
            }
            pos += len;
        }
        Err("No HID endpoint found")
    }

    /// Read HID input report
    ///
    /// Returns `Ok(true)` if a report was received, `Ok(false)` if NAK/timeout
    pub fn read_input(&mut self, report: &mut Xbox360InputReport) -> Result<bool, &'static str> {
        if !self.enumerated || self.hid_endpoint == 0 {
            return Err("Not enumerated");
        }

        const CH: usize = 1;
        let pid = if self.hid_data_toggle {
            HCTSIZ_PID_DATA1
        } else {
            HCTSIZ_PID_DATA0
        };
        let len = core::mem::size_of::<Xbox360InputReport>().min(self.hid_max_packet as usize);

        let report_bytes =
            unsafe { core::slice::from_raw_parts_mut(report as *mut _ as *mut u8, len) };

        match self.do_transfer(
            CH,
            self.hid_endpoint,
            true,
            HCCHAR_EPTYPE_INTR,
            pid,
            report_bytes,
            len,
        ) {
            TransferResult::Success(n) => {
                if n == 0 {
                    return Ok(false);
                }
                self.hid_data_toggle = !self.hid_data_toggle;
                Ok(true)
            }
            TransferResult::Nak => Ok(false),
            TransferResult::Timeout => Ok(false),
            _ => Err("Transfer error"),
        }
    }
}
