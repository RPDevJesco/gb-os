//! Minimal formatting utilities for debug output.
//!
//! Provides hex and decimal formatting without pulling in
//! the full core::fmt machinery.

/// Write a u64 as hexadecimal to a byte buffer.
/// Returns the number of bytes written.
pub fn write_hex(mut value: u64, buf: &mut [u8]) -> usize {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    if buf.len() < 2 {
        return 0;
    }

    // Handle zero case
    if value == 0 {
        if buf.len() >= 3 {
            buf[0] = b'0';
            buf[1] = b'x';
            buf[2] = b'0';
            return 3;
        }
        return 0;
    }

    // Write prefix
    buf[0] = b'0';
    buf[1] = b'x';

    // Count digits
    let mut temp = value;
    let mut digits = 0;
    while temp > 0 {
        digits += 1;
        temp >>= 4;
    }

    if buf.len() < 2 + digits {
        return 0;
    }

    // Write digits in reverse
    let mut pos = 2 + digits;
    while value > 0 {
        pos -= 1;
        buf[pos] = HEX_CHARS[(value & 0xF) as usize];
        value >>= 4;
    }

    2 + digits
}

/// Write a u64 as decimal to a byte buffer.
/// Returns the number of bytes written.
pub fn write_dec(mut value: u64, buf: &mut [u8]) -> usize {
    if buf.is_empty() {
        return 0;
    }

    // Handle zero case
    if value == 0 {
        buf[0] = b'0';
        return 1;
    }

    // Count digits
    let mut temp = value;
    let mut digits = 0;
    while temp > 0 {
        digits += 1;
        temp /= 10;
    }

    if buf.len() < digits {
        return 0;
    }

    // Write digits in reverse
    let mut pos = digits;
    while value > 0 {
        pos -= 1;
        buf[pos] = b'0' + (value % 10) as u8;
        value /= 10;
    }

    digits
}

/// Write a u32 as hexadecimal with fixed 8-char width.
pub fn write_hex32_fixed(value: u32, buf: &mut [u8; 10]) {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    buf[0] = b'0';
    buf[1] = b'x';

    for i in 0..8 {
        let nibble = (value >> (28 - i * 4)) & 0xF;
        buf[2 + i] = HEX_CHARS[nibble as usize];
    }
}

/// Write a u64 as hexadecimal with fixed 16-char width.
pub fn write_hex64_fixed(value: u64, buf: &mut [u8; 18]) {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    buf[0] = b'0';
    buf[1] = b'x';

    for i in 0..16 {
        let nibble = (value >> (60 - i * 4)) & 0xF;
        buf[2 + i] = HEX_CHARS[nibble as usize];
    }
}

/// Simple macro for printing to a Serial interface.
#[macro_export]
macro_rules! print {
    ($serial:expr, $($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($serial, $($arg)*);
    }};
}

/// Simple macro for printing with newline to a Serial interface.
#[macro_export]
macro_rules! println {
    ($serial:expr) => {{
        $crate::Serial::write_str($serial, "\r\n");
    }};
    ($serial:expr, $($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($serial, $($arg)*);
        $crate::Serial::write_str($serial, "\r\n");
    }};
}

/// Wrapper to implement core::fmt::Write for any Serial impl.
pub struct SerialWriter<'a, S: crate::Serial>(pub &'a mut S);

impl<'a, S: crate::Serial> core::fmt::Write for SerialWriter<'a, S> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.write_str(s);
        Ok(())
    }
}
