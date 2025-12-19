//! PCI Bus Enumeration and Configuration
//!
//! Provides access to PCI configuration space and device enumeration.
//! Uses I/O ports 0xCF8 (address) and 0xCFC (data).

use crate::arch::x86::io::{inl, outl};

// =============================================================================
// Constants
// =============================================================================

/// PCI Configuration Address Port
const PCI_CONFIG_ADDR: u16 = 0xCF8;
/// PCI Configuration Data Port
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// Maximum buses to scan
const MAX_BUS: u8 = 8;
/// Maximum devices per bus
const MAX_DEVICE: u8 = 32;
/// Maximum functions per device
const MAX_FUNCTION: u8 = 8;

// =============================================================================
// Known Device IDs
// =============================================================================

/// Intel Vendor ID
pub const INTEL_VENDOR: u16 = 0x8086;
/// ATI Vendor ID
pub const ATI_VENDOR: u16 = 0x1002;

/// Intel PIIX4 IDE Controller
pub const PIIX4_IDE: u16 = 0x7111;
/// Intel 82371AB PIIX4 ISA Bridge
pub const PIIX4_ISA: u16 = 0x7110;
/// Intel 440BX Host Bridge
pub const I440BX_HOST: u16 = 0x7190;
/// Intel 440BX AGP Bridge
pub const I440BX_AGP: u16 = 0x7191;

// =============================================================================
// PCI Class Codes
// =============================================================================

pub mod class {
    pub const MASS_STORAGE: u8 = 0x01;
    pub const NETWORK: u8 = 0x02;
    pub const DISPLAY: u8 = 0x03;
    pub const BRIDGE: u8 = 0x06;
}

pub mod subclass {
    pub const IDE: u8 = 0x01;
    pub const SATA: u8 = 0x06;
    pub const HOST_BRIDGE: u8 = 0x00;
    pub const ISA_BRIDGE: u8 = 0x01;
    pub const PCI_BRIDGE: u8 = 0x04;
}

// =============================================================================
// PCI Device Structure
// =============================================================================

/// Represents a detected PCI device
#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub header_type: u8,
}

impl PciDevice {
    /// Check if this is a multi-function device
    pub fn is_multifunction(&self) -> bool {
        (self.header_type & 0x80) != 0
    }

    /// Get a human-readable name for known devices
    pub fn name(&self) -> &'static str {
        match (self.vendor_id, self.device_id) {
            (INTEL_VENDOR, PIIX4_IDE) => "Intel PIIX4 IDE",
            (INTEL_VENDOR, PIIX4_ISA) => "Intel PIIX4 ISA",
            (INTEL_VENDOR, I440BX_HOST) => "Intel 440BX Host",
            (INTEL_VENDOR, I440BX_AGP) => "Intel 440BX AGP",
            (ATI_VENDOR, 0x4C4D) => "ATI Rage Mobility P",
            _ => match self.class {
                class::MASS_STORAGE => "Mass Storage",
                class::NETWORK => "Network",
                class::DISPLAY => "Display",
                class::BRIDGE => "Bridge",
                _ => "Unknown",
            }
        }
    }
}

// =============================================================================
// Configuration Space Access
// =============================================================================

/// Read 32-bit value from PCI configuration space
#[inline]
pub fn config_read32(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let address = 0x80000000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        outl(PCI_CONFIG_ADDR, address);
        inl(PCI_CONFIG_DATA)
    }
}

/// Read 16-bit value from PCI configuration space
#[inline]
pub fn config_read16(bus: u8, device: u8, func: u8, offset: u8) -> u16 {
    let dword = config_read32(bus, device, func, offset & 0xFC);
    let shift = ((offset & 2) * 8) as u32;
    ((dword >> shift) & 0xFFFF) as u16
}

/// Read 8-bit value from PCI configuration space
#[inline]
pub fn config_read8(bus: u8, device: u8, func: u8, offset: u8) -> u8 {
    let dword = config_read32(bus, device, func, offset & 0xFC);
    let shift = ((offset & 3) * 8) as u32;
    ((dword >> shift) & 0xFF) as u8
}

/// Write 32-bit value to PCI configuration space
#[inline]
pub fn config_write32(bus: u8, device: u8, func: u8, offset: u8, value: u32) {
    let address = 0x80000000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        outl(PCI_CONFIG_ADDR, address);
        outl(PCI_CONFIG_DATA, value);
    }
}

/// Write 16-bit value to PCI configuration space
#[inline]
pub fn config_write16(bus: u8, device: u8, func: u8, offset: u8, value: u16) {
    let dword = config_read32(bus, device, func, offset & 0xFC);
    let shift = ((offset & 2) * 8) as u32;
    let mask = !(0xFFFFu32 << shift);
    let new_value = (dword & mask) | ((value as u32) << shift);
    config_write32(bus, device, func, offset & 0xFC, new_value);
}

/// Write 8-bit value to PCI configuration space
#[inline]
pub fn config_write8(bus: u8, device: u8, func: u8, offset: u8, value: u8) {
    let dword = config_read32(bus, device, func, offset & 0xFC);
    let shift = ((offset & 3) * 8) as u32;
    let mask = !(0xFFu32 << shift);
    let new_value = (dword & mask) | ((value as u32) << shift);
    config_write32(bus, device, func, offset & 0xFC, new_value);
}

// =============================================================================
// Device Enumeration
// =============================================================================

/// Storage for discovered PCI devices
pub struct PciDevices {
    devices: [Option<PciDevice>; 32],
    count: usize,
}

impl PciDevices {
    pub const fn new() -> Self {
        Self {
            devices: [None; 32],
            count: 0,
        }
    }

    /// Add a device to the list
    pub fn add(&mut self, device: PciDevice) -> bool {
        if self.count < 32 {
            self.devices[self.count] = Some(device);
            self.count += 1;
            true
        } else {
            false
        }
    }

    /// Get number of devices
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get device by index
    pub fn get(&self, index: usize) -> Option<&PciDevice> {
        if index < self.count {
            self.devices[index].as_ref()
        } else {
            None
        }
    }

    /// Find device by vendor/device ID
    pub fn find(&self, vendor: u16, device_id: u16) -> Option<&PciDevice> {
        for i in 0..self.count {
            if let Some(dev) = &self.devices[i] {
                if dev.vendor_id == vendor && dev.device_id == device_id {
                    return Some(dev);
                }
            }
        }
        None
    }

    /// Find device by class/subclass
    pub fn find_by_class(&self, class: u8, subclass: u8) -> Option<&PciDevice> {
        for i in 0..self.count {
            if let Some(dev) = &self.devices[i] {
                if dev.class == class && dev.subclass == subclass {
                    return Some(dev);
                }
            }
        }
        None
    }

    /// Iterator over devices
    pub fn iter(&self) -> impl Iterator<Item = &PciDevice> {
        self.devices[..self.count].iter().filter_map(|d| d.as_ref())
    }
}

/// Global PCI device list
static mut PCI_DEVICES: PciDevices = PciDevices::new();

/// Check if a device exists at the given location
fn device_exists(bus: u8, device: u8, func: u8) -> bool {
    let vendor = config_read16(bus, device, func, 0x00);
    vendor != 0xFFFF && vendor != 0x0000
}

/// Read device info from PCI config space
fn read_device_info(bus: u8, device: u8, func: u8) -> PciDevice {
    let vendor_device = config_read32(bus, device, func, 0x00);
    let class_rev = config_read32(bus, device, func, 0x08);
    let header = config_read8(bus, device, func, 0x0E);

    PciDevice {
        bus,
        device,
        function: func,
        vendor_id: (vendor_device & 0xFFFF) as u16,
        device_id: ((vendor_device >> 16) & 0xFFFF) as u16,
        class: ((class_rev >> 24) & 0xFF) as u8,
        subclass: ((class_rev >> 16) & 0xFF) as u8,
        prog_if: ((class_rev >> 8) & 0xFF) as u8,
        header_type: header,
    }
}

/// Enumerate all PCI devices
///
/// Returns number of devices found.
/// Use `get_devices()` to access the device list.
pub fn enumerate() -> usize {
    unsafe {
        PCI_DEVICES = PciDevices::new();
    }

    // Draw debug indicator - yellow pixel at (0,0)
    draw_debug_pixel(0, 0x0E);

    // First check if PCI bus exists
    let test = config_read32(0, 0, 0, 0);
    if test == 0xFFFFFFFF {
        // No PCI bus
        draw_debug_pixel(1, 0x04);  // Red = no PCI
        return 0;
    }

    draw_debug_pixel(1, 0x02);  // Green = PCI exists

    let mut found = 0;

    for bus in 0..MAX_BUS {
        for device in 0..MAX_DEVICE {
            // Check function 0 first
            if !device_exists(bus, device, 0) {
                continue;
            }

            let dev_info = read_device_info(bus, device, 0);
            unsafe {
                if PCI_DEVICES.add(dev_info) {
                    found += 1;
                }
            }

            // Check other functions if multi-function device
            if dev_info.is_multifunction() {
                for func in 1..MAX_FUNCTION {
                    if device_exists(bus, device, func) {
                        let func_info = read_device_info(bus, device, func);
                        unsafe {
                            if PCI_DEVICES.add(func_info) {
                                found += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Debug: show device count as pixels
    draw_debug_pixel(2, found as u8);

    found
}

/// Get reference to discovered devices
pub fn get_devices() -> &'static PciDevices {
    unsafe { &PCI_DEVICES }
}

/// Find IDE controller
pub fn find_ide_controller() -> Option<&'static PciDevice> {
    let devices = get_devices();

    // First try to find PIIX4 specifically
    if let Some(dev) = devices.find(INTEL_VENDOR, PIIX4_IDE) {
        return Some(dev);
    }

    // Fall back to any IDE controller
    devices.find_by_class(class::MASS_STORAGE, subclass::IDE)
}

// =============================================================================
// Debug Helpers
// =============================================================================

/// Draw a debug pixel at x position on row 0 of VGA mode 13h
/// Color is a VGA palette index
fn draw_debug_pixel(x: usize, color: u8) {
    unsafe {
        let vga = 0xA0000 as *mut u8;
        core::ptr::write_volatile(vga.add(x), color);
    }
}

/// Draw a debug bar (8 pixels wide) to show progress
pub fn draw_debug_bar(stage: usize, color: u8) {
    unsafe {
        let vga = 0xA0000 as *mut u8;
        let start = stage * 10;
        for i in 0..8 {
            core::ptr::write_volatile(vga.add(start + i), color);
        }
    }
}

// =============================================================================
// PCI Register Offsets (for reference)
// =============================================================================

pub mod reg {
    pub const VENDOR_ID: u8 = 0x00;
    pub const DEVICE_ID: u8 = 0x02;
    pub const COMMAND: u8 = 0x04;
    pub const STATUS: u8 = 0x06;
    pub const REVISION: u8 = 0x08;
    pub const PROG_IF: u8 = 0x09;
    pub const SUBCLASS: u8 = 0x0A;
    pub const CLASS: u8 = 0x0B;
    pub const CACHE_LINE: u8 = 0x0C;
    pub const LATENCY: u8 = 0x0D;
    pub const HEADER_TYPE: u8 = 0x0E;
    pub const BIST: u8 = 0x0F;
    pub const BAR0: u8 = 0x10;
    pub const BAR1: u8 = 0x14;
    pub const BAR2: u8 = 0x18;
    pub const BAR3: u8 = 0x1C;
    pub const BAR4: u8 = 0x20;
    pub const BAR5: u8 = 0x24;
    pub const SUBSYSTEM_VENDOR: u8 = 0x2C;
    pub const SUBSYSTEM_ID: u8 = 0x2E;
    pub const INTERRUPT_LINE: u8 = 0x3C;
    pub const INTERRUPT_PIN: u8 = 0x3D;
}

/// PCI Command register bits
pub mod cmd {
    pub const IO_SPACE: u16 = 0x0001;
    pub const MEMORY_SPACE: u16 = 0x0002;
    pub const BUS_MASTER: u16 = 0x0004;
    pub const INTERRUPT_DISABLE: u16 = 0x0400;
}
