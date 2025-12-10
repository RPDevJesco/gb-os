//! Programmable Interrupt Controller (8259 PIC)
//!
//! The 8259 PIC is used on older x86 systems to manage hardware interrupts.
//! We remap IRQs 0-15 to interrupts 32-47 to avoid conflicts with CPU exceptions.

use super::io::{outb, inb, io_wait};

// PIC ports
const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

// PIC commands
const ICW1_INIT: u8 = 0x10;
const ICW1_ICW4: u8 = 0x01;
const ICW4_8086: u8 = 0x01;
const PIC_EOI: u8 = 0x20;

/// IRQ base for master PIC (IRQ 0-7 -> INT 32-39)
pub const IRQ_BASE_MASTER: u8 = 32;
/// IRQ base for slave PIC (IRQ 8-15 -> INT 40-47)
pub const IRQ_BASE_SLAVE: u8 = 40;

/// Initialize the 8259 PIC
///
/// Remaps IRQs to avoid conflicts with CPU exceptions:
/// - IRQ 0-7  -> INT 32-39
/// - IRQ 8-15 -> INT 40-47
pub fn init() {
    unsafe {
        // Save masks
        let mask1 = inb(PIC1_DATA);
        let mask2 = inb(PIC2_DATA);
        
        // Start initialization sequence (cascade mode)
        outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();
        outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();
        
        // Set vector offsets
        outb(PIC1_DATA, IRQ_BASE_MASTER);  // Master: IRQ 0-7 -> INT 32-39
        io_wait();
        outb(PIC2_DATA, IRQ_BASE_SLAVE);   // Slave: IRQ 8-15 -> INT 40-47
        io_wait();
        
        // Tell Master about Slave at IRQ2
        outb(PIC1_DATA, 4);  // Slave on IRQ2 (bit 2)
        io_wait();
        outb(PIC2_DATA, 2);  // Slave cascade identity
        io_wait();
        
        // Set 8086 mode
        outb(PIC1_DATA, ICW4_8086);
        io_wait();
        outb(PIC2_DATA, ICW4_8086);
        io_wait();
        
        // Restore masks (or set new ones)
        // Enable IRQ0 (timer), IRQ1 (keyboard), IRQ2 (cascade to slave)
        outb(PIC1_DATA, 0b11111000);  // Enable IRQ0, IRQ1, IRQ2
        io_wait();
        // Enable IRQ12 (mouse) on slave
        outb(PIC2_DATA, 0b11101111);  // Enable IRQ12 (bit 4 = 0)
        io_wait();
    }
}

/// Send End of Interrupt (EOI) signal
///
/// Must be called at the end of every IRQ handler.
pub fn send_eoi(interrupt: u8) {
    unsafe {
        // If it's from the slave PIC, send EOI to both
        if interrupt >= IRQ_BASE_SLAVE {
            outb(PIC2_COMMAND, PIC_EOI);
        }
        outb(PIC1_COMMAND, PIC_EOI);
    }
}

/// Enable a specific IRQ
pub fn enable_irq(irq: u8) {
    unsafe {
        if irq < 8 {
            let mask = inb(PIC1_DATA);
            outb(PIC1_DATA, mask & !(1 << irq));
        } else {
            let mask = inb(PIC2_DATA);
            outb(PIC2_DATA, mask & !(1 << (irq - 8)));
        }
    }
}

/// Disable a specific IRQ
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

/// Disable the PIC (for use with APIC)
pub fn disable() {
    unsafe {
        outb(PIC1_DATA, 0xFF);
        outb(PIC2_DATA, 0xFF);
    }
}

/// Get the current IRQ mask
pub fn get_mask() -> u16 {
    unsafe {
        let low = inb(PIC1_DATA) as u16;
        let high = inb(PIC2_DATA) as u16;
        (high << 8) | low
    }
}

/// Set the IRQ mask
pub fn set_mask(mask: u16) {
    unsafe {
        outb(PIC1_DATA, (mask & 0xFF) as u8);
        outb(PIC2_DATA, ((mask >> 8) & 0xFF) as u8);
    }
}
