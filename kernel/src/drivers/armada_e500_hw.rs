//! Compaq Armada E500 Hardware Constants
//!
//! Hardware-specific constants derived from the Armada E500 Technical Reference Guide
//! (Part Number 11QY-0200A-WWEN, March 2000).
//!
//! This file provides accurate hardware constants for the Intel 440BX/PIIX4M chipset
//! and SMC FDC37N97X (Tikki) Super I/O controller used in the Armada E500/V300.

// =============================================================================
// Chipset Identification
// =============================================================================

/// Intel 440BX Northbridge PCI identification
pub mod northbridge {
    pub const VENDOR_ID: u16 = 0x8086;      // Intel Corporation
    pub const DEVICE_ID: u16 = 0x7190;      // 82440BX Host Bridge
    pub const AGP_DEVICE_ID: u16 = 0x7191;  // 82440BX AGP Bridge
}

/// Intel PIIX4M/PIIX4E Southbridge PCI identification
pub mod southbridge {
    pub const VENDOR_ID: u16 = 0x8086;      // Intel Corporation
    pub const PIIX4E_DEVICE_ID: u16 = 0x7110; // PIIX4E ISA Bridge (PII)
    pub const PIIX4M_DEVICE_ID: u16 = 0x7198; // PIIX4M ISA Bridge (PIII)
    pub const IDE_DEVICE_ID: u16 = 0x7111;    // PIIX4 IDE Controller
    pub const USB_DEVICE_ID: u16 = 0x7112;    // PIIX4 USB Controller
    pub const PM_DEVICE_ID: u16 = 0x7113;     // PIIX4 Power Management
}

// =============================================================================
// PCI Bus Configuration (from Table 2-4)
// =============================================================================

pub mod pci {
    /// Standard PCI configuration ports
    pub const CONFIG_ADDRESS: u16 = 0x0CF8;
    pub const CONFIG_DATA: u16 = 0x0CFC;
    
    /// PCI slot assignments for Armada E500
    /// - Bus 0: Host bridge, ISA bridge, IDE, USB, PM
    /// - Bus 1: AGP (ATI Rage Mobility-P)
    /// - Bus 2: CardBus (TI PCI1225)
    pub const HOST_BRIDGE_BUS: u8 = 0;
    pub const HOST_BRIDGE_DEV: u8 = 0;
    
    pub const AGP_BRIDGE_BUS: u8 = 0;
    pub const AGP_BRIDGE_DEV: u8 = 1;
    
    pub const ISA_BRIDGE_BUS: u8 = 0;
    pub const ISA_BRIDGE_DEV: u8 = 7;
    pub const ISA_BRIDGE_FUNC_ISA: u8 = 0;
    pub const ISA_BRIDGE_FUNC_IDE: u8 = 1;
    pub const ISA_BRIDGE_FUNC_USB: u8 = 2;
    pub const ISA_BRIDGE_FUNC_PM: u8 = 3;
    
    /// TI PCI1225 CardBus controller
    pub const CARDBUS_BUS: u8 = 0;
    pub const CARDBUS_DEV: u8 = 2;
    pub const TI_PCI1225_VENDOR: u16 = 0x104C;
    pub const TI_PCI1225_DEVICE: u16 = 0xAC1C;
}

// =============================================================================
// IDE Controller Configuration (from Table 2-4)
// =============================================================================

pub mod ide {
    /// Primary IDE Channel (HDD)
    pub const PRIMARY_BASE: u16 = 0x1F0;
    pub const PRIMARY_CTRL: u16 = 0x3F6;
    pub const PRIMARY_IRQ: u8 = 14;
    
    /// Secondary IDE Channel (MultiBay: CD-ROM, DVD-ROM, LS-120)
    pub const SECONDARY_BASE: u16 = 0x170;
    pub const SECONDARY_CTRL: u16 = 0x376;
    pub const SECONDARY_IRQ: u8 = 15;
    
    /// IDE Register Offsets
    pub const REG_DATA: u16 = 0;        // Data (R/W)
    pub const REG_ERROR: u16 = 1;       // Error (R) / Features (W)
    pub const REG_FEATURES: u16 = 1;    // Features (W)
    pub const REG_SECTOR_COUNT: u16 = 2;
    pub const REG_LBA_LOW: u16 = 3;     // Sector Number / LBA 0:7
    pub const REG_LBA_MID: u16 = 4;     // Cylinder Low / LBA 8:15
    pub const REG_LBA_HIGH: u16 = 5;    // Cylinder High / LBA 16:23
    pub const REG_DEVICE: u16 = 6;      // Drive/Head / LBA 24:27
    pub const REG_STATUS: u16 = 7;      // Status (R)
    pub const REG_COMMAND: u16 = 7;     // Command (W)
    
    /// Control Register Offsets (from CTRL base)
    pub const CTRL_ALT_STATUS: u16 = 0; // Alternate Status (R)
    pub const CTRL_DEVICE_CTRL: u16 = 0;// Device Control (W)
    
    /// Status Register Bits
    pub const STATUS_BSY: u8 = 0x80;    // Busy
    pub const STATUS_DRDY: u8 = 0x40;   // Device Ready
    pub const STATUS_DF: u8 = 0x20;     // Device Fault
    pub const STATUS_DSC: u8 = 0x10;    // Seek Complete (deprecated)
    pub const STATUS_DRQ: u8 = 0x08;    // Data Request
    pub const STATUS_CORR: u8 = 0x04;   // Corrected Data (deprecated)
    pub const STATUS_IDX: u8 = 0x02;    // Index (deprecated)
    pub const STATUS_ERR: u8 = 0x01;    // Error
    
    /// Device Control Register Bits
    pub const CTRL_NIEN: u8 = 0x02;     // Disable Interrupts
    pub const CTRL_SRST: u8 = 0x04;     // Software Reset
    pub const CTRL_HOB: u8 = 0x80;      // High Order Byte (LBA48)
    
    /// ATA Commands
    pub const CMD_IDENTIFY: u8 = 0xEC;
    pub const CMD_IDENTIFY_PACKET: u8 = 0xA1;
    pub const CMD_READ_SECTORS: u8 = 0x20;
    pub const CMD_READ_SECTORS_EXT: u8 = 0x24;
    pub const CMD_WRITE_SECTORS: u8 = 0x30;
    pub const CMD_WRITE_SECTORS_EXT: u8 = 0x34;
    pub const CMD_READ_DMA: u8 = 0xC8;
    pub const CMD_WRITE_DMA: u8 = 0xCA;
    pub const CMD_PACKET: u8 = 0xA0;
    pub const CMD_SET_FEATURES: u8 = 0xEF;
    
    /// PIIX4 IDE Transfer Modes (from Tech Ref Guide)
    /// - PIO transfers up to 14 MB/s
    /// - Bus Master DMA up to 33 MB/s (Ultra-DMA-33)
    pub const MAX_PIO_SPEED: u32 = 14_000_000;
    pub const MAX_UDMA_SPEED: u32 = 33_000_000;
    
    /// PIIX4 IDE Buffer Configuration
    /// 16 x 32-bit buffers per channel
    pub const BUFFER_SIZE: usize = 64;  // 16 * 4 bytes
}

// =============================================================================
// System Interrupt Map (from Table 2-7)
// =============================================================================

pub mod irq {
    pub const TIMER: u8 = 0;            // System timer (PIT)
    pub const KEYBOARD: u8 = 1;         // Keyboard controller
    pub const CASCADE: u8 = 2;          // Cascade for IRQ 8-15
    pub const COM2_COM4: u8 = 3;        // Serial COM2/COM4
    pub const COM1_COM3: u8 = 4;        // Serial COM1/COM3
    pub const AUDIO: u8 = 5;            // ESS Maestro 2E Audio
    pub const DISKETTE: u8 = 6;         // Floppy disk controller
    pub const PARALLEL: u8 = 7;         // Parallel port (LPT1)
    pub const RTC: u8 = 8;              // Real-Time Clock
    pub const ACPI_SCI: u8 = 9;         // ACPI System Control Interrupt
    pub const CARDBUS: u8 = 10;         // TI PCI1225 CardBus
    pub const PCI: u8 = 11;             // PCI IRQ (shared)
    pub const PS2_MOUSE: u8 = 12;       // PS/2 pointing device
    pub const FPU: u8 = 13;             // Numeric coprocessor
    pub const PRIMARY_IDE: u8 = 14;     // Primary IDE (HDD)
    pub const FAST_IR: u8 = 15;         // Fast Infrared / Secondary IDE
    
    /// Remapped IRQ numbers for PIC (IRQ + 32)
    pub const INT_TIMER: u8 = 32;
    pub const INT_KEYBOARD: u8 = 33;
    pub const INT_PS2_MOUSE: u8 = 44;
    pub const INT_PRIMARY_IDE: u8 = 46;
    pub const INT_SECONDARY_IDE: u8 = 47;
}

// =============================================================================
// Super I/O Controller - SMC FDC37N97X "Tikki" (from Chapter 5)
// =============================================================================

pub mod super_io {
    /// Configuration port access
    pub const CONFIG_PORT: u16 = 0x3F0;
    pub const CONFIG_INDEX: u16 = 0x3F0;
    pub const CONFIG_DATA: u16 = 0x3F1;
    
    /// Logical Device Numbers (LDN)
    pub const LDN_FDC: u8 = 0;          // Floppy Disk Controller
    pub const LDN_PARALLEL: u8 = 3;     // Parallel Port
    pub const LDN_UART_A: u8 = 4;       // Serial Port A (COM1)
    pub const LDN_UART_B: u8 = 5;       // Serial Port B / IrCC
    pub const LDN_RTC: u8 = 6;          // Real-Time Clock
    pub const LDN_KEYBOARD: u8 = 7;     // Keyboard Controller
    pub const LDN_AUXIO: u8 = 8;        // Auxiliary I/O
    pub const LDN_ACPI: u8 = 10;        // ACPI Embedded Controller
    
    /// Configuration Registers
    pub const REG_DEVICE_ID: u8 = 0x20;
    pub const REG_DEVICE_REV: u8 = 0x21;
    pub const REG_POWER_CTRL: u8 = 0x22;
    pub const REG_LOGICAL_DEV: u8 = 0x07;
    pub const REG_ACTIVATE: u8 = 0x30;
    pub const REG_BASE_HIGH: u8 = 0x60;
    pub const REG_BASE_LOW: u8 = 0x61;
    pub const REG_IRQ: u8 = 0x70;
    pub const REG_DMA: u8 = 0x74;
    
    /// FDC37N97X Device ID
    pub const DEVICE_ID: u8 = 0x0F;     // Expected device ID
}

// =============================================================================
// Keyboard/PS2 Controller (8051 via MSIO)
// =============================================================================

pub mod ps2 {
    /// Standard PS/2 I/O ports (via 8051 mailbox)
    pub const DATA_PORT: u16 = 0x60;
    pub const STATUS_PORT: u16 = 0x64;
    pub const COMMAND_PORT: u16 = 0x64;
    
    /// Status Register Bits
    pub const STATUS_OUTPUT_FULL: u8 = 0x01;  // Output buffer full (data ready)
    pub const STATUS_INPUT_FULL: u8 = 0x02;   // Input buffer full (controller busy)
    pub const STATUS_SYSTEM: u8 = 0x04;       // System flag
    pub const STATUS_CMD_DATA: u8 = 0x08;     // 0=data, 1=command
    pub const STATUS_INHIBIT: u8 = 0x10;      // Keyboard inhibited
    pub const STATUS_AUX_FULL: u8 = 0x20;     // Auxiliary output buffer full (mouse)
    pub const STATUS_TIMEOUT: u8 = 0x40;      // Timeout error
    pub const STATUS_PARITY: u8 = 0x80;       // Parity error
    
    /// Controller Commands
    pub const CMD_READ_CONFIG: u8 = 0x20;     // Read controller configuration
    pub const CMD_WRITE_CONFIG: u8 = 0x60;    // Write controller configuration
    pub const CMD_DISABLE_AUX: u8 = 0xA7;     // Disable auxiliary (mouse)
    pub const CMD_ENABLE_AUX: u8 = 0xA8;      // Enable auxiliary (mouse)
    pub const CMD_TEST_AUX: u8 = 0xA9;        // Test auxiliary port
    pub const CMD_SELF_TEST: u8 = 0xAA;       // Self test (returns 0x55 on success)
    pub const CMD_TEST_PORT1: u8 = 0xAB;      // Test keyboard port
    pub const CMD_DISABLE_KB: u8 = 0xAD;      // Disable keyboard
    pub const CMD_ENABLE_KB: u8 = 0xAE;       // Enable keyboard
    pub const CMD_READ_INPUT: u8 = 0xC0;      // Read input port
    pub const CMD_WRITE_AUX: u8 = 0xD4;       // Write to auxiliary device
    
    /// Configuration Byte Bits
    pub const CFG_INT_KB: u8 = 0x01;          // Enable keyboard interrupt (IRQ1)
    pub const CFG_INT_AUX: u8 = 0x02;         // Enable auxiliary interrupt (IRQ12)
    pub const CFG_SYSTEM: u8 = 0x04;          // System flag
    pub const CFG_DISABLE_KB: u8 = 0x10;      // Disable keyboard clock
    pub const CFG_DISABLE_AUX: u8 = 0x20;     // Disable auxiliary clock
    pub const CFG_TRANSLATE: u8 = 0x40;       // Enable scancode translation
    
    /// Device Commands (keyboard/mouse)
    pub const DEV_RESET: u8 = 0xFF;           // Reset device
    pub const DEV_RESEND: u8 = 0xFE;          // Resend last byte
    pub const DEV_ACK: u8 = 0xFA;             // Acknowledge
    pub const DEV_SET_DEFAULTS: u8 = 0xF6;    // Set default parameters
    pub const DEV_DISABLE: u8 = 0xF5;         // Disable device
    pub const DEV_ENABLE: u8 = 0xF4;          // Enable device
    pub const DEV_SET_RATE: u8 = 0xF3;        // Set sample/typematic rate
    pub const DEV_GET_ID: u8 = 0xF2;          // Get device ID
    pub const DEV_SET_LEDS: u8 = 0xED;        // Set keyboard LEDs
    
    /// Response Codes
    pub const RESP_ACK: u8 = 0xFA;
    pub const RESP_RESEND: u8 = 0xFE;
    pub const RESP_SELF_TEST_OK: u8 = 0xAA;
    pub const RESP_SELF_TEST_FAIL: u8 = 0xFC;
}

// =============================================================================
// Synaptics TouchPad (from Chapter 8)
// =============================================================================

pub mod synaptics {
    /// Synaptics identification
    pub const MAGIC_ID: u8 = 0x47;            // Synaptics device ID
    
    /// Model capabilities
    pub const CAP_EXTENDED: u8 = 0x80;        // Extended capabilities
    pub const CAP_MIDDLE_BTN: u8 = 0x40;      // Middle button
    pub const CAP_FOUR_BTN: u8 = 0x20;        // Four buttons
    pub const CAP_MULTI_FINGER: u8 = 0x10;    // Multi-finger detection
    pub const CAP_PALM_DETECT: u8 = 0x08;     // Palm detection
    pub const CAP_BALLISTICS: u8 = 0x04;      // Ballistics
    
    /// Identification sequence (knock pattern)
    /// Send: E8 00, E8 00, E8 00, E8 00, E9 (status request)
    pub const KNOCK_SET_RES: u8 = 0xE8;
    pub const KNOCK_STATUS: u8 = 0xE9;
    
    /// Mode byte bits
    pub const MODE_ABSOLUTE: u8 = 0x80;       // Absolute mode
    pub const MODE_HIGH_RATE: u8 = 0x40;      // 80 packets/sec (vs 40)
    pub const MODE_SLEEP: u8 = 0x08;          // Sleep mode
    pub const MODE_DIS_GEST: u8 = 0x04;       // Disable gesture processing
    pub const MODE_W_MODE: u8 = 0x01;         // Enable W (width) in packets
    
    /// Sample rates
    pub const RATE_40: u8 = 40;               // 40 packets per second
    pub const RATE_80: u8 = 80;               // 80 packets per second
    pub const RATE_100: u8 = 100;             // Standard PS/2 rate
    
    /// Resolution settings (counts per mm)
    pub const RES_1: u8 = 0;                  // 1 count/mm
    pub const RES_2: u8 = 1;                  // 2 counts/mm
    pub const RES_4: u8 = 2;                  // 4 counts/mm
    pub const RES_8: u8 = 3;                  // 8 counts/mm
}

// =============================================================================
// Floppy Disk Controller (82077AA compatible)
// =============================================================================

pub mod fdc {
    /// I/O Port addresses
    pub const STATUS_A: u16 = 0x3F0;          // Status Register A (PS/2)
    pub const STATUS_B: u16 = 0x3F1;          // Status Register B (PS/2)
    pub const DOR: u16 = 0x3F2;               // Digital Output Register
    pub const TDR: u16 = 0x3F3;               // Tape Drive Register
    pub const MSR: u16 = 0x3F4;               // Main Status Register
    pub const DSR: u16 = 0x3F4;               // Data Rate Select Register
    pub const FIFO: u16 = 0x3F5;              // Data FIFO
    pub const DIR: u16 = 0x3F7;               // Digital Input Register
    pub const CCR: u16 = 0x3F7;               // Configuration Control Register
    
    /// DOR bits
    pub const DOR_DRIVE_SEL: u8 = 0x03;       // Drive select mask (0-3)
    pub const DOR_NOT_RESET: u8 = 0x04;       // 0 = reset, 1 = normal
    pub const DOR_DMA_EN: u8 = 0x08;          // DMA enable
    pub const DOR_MOTOR_A: u8 = 0x10;         // Motor A on
    pub const DOR_MOTOR_B: u8 = 0x20;         // Motor B on
    pub const DOR_MOTOR_C: u8 = 0x40;         // Motor C on
    pub const DOR_MOTOR_D: u8 = 0x80;         // Motor D on
    
    /// MSR bits
    pub const MSR_DRV0_BUSY: u8 = 0x01;
    pub const MSR_DRV1_BUSY: u8 = 0x02;
    pub const MSR_DRV2_BUSY: u8 = 0x04;
    pub const MSR_DRV3_BUSY: u8 = 0x08;
    pub const MSR_CMD_BSY: u8 = 0x10;         // Command busy
    pub const MSR_NON_DMA: u8 = 0x20;         // Non-DMA mode
    pub const MSR_DIO: u8 = 0x40;             // Data direction (1=read)
    pub const MSR_RQM: u8 = 0x80;             // Data register ready
    
    /// Data rates (from Tech Ref: 125, 250, 300, 500, 1000 Kbps)
    pub const RATE_500K: u8 = 0x00;
    pub const RATE_300K: u8 = 0x01;
    pub const RATE_250K: u8 = 0x02;
    pub const RATE_1M: u8 = 0x03;
    
    /// Commands
    pub const CMD_READ_TRACK: u8 = 0x02;
    pub const CMD_SPECIFY: u8 = 0x03;
    pub const CMD_SENSE_STATUS: u8 = 0x04;
    pub const CMD_WRITE_DATA: u8 = 0x05;
    pub const CMD_READ_DATA: u8 = 0x06;
    pub const CMD_RECALIBRATE: u8 = 0x07;
    pub const CMD_SENSE_INT: u8 = 0x08;
    pub const CMD_WRITE_DEL_DATA: u8 = 0x09;
    pub const CMD_READ_ID: u8 = 0x0A;
    pub const CMD_READ_DEL_DATA: u8 = 0x0C;
    pub const CMD_FORMAT_TRACK: u8 = 0x0D;
    pub const CMD_SEEK: u8 = 0x0F;
    pub const CMD_VERSION: u8 = 0x10;
    pub const CMD_CONFIGURE: u8 = 0x13;
    pub const CMD_PERPENDICULAR: u8 = 0x12;
    pub const CMD_LOCK: u8 = 0x14;
}

// =============================================================================
// ATI Rage Mobility-P Graphics (from Chapter 7)
// =============================================================================

pub mod ati_rage {
    /// PCI identification
    pub const VENDOR_ID: u16 = 0x1002;        // ATI Technologies
    pub const DEVICE_ID: u16 = 0x4C4D;        // Rage Mobility-P (LM)
    
    /// AGP configuration
    /// - AGP 2X support (133 MHz throughput)
    /// - 4 or 8 MB SGRAM video memory
    /// - Supports sideband protocol
    pub const AGP_1X_SPEED: u32 = 66_000_000;
    pub const AGP_2X_SPEED: u32 = 133_000_000;
    
    /// Video memory sizes
    pub const VRAM_4MB: u32 = 4 * 1024 * 1024;
    pub const VRAM_8MB: u32 = 8 * 1024 * 1024;
    
    /// Supported resolutions (from Table 7-4)
    pub const RES_640X480: (u32, u32) = (640, 480);
    pub const RES_800X600: (u32, u32) = (800, 600);
    pub const RES_1024X768: (u32, u32) = (1024, 768);
    pub const RES_1280X1024: (u32, u32) = (1280, 1024);  // CRT only
    pub const RES_1600X1200: (u32, u32) = (1600, 1200);  // CRT only
    
    /// MMIO register base offset (BAR2)
    pub const MMIO_SIZE: u32 = 16 * 1024;     // 16 KB MMIO region
}

// =============================================================================
// USB Controller (PIIX4)
// =============================================================================

pub mod usb {
    /// PIIX4 USB base addresses (legacy mode)
    pub const IO_BASE: u16 = 0xE000;          // Typical I/O base
    pub const IO_SIZE: u16 = 32;              // 32 bytes I/O space
    
    /// USB Host Controller Register Offsets
    pub const USBCMD: u16 = 0x00;             // USB Command
    pub const USBSTS: u16 = 0x02;             // USB Status
    pub const USBINTR: u16 = 0x04;            // USB Interrupt Enable
    pub const FRNUM: u16 = 0x06;              // Frame Number
    pub const FLBASEADD: u16 = 0x08;          // Frame List Base Address
    pub const SOFMOD: u16 = 0x0C;             // Start of Frame Modify
    pub const PORTSC1: u16 = 0x10;            // Port 1 Status/Control
    pub const PORTSC2: u16 = 0x12;            // Port 2 Status/Control
}

// =============================================================================
// CardBus Controller - TI PCI1225 (from Chapter 4)
// =============================================================================

pub mod cardbus {
    /// PCI identification
    pub const VENDOR_ID: u16 = 0x104C;        // Texas Instruments
    pub const DEVICE_ID: u16 = 0xAC1C;        // PCI1225
    
    /// Socket capabilities
    pub const SOCKET_A: u8 = 0;
    pub const SOCKET_B: u8 = 1;
    
    /// CardBus supports
    /// - Two Type II PC Card slots (E500) / One slot (V300)
    /// - 32-bit CardBus and 16-bit PC Cards
    /// - Zoomed Video in bottom slot
    /// - Hot removal/insertion
    pub const MAX_SOCKETS_E500: u8 = 2;
    pub const MAX_SOCKETS_V300: u8 = 1;
    
    /// ExCA register base offset
    pub const EXCA_OFFSET: u16 = 0x800;
    
    /// Power management
    pub const D0_STATE: u8 = 0;               // Full power
    pub const D3_HOT: u8 = 3;                 // Low power, context preserved
    pub const D3_COLD: u8 = 4;                // No power
}

// =============================================================================
// Power Management (ACPI/APM from Appendix B)
// =============================================================================

pub mod power {
    /// System power states supported
    pub const S0_WORKING: u8 = 0;             // Full on
    pub const S1_POS: u8 = 1;                 // Power-On Suspend
    pub const S3_STR: u8 = 3;                 // Suspend-to-RAM
    pub const S4_STD: u8 = 4;                 // Suspend-to-Disk
    pub const S5_SOFT_OFF: u8 = 5;            // Soft Off
    
    /// ACPI SCI (System Control Interrupt)
    pub const SCI_IRQ: u8 = 9;
    
    /// PIIX4 Power Management I/O base
    pub const PM_IO_BASE: u16 = 0xE400;       // Typical PM I/O base
    
    /// PM Register Offsets
    pub const PM1_STS: u16 = 0x00;            // PM1 Status
    pub const PM1_EN: u16 = 0x02;             // PM1 Enable
    pub const PM1_CNT: u16 = 0x04;            // PM1 Control
    pub const PM_TMR: u16 = 0x08;             // PM Timer
    pub const GPE0_STS: u16 = 0x0C;           // GPE0 Status
    pub const GPE0_EN: u16 = 0x0E;            // GPE0 Enable
}

// =============================================================================
// DMA Controller (8237 compatible)
// =============================================================================

pub mod dma {
    /// DMA channel assignments
    pub const CH2_FDC: u8 = 2;                // Floppy disk controller
    
    /// DMA controller ports
    pub const MASTER_CMD: u16 = 0xD0;
    pub const MASTER_REQ: u16 = 0xD2;
    pub const MASTER_MASK_SINGLE: u16 = 0xD4;
    pub const MASTER_MODE: u16 = 0xD6;
    pub const MASTER_CLEAR_FF: u16 = 0xD8;
    pub const MASTER_MASTER_CLR: u16 = 0xDA;
    pub const MASTER_MASK_ALL: u16 = 0xDE;
    
    pub const SLAVE_CMD: u16 = 0x08;
    pub const SLAVE_REQ: u16 = 0x09;
    pub const SLAVE_MASK_SINGLE: u16 = 0x0A;
    pub const SLAVE_MODE: u16 = 0x0B;
    pub const SLAVE_CLEAR_FF: u16 = 0x0C;
    pub const SLAVE_MASTER_CLR: u16 = 0x0D;
    pub const SLAVE_MASK_ALL: u16 = 0x0F;
}

// =============================================================================
// MultiBay / DualBay Device Detection
// =============================================================================

pub mod multibay {
    /// MultiBay device types (68-pin connector)
    pub const DEVICE_NONE: u8 = 0;
    pub const DEVICE_CDROM: u8 = 1;           // 24X CD-ROM
    pub const DEVICE_DVDROM: u8 = 2;          // 4X DVD-ROM
    pub const DEVICE_LS120: u8 = 3;           // SuperDisk LS-120
    pub const DEVICE_HDD: u8 = 4;             // Secondary HDD (with adapter)
    pub const DEVICE_BATTERY: u8 = 5;         // 6-cell Li-ion battery
    
    /// DualBay device types (E500 only)
    pub const DUALBAY_NONE: u8 = 0;
    pub const DUALBAY_FDD: u8 = 1;            // 1.44MB diskette drive
    pub const DUALBAY_BATTERY: u8 = 2;        // Second battery
}
