//! Interrupt Descriptor Table (IDT)
//!
//! Sets up interrupt handlers for exceptions and hardware interrupts.
//! In the EventChains architecture, interrupts can dispatch events.

use core::arch::global_asm;
use core::mem::size_of;
use super::gdt::selectors;
use super::pic;

/// IDT Entry (Interrupt Gate Descriptor)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    /// Offset bits 0-15
    offset_low: u16,
    /// Segment selector
    selector: u16,
    /// Reserved (must be 0)
    reserved: u8,
    /// Gate type and attributes
    type_attr: u8,
    /// Offset bits 16-31
    offset_high: u16,
}

impl IdtEntry {
    /// Create a null/absent entry
    pub const fn null() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            reserved: 0,
            type_attr: 0,
            offset_high: 0,
        }
    }

    /// Create an interrupt gate entry
    pub const fn interrupt_gate(handler: u32, selector: u16, dpl: u8) -> Self {
        Self {
            offset_low: (handler & 0xFFFF) as u16,
            selector,
            reserved: 0,
            // Present | DPL | 0 | Type (0xE = 32-bit interrupt gate)
            type_attr: 0x80 | ((dpl & 3) << 5) | 0x0E,
            offset_high: ((handler >> 16) & 0xFFFF) as u16,
        }
    }

    /// Create a trap gate entry (doesn't disable interrupts)
    pub const fn trap_gate(handler: u32, selector: u16, dpl: u8) -> Self {
        Self {
            offset_low: (handler & 0xFFFF) as u16,
            selector,
            reserved: 0,
            // Present | DPL | 0 | Type (0xF = 32-bit trap gate)
            type_attr: 0x80 | ((dpl & 3) << 5) | 0x0F,
            offset_high: ((handler >> 16) & 0xFFFF) as u16,
        }
    }

    /// Set the handler address
    pub fn set_handler(&mut self, handler: u32) {
        self.offset_low = (handler & 0xFFFF) as u16;
        self.offset_high = ((handler >> 16) & 0xFFFF) as u16;
    }
}

/// IDT Pointer structure for LIDT instruction
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtPointer {
    /// Size of IDT - 1
    limit: u16,
    /// Linear address of IDT
    base: u32,
}

/// Number of IDT entries (256 for x86)
const IDT_ENTRIES: usize = 256;

/// The Interrupt Descriptor Table
/// Wrapper for aligned IDT
#[repr(C, align(8))]
struct AlignedIdt([IdtEntry; IDT_ENTRIES]);

/// The Interrupt Descriptor Table
static mut IDT: AlignedIdt = AlignedIdt([IdtEntry::null(); IDT_ENTRIES]);

/// IDT pointer for LIDT instruction
static mut IDT_PTR: IdtPointer = IdtPointer {
    limit: 0,
    base: 0,
};

/// Simple tick counter for timer (if PIT module not available)
#[allow(dead_code)]
static mut TICK_COUNT: u32 = 0;

/// Interrupt frame pushed by CPU
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct InterruptFrame {
    // Pushed by our stub
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub esp_dummy: u32,  // ESP at pushad time
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,
    pub interrupt_number: u32,
    pub error_code: u32,
    // Pushed by CPU
    pub eip: u32,
    pub cs: u32,
    pub eflags: u32,
    // Only present on privilege level change
    pub user_esp: u32,
    pub user_ss: u32,
}

// External references to assembly stubs
extern "C" {
    fn isr_stub_0();
    fn isr_stub_1();
    fn isr_stub_2();
    fn isr_stub_3();
    fn isr_stub_4();
    fn isr_stub_5();
    fn isr_stub_6();
    fn isr_stub_7();
    fn isr_stub_8();
    fn isr_stub_9();
    fn isr_stub_10();
    fn isr_stub_11();
    fn isr_stub_12();
    fn isr_stub_13();
    fn isr_stub_14();
    fn isr_stub_15();
    fn isr_stub_16();
    fn isr_stub_17();
    fn isr_stub_18();
    fn isr_stub_19();
    fn isr_stub_default();
    fn irq_stub_0();
    fn irq_stub_1();
    fn irq_stub_12();
    fn irq_stub_default();
}

// ISR stubs in assembly using global_asm!
global_asm!(
    // Common handler - saves state, calls Rust handler, restores state
    ".global isr_common",
    "isr_common:",
    "    pushad",              // Save all registers
    "    mov eax, esp",        // Frame pointer
    "    push eax",
    "    call interrupt_handler",
    "    add esp, 4",
    "    popad",               // Restore registers
    "    add esp, 8",          // Remove error code and interrupt number
    "    iretd",               // Return from interrupt

    // ISR stubs without error codes
    ".global isr_stub_0",
    "isr_stub_0:",
    "    push 0",              // Dummy error code
    "    push 0",              // Interrupt number
    "    jmp isr_common",

    ".global isr_stub_1",
    "isr_stub_1:",
    "    push 0",
    "    push 1",
    "    jmp isr_common",

    ".global isr_stub_2",
    "isr_stub_2:",
    "    push 0",
    "    push 2",
    "    jmp isr_common",

    ".global isr_stub_3",
    "isr_stub_3:",
    "    push 0",
    "    push 3",
    "    jmp isr_common",

    ".global isr_stub_4",
    "isr_stub_4:",
    "    push 0",
    "    push 4",
    "    jmp isr_common",

    ".global isr_stub_5",
    "isr_stub_5:",
    "    push 0",
    "    push 5",
    "    jmp isr_common",

    ".global isr_stub_6",
    "isr_stub_6:",
    "    push 0",
    "    push 6",
    "    jmp isr_common",

    ".global isr_stub_7",
    "isr_stub_7:",
    "    push 0",
    "    push 7",
    "    jmp isr_common",

    // ISR 8 has error code
    ".global isr_stub_8",
    "isr_stub_8:",
    "    push 8",
    "    jmp isr_common",

    ".global isr_stub_9",
    "isr_stub_9:",
    "    push 0",
    "    push 9",
    "    jmp isr_common",

    // ISR 10-14 have error codes
    ".global isr_stub_10",
    "isr_stub_10:",
    "    push 10",
    "    jmp isr_common",

    ".global isr_stub_11",
    "isr_stub_11:",
    "    push 11",
    "    jmp isr_common",

    ".global isr_stub_12",
    "isr_stub_12:",
    "    push 12",
    "    jmp isr_common",

    ".global isr_stub_13",
    "isr_stub_13:",
    "    push 13",
    "    jmp isr_common",

    ".global isr_stub_14",
    "isr_stub_14:",
    "    push 14",
    "    jmp isr_common",

    ".global isr_stub_15",
    "isr_stub_15:",
    "    push 0",
    "    push 15",
    "    jmp isr_common",

    ".global isr_stub_16",
    "isr_stub_16:",
    "    push 0",
    "    push 16",
    "    jmp isr_common",

    // ISR 17 has error code
    ".global isr_stub_17",
    "isr_stub_17:",
    "    push 17",
    "    jmp isr_common",

    ".global isr_stub_18",
    "isr_stub_18:",
    "    push 0",
    "    push 18",
    "    jmp isr_common",

    ".global isr_stub_19",
    "isr_stub_19:",
    "    push 0",
    "    push 19",
    "    jmp isr_common",

    ".global isr_stub_default",
    "isr_stub_default:",
    "    push 0",
    "    push 255",
    "    jmp isr_common",

    // IRQ stubs (mapped to INT 32-47)
    ".global irq_stub_0",
    "irq_stub_0:",
    "    push 0",
    "    push 32",             // IRQ 0 = INT 32
    "    jmp isr_common",

    ".global irq_stub_1",
    "irq_stub_1:",
    "    push 0",
    "    push 33",             // IRQ 1 = INT 33
    "    jmp isr_common",

    ".global irq_stub_12",
    "irq_stub_12:",
    "    push 0",
    "    push 44",             // IRQ 12 = INT 44 (mouse/touchpad)
    "    jmp isr_common",

    ".global irq_stub_default",
    "irq_stub_default:",
    "    push 0",
    "    push 255",
    "    jmp isr_common",
);

/// Initialize the IDT
pub fn init() {
    // Initialize PIC first
    pic::init();

    unsafe {
        // Set up CPU exception handlers (interrupts 0-31)
        set_exception_handlers();

        // Set up IRQ handlers (interrupts 32-47)
        set_irq_handlers();

        // Set up IDT pointer
        IDT_PTR.limit = (size_of::<[IdtEntry; IDT_ENTRIES]>() - 1) as u16;
        IDT_PTR.base = IDT.0.as_ptr() as u32;

        // Load IDT
        core::arch::asm!(
        "lidt [{}]",
        in(reg) &IDT_PTR,
        options(nostack, preserves_flags)
        );
    }
}

/// Set up CPU exception handlers
unsafe fn set_exception_handlers() {
    IDT.0[0] = IdtEntry::interrupt_gate(isr_stub_0 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[1] = IdtEntry::interrupt_gate(isr_stub_1 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[2] = IdtEntry::interrupt_gate(isr_stub_2 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[3] = IdtEntry::trap_gate(isr_stub_3 as u32, selectors::KERNEL_CODE, 3);
    IDT.0[4] = IdtEntry::trap_gate(isr_stub_4 as u32, selectors::KERNEL_CODE, 3);
    IDT.0[5] = IdtEntry::interrupt_gate(isr_stub_5 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[6] = IdtEntry::interrupt_gate(isr_stub_6 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[7] = IdtEntry::interrupt_gate(isr_stub_7 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[8] = IdtEntry::interrupt_gate(isr_stub_8 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[9] = IdtEntry::interrupt_gate(isr_stub_9 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[10] = IdtEntry::interrupt_gate(isr_stub_10 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[11] = IdtEntry::interrupt_gate(isr_stub_11 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[12] = IdtEntry::interrupt_gate(isr_stub_12 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[13] = IdtEntry::interrupt_gate(isr_stub_13 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[14] = IdtEntry::interrupt_gate(isr_stub_14 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[15] = IdtEntry::interrupt_gate(isr_stub_15 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[16] = IdtEntry::interrupt_gate(isr_stub_16 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[17] = IdtEntry::interrupt_gate(isr_stub_17 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[18] = IdtEntry::interrupt_gate(isr_stub_18 as u32, selectors::KERNEL_CODE, 0);
    IDT.0[19] = IdtEntry::interrupt_gate(isr_stub_19 as u32, selectors::KERNEL_CODE, 0);

    // Fill rest of exceptions with default handler
    for i in 20..32 {
        IDT.0[i] = IdtEntry::interrupt_gate(isr_stub_default as u32, selectors::KERNEL_CODE, 0);
    }
}

/// Set up IRQ handlers (PIC interrupts mapped to 32-47)
unsafe fn set_irq_handlers() {
    // IRQ 0 - Timer (INT 32)
    IDT.0[32] = IdtEntry::interrupt_gate(irq_stub_0 as u32, selectors::KERNEL_CODE, 0);
    // IRQ 1 - Keyboard (INT 33)
    IDT.0[33] = IdtEntry::interrupt_gate(irq_stub_1 as u32, selectors::KERNEL_CODE, 0);

    // IRQ 2-11 use default handler
    for i in 2..12 {
        IDT.0[32 + i] = IdtEntry::interrupt_gate(irq_stub_default as u32, selectors::KERNEL_CODE, 0);
    }

    // IRQ 12 - PS/2 Mouse/Touchpad (INT 44) - dedicated handler
    IDT.0[44] = IdtEntry::interrupt_gate(irq_stub_12 as u32, selectors::KERNEL_CODE, 0);

    // IRQ 13-15 use default handler
    for i in 13..16 {
        IDT.0[32 + i] = IdtEntry::interrupt_gate(irq_stub_default as u32, selectors::KERNEL_CODE, 0);
    }
}

/// Main interrupt handler (called from assembly)
#[no_mangle]
extern "C" fn interrupt_handler(frame: &InterruptFrame) {
    let int_num = frame.interrupt_number;

    match int_num {
        // CPU Exceptions
        0 => exception_handler("Division by zero", frame),
        6 => exception_handler("Invalid opcode", frame),
        8 => exception_handler("Double fault", frame),
        13 => exception_handler("General protection fault", frame),
        14 => page_fault_handler(frame),

        // IRQs (32-47)
        32 => timer_handler(),
        33 => keyboard_handler(),
        44 => mouse_handler(),  // IRQ 12 = interrupt 44

        // Other IRQs
        32..=47 => {
            pic::send_eoi(int_num as u8);
        }

        _ => {
            // Unknown interrupt
        }
    }
}

fn exception_handler(name: &str, frame: &InterruptFrame) {
    // Write directly to VGA buffer for debugging
    unsafe {
        let vga = 0xB8000 as *mut u8;

        // Clear first line and write exception name
        for i in 0..80 {
            vga.add(i * 2).write_volatile(b' ');
            vga.add(i * 2 + 1).write_volatile(0x4F); // White on red
        }

        // Write "EXCEPTION: " prefix
        let prefix = b"EXCEPTION: ";
        for (i, &c) in prefix.iter().enumerate() {
            vga.add(i * 2).write_volatile(c);
        }

        // Write exception name
        for (i, c) in name.bytes().enumerate() {
            vga.add((prefix.len() + i) * 2).write_volatile(c);
        }

        // Write EIP on second line
        let eip_prefix = b"EIP: 0x";
        let line2 = 160; // Second line starts at offset 160
        for (i, &c) in eip_prefix.iter().enumerate() {
            vga.add(line2 + i * 2).write_volatile(c);
            vga.add(line2 + i * 2 + 1).write_volatile(0x4F);
        }

        // Write EIP as hex
        let eip = frame.eip;
        for i in 0..8 {
            let nibble = ((eip >> (28 - i * 4)) & 0xF) as u8;
            let c = if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 };
            vga.add(line2 + (eip_prefix.len() + i) * 2).write_volatile(c);
            vga.add(line2 + (eip_prefix.len() + i) * 2 + 1).write_volatile(0x4F);
        }

        // Also try the VGA writer if available
        if let Some(writer) = crate::drivers::vga::WRITER.as_mut() {
            use core::fmt::Write;
            let _ = writeln!(writer, "\n!!! EXCEPTION: {} !!!", name);
            let _ = writeln!(writer, "EIP: 0x{:08X}", frame.eip);
            let _ = writeln!(writer, "Error code: 0x{:08X}", frame.error_code);
            let _ = writeln!(writer, "EAX: 0x{:08X}  EBX: 0x{:08X}", frame.eax, frame.ebx);
            let _ = writeln!(writer, "ECX: 0x{:08X}  EDX: 0x{:08X}", frame.ecx, frame.edx);
            let _ = writeln!(writer, "ESI: 0x{:08X}  EDI: 0x{:08X}", frame.esi, frame.edi);
            let _ = writeln!(writer, "EBP: 0x{:08X}  CS:  0x{:04X}", frame.ebp, frame.cs);
        }
    }

    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}

fn page_fault_handler(frame: &InterruptFrame) {
    let fault_addr: u32;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) fault_addr);
    }

    // Write to VGA text buffer directly
    unsafe {
        let vga = 0xB8000 as *mut u8;

        // Clear first line
        for i in 0..80 {
            vga.add(i * 2).write_volatile(b' ');
            vga.add(i * 2 + 1).write_volatile(0x4F);
        }

        let msg = b"PAGE FAULT at 0x";
        for (i, &c) in msg.iter().enumerate() {
            vga.add(i * 2).write_volatile(c);
        }

        // Write fault address as hex
        for i in 0..8 {
            let nibble = ((fault_addr >> (28 - i * 4)) & 0xF) as u8;
            let c = if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 };
            vga.add((msg.len() + i) * 2).write_volatile(c);
        }

        if let Some(writer) = crate::drivers::vga::WRITER.as_mut() {
            use core::fmt::Write;
            let _ = writeln!(writer, "\n!!! PAGE FAULT !!!");
            let _ = writeln!(writer, "Faulting address: 0x{:08X}", fault_addr);
            let _ = writeln!(writer, "EIP: 0x{:08X}", frame.eip);
            let _ = writeln!(writer, "Error code: 0x{:08X}", frame.error_code);

            // Decode error code
            let present = (frame.error_code & 0x01) != 0;
            let write = (frame.error_code & 0x02) != 0;
            let user = (frame.error_code & 0x04) != 0;
            let reserved = (frame.error_code & 0x08) != 0;
            let _ = writeln!(writer, "  Present: {}, Write: {}, User: {}, Reserved: {}",
                             present, write, user, reserved);
        }
    }

    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}

fn timer_handler() {
    // Update PIT tick counter
    crate::arch::x86::pit::tick();

    // Send End of Interrupt
    pic::send_eoi(32);
}

/// Get current tick count (for compatibility)
pub fn ticks() -> u32 {
    crate::arch::x86::pit::ticks()
}

fn keyboard_handler() {
    let scancode = unsafe { super::io::inb(0x60) };

    // Process through keyboard driver
    unsafe {
        if let Some(_event) = crate::drivers::keyboard::KEYBOARD.process_scancode(scancode) {
            // Event will be handled by GUI event loop
        }
    }

    pic::send_eoi(33);
}

/// Mouse/Touchpad IRQ handler
/// Routes to Synaptics driver if initialized, otherwise to generic PS/2 mouse
fn mouse_handler() {
    // Check if data is from mouse (bit 5 of status indicates AUX data)
    let status = unsafe { super::io::inb(0x64) };
    if status & 0x20 == 0 {
        // Not mouse data, send EOI and return
        pic::send_eoi(44);
        return;
    }

    // Read the data byte
    let byte = unsafe { super::io::inb(0x60) };

    // Route to appropriate driver based on what's initialized
    // Check Synaptics first (preferred driver)
    if crate::drivers::synaptics::is_initialized() {
        crate::drivers::synaptics::handle_irq_byte(byte);
    } else {
        // Fall back to generic PS/2 mouse driver
        unsafe {
            crate::drivers::mouse::MOUSE.process_byte(byte);
        }
    }

    // IRQ12 is on the slave PIC, so we need to send EOI to both
    pic::send_eoi(44);
}
