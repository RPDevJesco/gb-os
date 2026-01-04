//! Boot information structure.

/// Information passed from bootloader to kernel.
#[derive(Debug, Clone, Copy)]
pub struct BootInfo {
    /// Physical memory available (bytes)
    pub memory_size: u64,
    /// Kernel load address
    pub kernel_start: u64,
    /// Kernel end address
    pub kernel_end: u64,
    /// Framebuffer address (if available)
    pub framebuffer: Option<FramebufferInfo>,
}

/// Framebuffer information.
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    pub address: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u8,
}

impl BootInfo {
    pub const fn empty() -> Self {
        Self {
            memory_size: 0,
            kernel_start: 0,
            kernel_end: 0,
            framebuffer: None,
        }
    }
}
