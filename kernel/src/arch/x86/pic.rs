//! 8259 PIC (Programmable Interrupt Controller) Driver
//!
//! Configured for Compaq Armada E500 interrupt mapping per Technical Reference Guide.
//!
//! System Interrupt Map (from Table 2-7):
//! IRQ 0  - System timer (PIT 8254)
//! IRQ 1  - Keyboard (via MSIO 8051)
//! IRQ 2  - Cascade for IRQ 8-15
//! IRQ 3  - Serial COM2/COM4
//! IRQ 4  - Serial COM1/COM3
//! IRQ 5  - ESS Maestro 2E Audio
//! IRQ 6  - Diskette controller (FDC)
//! IRQ 7  - Parallel port (LPT1)
//! IRQ 8  - Real-Time Clock (PIIX4)
//! IRQ 9  - ACPI SCI interrupt
//! IRQ 10 - CardBus (TI PCI1225)
//! IRQ 11 - PCI IRQ (shared)
//! IRQ 12 - PS/2 pointing device (touchpad/mouse)
//! IRQ 13 - FPU/Numeric coprocessor
//! IRQ 14 - Primary IDE controller (HDD)
//! IRQ 15 - Fast Infrared / Secondary IDE (MultiBay)

use crate::arch::x86::io::{outb, inb};

// =============================================================================
// PIC I/O Ports
// =============================================================================

/// Master PIC ports
pub const PIC1_COMMAND: u16 = 0x20;
pub const PIC1_DATA: u16 = 0x21;

/// Slave PIC ports
pub const PIC2_COMMAND: u16 = 0xA0;
pub const PIC2_DATA: u16 = 0xA1;

// =============================================================================
// PIC Commands
// =============================================================================

/// Initialization Command Word 1 (ICW1)
pub const ICW1_ICW4: u8 = 0x01;       // ICW4 needed
pub const ICW1_SINGLE: u8 = 0x02;     // Single mode (not cascaded)
pub const ICW1_INTERVAL4: u8 = 0x04;  // Call address interval 4
pub const ICW1_LEVEL: u8 = 0x08;      // Level triggered mode
pub const ICW1_INIT: u8 = 0x10;       // Initialization

/// ICW4 values
pub const ICW4_8086: u8 = 0x01;       // 8086/88 mode
pub const ICW4_AUTO: u8 = 0x02;       // Auto EOI
pub const ICW4_BUF_SLAVE: u8 = 0x08;  // Buffered mode/slave
pub const ICW4_BUF_MASTER: u8 = 0x0C; // Buffered mode/master
pub const ICW4_SFNM: u8 = 0x10;       // Special fully nested mode

/// OCW2 - End of Interrupt commands
pub const OCW2_EOI: u8 = 0x20;        // Non-specific EOI
pub const OCW2_SPECIFIC_EOI: u8 = 0x60; // Specific EOI (add IRQ number)

/// OCW3 - Read commands
pub const OCW3_READ_IRR: u8 = 0x0A;   // Read IRR (pending interrupts)
pub const OCW3_READ_ISR: u8 = 0x0B;   // Read ISR (in-service interrupts)
pub const IRQ_BASE_MASTER: u8 = 32;
pub const IRQ_BASE_SLAVE: u8 = 40;

// =============================================================================
// IRQ Numbers (Armada E500 specific from Table 2-7)
// =============================================================================

pub mod irq {
    pub const TIMER: u8 = 0;
    pub const KEYBOARD: u8 = 1;
    pub const CASCADE: u8 = 2;
    pub const COM2: u8 = 3;
    pub const COM1: u8 = 4;
    pub const AUDIO: u8 = 5;           // ESS Maestro 2E
    pub const FDC: u8 = 6;             // Floppy Disk Controller
    pub const LPT1: u8 = 7;            // Parallel Port
    pub const RTC: u8 = 8;             // Real-Time Clock
    pub const ACPI_SCI: u8 = 9;        // ACPI System Control Interrupt
    pub const CARDBUS: u8 = 10;        // TI PCI1225 CardBus
    pub const PCI: u8 = 11;            // PCI shared IRQ
    pub const PS2_MOUSE: u8 = 12;      // Touchpad/Mouse
    pub const FPU: u8 = 13;            // Numeric Coprocessor
    pub const PRIMARY_IDE: u8 = 14;    // Primary IDE (HDD)
    pub const SECONDARY_IDE: u8 = 15;  // Secondary IDE / Fast IR
}

/// Interrupt vector numbers (IRQ + offset)
/// Standard mapping: Master PIC at 0x20 (32), Slave PIC at 0x28 (40)
pub mod int {
    pub const TIMER: u8 = 32;
    pub const KEYBOARD: u8 = 33;
    pub const CASCADE: u8 = 34;
    pub const COM2: u8 = 35;
    pub const COM1: u8 = 36;
    pub const AUDIO: u8 = 37;
    pub const FDC: u8 = 38;
    pub const LPT1: u8 = 39;
    pub const RTC: u8 = 40;
    pub const ACPI_SCI: u8 = 41;
    pub const CARDBUS: u8 = 42;
    pub const PCI: u8 = 43;
    pub const PS2_MOUSE: u8 = 44;
    pub const FPU: u8 = 45;
    pub const PRIMARY_IDE: u8 = 46;
    pub const SECONDARY_IDE: u8 = 47;
}

// =============================================================================
// PIC Driver
// =============================================================================

/// Initialize both PICs with standard remapping
/// Remaps IRQ 0-7 to INT 32-39, IRQ 8-15 to INT 40-47
pub fn init() {
    unsafe {
        // Save current masks
        let mask1 = inb(PIC1_DATA);
        let mask2 = inb(PIC2_DATA);

        // ICW1: Start initialization sequence (cascade mode, edge triggered)
        outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();
        outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();

        // ICW2: Vector offsets
        // Master PIC: IRQ 0-7 → INT 32-39 (0x20)
        outb(PIC1_DATA, 0x20);
        io_wait();
        // Slave PIC: IRQ 8-15 → INT 40-47 (0x28)
        outb(PIC2_DATA, 0x28);
        io_wait();

        // ICW3: Cascade configuration
        // Master: Slave on IRQ2 (bit 2 = 0x04)
        outb(PIC1_DATA, 0x04);
        io_wait();
        // Slave: Cascade identity (IRQ2 = 0x02)
        outb(PIC2_DATA, 0x02);
        io_wait();

        // ICW4: 8086 mode, normal EOI
        outb(PIC1_DATA, ICW4_8086);
        io_wait();
        outb(PIC2_DATA, ICW4_8086);
        io_wait();

        // Restore masks (all masked initially, will be unmasked as needed)
        outb(PIC1_DATA, mask1);
        outb(PIC2_DATA, mask2);
    }
}

/// Initialize PICs with RetroFutureGB-optimized configuration
/// Only enables IRQs needed for emulator operation
pub fn init_for_emulator() {
    unsafe {
        // Full initialization
        outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();
        outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();

        // Standard vector offsets
        outb(PIC1_DATA, 0x20);
        io_wait();
        outb(PIC2_DATA, 0x28);
        io_wait();

        // Cascade configuration
        outb(PIC1_DATA, 0x04);
        io_wait();
        outb(PIC2_DATA, 0x02);
        io_wait();

        // 8086 mode
        outb(PIC1_DATA, ICW4_8086);
        io_wait();
        outb(PIC2_DATA, ICW4_8086);
        io_wait();

        // Set masks for emulator operation:
        // Master (IRQ 0-7): Enable timer (0), keyboard (1), cascade (2)
        // Binary: 11111000 = 0xF8 (1 = masked)
        // Actually enable: ~0xF8 = 0x07 means IRQ 0,1,2 unmasked
        outb(PIC1_DATA, 0xF8);  // Mask all except 0,1,2

        // Slave (IRQ 8-15): Enable mouse (12), primary IDE (14)
        // IRQ 12 = bit 4 on slave, IRQ 14 = bit 6 on slave
        // Binary: 10101111 = 0xAF (mask all except 4 and 6)
        // Wait, IRQ 12 is IRQ (12-8) = 4 on slave, IRQ 14 is (14-8) = 6 on slave
        outb(PIC2_DATA, 0xAF);  // Mask all except IRQ 12 and 14
    }
}

/// Small delay for I/O operations
fn io_wait() {
    // Write to unused port 0x80 (POST diagnostic port) for ~1μs delay
    unsafe { outb(0x80, 0); }
}

/// Send End of Interrupt signal
pub fn send_eoi(irq: u8) {
    unsafe {
        if irq >= 8 {
            // IRQ from slave PIC, send EOI to both
            outb(PIC2_COMMAND, OCW2_EOI);
        }
        // Always send EOI to master
        outb(PIC1_COMMAND, OCW2_EOI);
    }
}

/// Send specific EOI for an IRQ
pub fn send_specific_eoi(irq: u8) {
    unsafe {
        if irq >= 8 {
            // Send specific EOI to slave (IRQ - 8)
            outb(PIC2_COMMAND, OCW2_SPECIFIC_EOI | (irq - 8));
            // Then send EOI for cascade (IRQ 2) to master
            outb(PIC1_COMMAND, OCW2_SPECIFIC_EOI | 2);
        } else {
            // Send specific EOI to master
            outb(PIC1_COMMAND, OCW2_SPECIFIC_EOI | irq);
        }
    }
}

/// Enable (unmask) a specific IRQ
pub fn enable_irq(irq: u8) {
    unsafe {
        if irq < 8 {
            let mask = inb(PIC1_DATA);
            outb(PIC1_DATA, mask & !(1 << irq));
        } else {
            let mask = inb(PIC2_DATA);
            outb(PIC2_DATA, mask & !(1 << (irq - 8)));
            // Also ensure cascade (IRQ 2) is enabled
            let master_mask = inb(PIC1_DATA);
            outb(PIC1_DATA, master_mask & !(1 << 2));
        }
    }
}

/// Disable (mask) a specific IRQ
pub fn disable_irq(irq: u8) {
    unsafe {
        if irq < 8 {
            let mask = inb(PIC1_DATA);
            outb(PIC1_DATA, mask | (1 << irq));
        } else {
            let mask = inb(PIC2_DATA);
            outb(PIC2_DATA, mask | (1 << (irq - 8)));
        }
    }
}

/// Get the current IRQ mask
pub fn get_mask() -> u16 {
    unsafe {
        let master = inb(PIC1_DATA) as u16;
        let slave = inb(PIC2_DATA) as u16;
        master | (slave << 8)
    }
}

/// Set the IRQ mask
pub fn set_mask(mask: u16) {
    unsafe {
        outb(PIC1_DATA, (mask & 0xFF) as u8);
        outb(PIC2_DATA, ((mask >> 8) & 0xFF) as u8);
    }
}

/// Read the In-Service Register (ISR)
/// Returns which IRQs are currently being serviced
pub fn read_isr() -> u16 {
    unsafe {
        outb(PIC1_COMMAND, OCW3_READ_ISR);
        outb(PIC2_COMMAND, OCW3_READ_ISR);
        let master = inb(PIC1_COMMAND) as u16;
        let slave = inb(PIC2_COMMAND) as u16;
        master | (slave << 8)
    }
}

/// Read the Interrupt Request Register (IRR)
/// Returns which IRQs have pending interrupts
pub fn read_irr() -> u16 {
    unsafe {
        outb(PIC1_COMMAND, OCW3_READ_IRR);
        outb(PIC2_COMMAND, OCW3_READ_IRR);
        let master = inb(PIC1_COMMAND) as u16;
        let slave = inb(PIC2_COMMAND) as u16;
        master | (slave << 8)
    }
}

/// Check if an IRQ is a spurious interrupt
/// For IRQ 7 (master) or IRQ 15 (slave)
pub fn is_spurious(irq: u8) -> bool {
    if irq == 7 {
        // Check if IRQ 7 is really in service
        unsafe {
            outb(PIC1_COMMAND, OCW3_READ_ISR);
            let isr = inb(PIC1_COMMAND);
            (isr & 0x80) == 0  // Spurious if bit 7 not set
        }
    } else if irq == 15 {
        // Check if IRQ 15 is really in service
        unsafe {
            outb(PIC2_COMMAND, OCW3_READ_ISR);
            let isr = inb(PIC2_COMMAND);
            if (isr & 0x80) == 0 {
                // Spurious IRQ 15, but still need to send EOI to master
                outb(PIC1_COMMAND, OCW2_EOI);
                return true;
            }
            false
        }
    } else {
        false
    }
}

/// Disable both PICs completely (for APIC migration)
pub fn disable() {
    unsafe {
        // Mask all interrupts
        outb(PIC1_DATA, 0xFF);
        outb(PIC2_DATA, 0xFF);
    }
}

// =============================================================================
// Convenience Functions for Armada E500 Devices
// =============================================================================

/// Enable timer interrupt (IRQ 0)
pub fn enable_timer() {
    enable_irq(irq::TIMER);
}

/// Enable keyboard interrupt (IRQ 1)
pub fn enable_keyboard() {
    enable_irq(irq::KEYBOARD);
}

/// Enable touchpad/mouse interrupt (IRQ 12)
pub fn enable_mouse() {
    enable_irq(irq::PS2_MOUSE);
}

/// Enable primary IDE interrupt (IRQ 14)
pub fn enable_primary_ide() {
    enable_irq(irq::PRIMARY_IDE);
}

/// Enable secondary IDE interrupt (IRQ 15)
pub fn enable_secondary_ide() {
    enable_irq(irq::SECONDARY_IDE);
}

/// Enable audio interrupt (IRQ 5) for ESS Maestro 2E
pub fn enable_audio() {
    enable_irq(irq::AUDIO);
}

/// Enable CardBus interrupt (IRQ 10) for TI PCI1225
pub fn enable_cardbus() {
    enable_irq(irq::CARDBUS);
}

/// Standard emulator setup: timer, keyboard, mouse
pub fn setup_for_emulator() {
    // Disable all first
    set_mask(0xFFFF);

    // Enable needed IRQs
    enable_timer();
    enable_keyboard();
    enable_mouse();

    // Enable cascade (always needed for slave PIC)
    enable_irq(irq::CASCADE);
}
