//! Global Descriptor Table (GDT)
//!
//! Sets up the kernel's GDT for protected mode operation.
//! We use a flat memory model with separate code/data segments.

use core::mem::size_of;

/// GDT Entry (Segment Descriptor)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct GdtEntry {
    /// Limit bits 0-15
    limit_low: u16,
    /// Base bits 0-15
    base_low: u16,
    /// Base bits 16-23
    base_mid: u8,
    /// Access byte
    access: u8,
    /// Limit bits 16-19 + flags
    granularity: u8,
    /// Base bits 24-31
    base_high: u8,
}

impl GdtEntry {
    /// Create a null descriptor
    pub const fn null() -> Self {
        Self {
            limit_low: 0,
            base_low: 0,
            base_mid: 0,
            access: 0,
            granularity: 0,
            base_high: 0,
        }
    }
    
    /// Create a new GDT entry
    pub const fn new(base: u32, limit: u32, access: u8, flags: u8) -> Self {
        Self {
            limit_low: (limit & 0xFFFF) as u16,
            base_low: (base & 0xFFFF) as u16,
            base_mid: ((base >> 16) & 0xFF) as u8,
            access,
            granularity: ((limit >> 16) & 0x0F) as u8 | (flags << 4),
            base_high: ((base >> 24) & 0xFF) as u8,
        }
    }
    
    /// Create a kernel code segment descriptor
    pub const fn kernel_code() -> Self {
        // Base: 0, Limit: 0xFFFFF (4GB with 4KB granularity)
        // Access: Present, Ring 0, Code segment, Executable, Readable
        // Flags: 4KB granularity, 32-bit
        Self::new(0, 0xFFFFF, 0b10011010, 0b1100)
    }
    
    /// Create a kernel data segment descriptor
    pub const fn kernel_data() -> Self {
        // Base: 0, Limit: 0xFFFFF (4GB with 4KB granularity)
        // Access: Present, Ring 0, Data segment, Writable
        // Flags: 4KB granularity, 32-bit
        Self::new(0, 0xFFFFF, 0b10010010, 0b1100)
    }
    
    /// Create a user code segment descriptor
    pub const fn user_code() -> Self {
        // Same as kernel but Ring 3
        Self::new(0, 0xFFFFF, 0b11111010, 0b1100)
    }
    
    /// Create a user data segment descriptor
    pub const fn user_data() -> Self {
        // Same as kernel data but Ring 3
        Self::new(0, 0xFFFFF, 0b11110010, 0b1100)
    }
}

/// GDT Pointer structure for LGDT instruction
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct GdtPointer {
    /// Size of GDT - 1
    limit: u16,
    /// Linear address of GDT
    base: u32,
}

/// Segment selectors
pub mod selectors {
    pub const KERNEL_CODE: u16 = 0x08;  // Index 1, GDT, Ring 0
    pub const KERNEL_DATA: u16 = 0x10;  // Index 2, GDT, Ring 0
    pub const USER_CODE: u16 = 0x18 | 3;  // Index 3, GDT, Ring 3
    pub const USER_DATA: u16 = 0x20 | 3;  // Index 4, GDT, Ring 3
    pub const TSS: u16 = 0x28;  // Index 5, GDT, Ring 0
}

/// Wrapper for aligned GDT
#[repr(C, align(8))]
struct AlignedGdt([GdtEntry; 6]);

/// The Global Descriptor Table
/// 
/// Layout:
/// - 0x00: Null descriptor
/// - 0x08: Kernel code segment
/// - 0x10: Kernel data segment
/// - 0x18: User code segment
/// - 0x20: User data segment
/// - 0x28: TSS (set up later)
static mut GDT: AlignedGdt = AlignedGdt([
    GdtEntry::null(),           // 0x00: Null
    GdtEntry::kernel_code(),    // 0x08: Kernel Code
    GdtEntry::kernel_data(),    // 0x10: Kernel Data
    GdtEntry::user_code(),      // 0x18: User Code
    GdtEntry::user_data(),      // 0x20: User Data
    GdtEntry::null(),           // 0x28: TSS (placeholder)
]);

/// GDT pointer for LGDT instruction
static mut GDT_PTR: GdtPointer = GdtPointer {
    limit: 0,
    base: 0,
};

/// Initialize the GDT
pub fn init() {
    unsafe {
        // Set up GDT pointer
        GDT_PTR.limit = (size_of::<[GdtEntry; 6]>() - 1) as u16;
        GDT_PTR.base = GDT.0.as_ptr() as u32;
        
        // Load GDT
        core::arch::asm!(
            "lgdt [{}]",
            in(reg) &GDT_PTR,
            options(nostack, preserves_flags)
        );
        
        // Reload segment registers
        // We need to do a far jump to reload CS
        core::arch::asm!(
            // Reload data segments
            "mov ax, 0x10",     // Kernel data selector
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            // Far jump to reload CS (kernel code selector 0x08)
            "push 0x08",        // CS
            "lea eax, [2f]",    // Get address of label 2
            "push eax",
            "retf",             // Far return = pop EIP, pop CS
            "2:",
            out("eax") _,
            options(nostack)
        );
    }
}

/// Set up the TSS entry (called after memory manager is ready)
pub fn set_tss(tss_base: u32, tss_limit: u32) {
    unsafe {
        // TSS descriptor: Present, Ring 0, Type 0x9 (32-bit TSS available)
        GDT.0[5] = GdtEntry::new(tss_base, tss_limit, 0b10001001, 0b0000);
    }
}

/// Load the TSS
pub fn load_tss() {
    unsafe {
        core::arch::asm!(
            "ltr ax",
            in("ax") selectors::TSS,
            options(nostack, preserves_flags)
        );
    }
}
