//! x86 I/O Port Access
//!
//! Provides safe wrappers for port I/O operations.

/// Read a byte from an I/O port
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!(
        "in al, dx",
        out("al") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    value
}

/// Write a byte to an I/O port
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// Read a word (16-bit) from an I/O port
#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    core::arch::asm!(
        "in ax, dx",
        out("ax") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    value
}

/// Write a word (16-bit) to an I/O port
#[inline]
pub unsafe fn outw(port: u16, value: u16) {
    core::arch::asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// Read a dword (32-bit) from an I/O port
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    core::arch::asm!(
        "in eax, dx",
        out("eax") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    value
}

/// Write a dword (32-bit) to an I/O port
#[inline]
pub unsafe fn outl(port: u16, value: u32) {
    core::arch::asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// Small I/O delay (for slow devices)
#[inline]
pub unsafe fn io_wait() {
    // Writing to port 0x80 causes a small delay
    // This port is used for POST codes and is safe to write to
    outb(0x80, 0);
}

/// Read multiple bytes from an I/O port (string I/O)
#[inline]
pub unsafe fn insb(port: u16, buf: &mut [u8]) {
    for i in 0..buf.len() {
        buf[i] = inb(port);
    }
}

/// Write multiple bytes to an I/O port (string I/O)
#[inline]
pub unsafe fn outsb(port: u16, buf: &[u8]) {
    for &byte in buf {
        outb(port, byte);
    }
}

/// Read multiple words from an I/O port (string I/O)
#[inline]
pub unsafe fn insw(port: u16, buf: &mut [u16]) {
    for i in 0..buf.len() {
        buf[i] = inw(port);
    }
}

/// Write multiple words to an I/O port (string I/O)
#[inline]
pub unsafe fn outsw(port: u16, buf: &[u16]) {
    for &word in buf {
        outw(port, word);
    }
}

/// Read multiple dwords from an I/O port (string I/O)
#[inline]
pub unsafe fn insl(port: u16, buf: &mut [u32]) {
    for i in 0..buf.len() {
        buf[i] = inl(port);
    }
}

/// Write multiple dwords to an I/O port (string I/O)
#[inline]
pub unsafe fn outsl(port: u16, buf: &[u32]) {
    for &dword in buf {
        outl(port, dword);
    }
}
