//! Console I/O Abstraction
//! 
//! Provides print/println macros and input handling for UEFI.

#![allow(static_mut_refs)]

use core::fmt::{self, Write};
use core::sync::atomic::{AtomicPtr, Ordering};
use crate::uefi::*;

/// Global console pointer - set during initialization
static CONSOLE_PTR: AtomicPtr<Console> = AtomicPtr::new(core::ptr::null_mut());

/// Console wrapper around UEFI text protocols
pub struct Console {
    con_out: *mut EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL,
    con_in: *mut EFI_SIMPLE_TEXT_INPUT_PROTOCOL,
}

// Safety: Console is only accessed from a single thread in UEFI environment
unsafe impl Sync for Console {}
unsafe impl Send for Console {}

/// Static storage for the console - use a raw byte array
/// We'll initialize this manually
static mut CONSOLE_STORAGE: [u8; core::mem::size_of::<Console>()] = [0; core::mem::size_of::<Console>()];

impl Console {
    /// Create a new console from system table
    /// 
    /// # Safety
    /// Caller must ensure system_table is a valid pointer to EFI_SYSTEM_TABLE
    pub unsafe fn new(system_table: *mut EFI_SYSTEM_TABLE) -> Self {
        unsafe {
            Self {
                con_out: (*system_table).con_out,
                con_in: (*system_table).con_in,
            }
        }
    }
    
    /// Initialize the global console
    /// 
    /// # Safety
    /// Caller must ensure system_table is valid and this is called only once
    pub unsafe fn init(system_table: *mut EFI_SYSTEM_TABLE) {
        unsafe {
            let console = Console::new(system_table);
            let storage_ptr = CONSOLE_STORAGE.as_mut_ptr() as *mut Console;
            core::ptr::write(storage_ptr, console);
            CONSOLE_PTR.store(storage_ptr, Ordering::Release);
        }
    }
    
    /// Get global console reference
    pub fn get() -> Option<&'static mut Console> {
        let ptr = CONSOLE_PTR.load(Ordering::Acquire);
        if ptr.is_null() {
            None
        } else {
            // Safety: We only set this pointer once during init, and the Console
            // lives for the entire program lifetime in static storage
            unsafe { Some(&mut *ptr) }
        }
    }
    
    /// Clear the screen
    pub fn clear(&self) -> EFI_STATUS {
        unsafe { ((*self.con_out).clear_screen)(self.con_out) }
    }
    
    /// Set text color
    pub fn set_color(&self, foreground: UINTN, background: UINTN) -> EFI_STATUS {
        unsafe {
            ((*self.con_out).set_attribute)(self.con_out, efi_text_attr(foreground, background))
        }
    }
    
    /// Set cursor position
    #[allow(dead_code)]
    pub fn set_cursor(&self, col: UINTN, row: UINTN) -> EFI_STATUS {
        unsafe { ((*self.con_out).set_cursor_position)(self.con_out, col, row) }
    }
    
    /// Enable/disable cursor
    #[allow(dead_code)]
    pub fn enable_cursor(&self, visible: bool) -> EFI_STATUS {
        unsafe { ((*self.con_out).enable_cursor)(self.con_out, visible as BOOLEAN) }
    }
    
    /// Print a UTF-16 string directly
    pub fn print_raw(&self, s: &[u16]) -> EFI_STATUS {
        unsafe { ((*self.con_out).output_string)(self.con_out, s.as_ptr()) }
    }
    
    /// Print a single character
    pub fn putchar(&self, c: char) {
        let mut buf = [0u16; 3];
        let encoded = c.encode_utf16(&mut buf);
        // Null terminate
        buf[encoded.len()] = 0;
        let _ = self.print_raw(&buf);
    }
    
    /// Print a string (handles \n -> \r\n conversion)
    pub fn print_str(&self, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                self.putchar('\r');
            }
            self.putchar(c);
        }
    }
    
    /// Wait for a key press and return it
    pub fn wait_key(&self) -> Result<EFI_INPUT_KEY, EFI_STATUS> {
        unsafe {
            // Reset input
            let _ = ((*self.con_in).reset)(self.con_in, 0);
            
            let mut key = EFI_INPUT_KEY {
                scan_code: 0,
                unicode_char: 0,
            };
            
            // Poll for key
            loop {
                let status = ((*self.con_in).read_key_stroke)(self.con_in, &mut key);
                if status == EFI_SUCCESS {
                    return Ok(key);
                }
                if efi_error(status) && status != EFI_NOT_READY {
                    return Err(status);
                }
                // Small delay to prevent busy spinning
                core::hint::spin_loop();
            }
        }
    }
    
    /// Read a line of input (up to buffer size)
    #[allow(dead_code)]
    pub fn read_line(&self, buf: &mut [u8]) -> usize {
        let mut pos = 0;
        
        while pos < buf.len() - 1 {
            if let Ok(key) = self.wait_key() {
                let c = key.unicode_char;
                
                // Enter key
                if c == 0x000D {
                    self.print_str("\n");
                    break;
                }
                
                // Backspace
                if c == 0x0008 && pos > 0 {
                    pos -= 1;
                    self.print_str("\x08 \x08");
                    continue;
                }
                
                // Escape
                if key.scan_code == SCAN_ESC {
                    break;
                }
                
                // Printable ASCII
                if c >= 0x20 && c < 0x7F {
                    buf[pos] = c as u8;
                    pos += 1;
                    self.putchar(c as u8 as char);
                }
            }
        }
        
        buf[pos] = 0;
        pos
    }
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.print_str(s);
        Ok(())
    }
}

/// Writer that implements fmt::Write for use with write!/writeln!
pub struct ConsoleWriter;

impl Write for ConsoleWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Some(console) = Console::get() {
            console.print_str(s);
        }
        Ok(())
    }
}

/// Print formatted output
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::console::ConsoleWriter, $($arg)*);
    }};
}

/// Print formatted output with newline
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = writeln!($crate::console::ConsoleWriter, $($arg)*);
    }};
}

// =============================================================================
// Helper functions for printing status/hex/memory
// =============================================================================

/// Print a UEFI status code
#[allow(dead_code)]
pub fn print_status(status: EFI_STATUS) {
    let msg = match status {
        EFI_SUCCESS => "Success",
        EFI_LOAD_ERROR => "Load Error",
        EFI_INVALID_PARAMETER => "Invalid Parameter",
        EFI_UNSUPPORTED => "Unsupported",
        EFI_BAD_BUFFER_SIZE => "Bad Buffer Size",
        EFI_BUFFER_TOO_SMALL => "Buffer Too Small",
        EFI_NOT_READY => "Not Ready",
        EFI_DEVICE_ERROR => "Device Error",
        EFI_WRITE_PROTECTED => "Write Protected",
        EFI_OUT_OF_RESOURCES => "Out of Resources",
        EFI_NOT_FOUND => "Not Found",
        _ => "Unknown",
    };
    println!("Status: {} (0x{:X})", msg, status);
}

/// Print hex dump of memory region
#[allow(dead_code)]
pub fn hexdump(addr: *const u8, len: usize) {
    for i in 0..len {
        if i % 16 == 0 {
            if i > 0 {
                println!();
            }
            print!("{:016X}: ", addr as usize + i);
        }
        unsafe {
            print!("{:02X} ", *addr.add(i));
        }
    }
    println!();
}
