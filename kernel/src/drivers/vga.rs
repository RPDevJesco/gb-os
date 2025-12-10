//! VGA/VESA Display Driver
//!
//! Supports both VGA text mode and VESA linear framebuffer.
//! This is the foundation for the Plan 9-style GUI.

use core::fmt;

/// VGA text mode colors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// VGA text mode color attribute
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    pub const fn new(foreground: Color, background: Color) -> Self {
        Self((background as u8) << 4 | (foreground as u8))
    }
}

/// Display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// VGA 80x25 text mode
    TextMode,
    /// VESA linear framebuffer
    Framebuffer,
}

/// Display writer - unified interface for text and graphics
pub struct Writer {
    mode: DisplayMode,
    // Text mode state
    column: usize,
    row: usize,
    color: ColorCode,
    // Display dimensions
    width: usize,
    height: usize,
    // Framebuffer info (for VESA mode)
    framebuffer: *mut u8,
    pitch: usize,
    bpp: usize,
    // Font for framebuffer text rendering
    char_width: usize,
    char_height: usize,
}

/// Global writer instance
pub static mut WRITER: Option<Writer> = None;

impl Writer {
    /// Create a new text mode writer
    pub fn text_mode() -> Self {
        Self {
            mode: DisplayMode::TextMode,
            column: 0,
            row: 0,
            color: ColorCode::new(Color::LightGray, Color::Black),
            width: 80,
            height: 25,
            framebuffer: 0xB8000 as *mut u8,
            pitch: 160,
            bpp: 16,
            char_width: 1,
            char_height: 1,
        }
    }
    
    /// Create a new framebuffer writer
    pub fn framebuffer(addr: u32, width: u32, height: u32, bpp: u32, pitch: u32) -> Self {
        // For framebuffer, we use a simple 8x16 font
        let char_width = 8;
        let char_height = 16;
        
        Self {
            mode: DisplayMode::Framebuffer,
            column: 0,
            row: 0,
            color: ColorCode::new(Color::LightGray, Color::Black),
            width: (width as usize) / char_width,
            height: (height as usize) / char_height,
            framebuffer: addr as *mut u8,
            pitch: pitch as usize,
            bpp: bpp as usize,
            char_width,
            char_height,
        }
    }
    
    /// Clear the screen
    pub fn clear(&mut self) {
        match self.mode {
            DisplayMode::TextMode => {
                let blank = (self.color.0 as u16) << 8 | b' ' as u16;
                let buffer = self.framebuffer as *mut u16;
                for i in 0..(self.width * self.height) {
                    unsafe {
                        *buffer.add(i) = blank;
                    }
                }
            }
            DisplayMode::Framebuffer => {
                // Clear to black
                let total_bytes = self.pitch * self.height * self.char_height;
                unsafe {
                    core::ptr::write_bytes(self.framebuffer, 0, total_bytes);
                }
            }
        }
        self.column = 0;
        self.row = 0;
    }
    
    /// Set the text color
    pub fn set_color(&mut self, foreground: Color, background: Color) {
        self.color = ColorCode::new(foreground, background);
    }
    
    /// Write a single byte
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            b'\r' => self.column = 0,
            b'\t' => {
                // Tab to next 8-column boundary
                let spaces = 8 - (self.column % 8);
                for _ in 0..spaces {
                    self.write_byte(b' ');
                }
            }
            byte => {
                if self.column >= self.width {
                    self.new_line();
                }
                
                match self.mode {
                    DisplayMode::TextMode => {
                        self.write_text_char(byte);
                    }
                    DisplayMode::Framebuffer => {
                        self.draw_char(byte);
                    }
                }
                
                self.column += 1;
            }
        }
    }
    
    /// Write a character in text mode
    fn write_text_char(&mut self, byte: u8) {
        let offset = self.row * self.width + self.column;
        let buffer = self.framebuffer as *mut u16;
        let value = (self.color.0 as u16) << 8 | byte as u16;
        unsafe {
            *buffer.add(offset) = value;
        }
    }
    
    /// Draw a character in framebuffer mode
    fn draw_char(&mut self, byte: u8) {
        let x = self.column * self.char_width;
        let y = self.row * self.char_height;
        
        // Get font data for this character
        let font_data = get_font_char(byte);
        
        // Draw each pixel of the character
        for (row_idx, &font_row) in font_data.iter().enumerate() {
            for col_idx in 0..8 {
                let pixel_on = (font_row >> (7 - col_idx)) & 1 != 0;
                let px = x + col_idx;
                let py = y + row_idx;
                
                if pixel_on {
                    self.set_pixel(px, py, 0xAAAAAA); // Light gray
                } else {
                    self.set_pixel(px, py, 0x000000); // Black
                }
            }
        }
    }
    
    /// Set a pixel in framebuffer mode
    fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if self.mode != DisplayMode::Framebuffer {
            return;
        }
        
        let bytes_per_pixel = self.bpp / 8;
        let offset = y * self.pitch + x * bytes_per_pixel;
        
        unsafe {
            let pixel = self.framebuffer.add(offset);
            match self.bpp {
                32 => {
                    *(pixel as *mut u32) = color;
                }
                24 => {
                    *pixel = (color & 0xFF) as u8;
                    *pixel.add(1) = ((color >> 8) & 0xFF) as u8;
                    *pixel.add(2) = ((color >> 16) & 0xFF) as u8;
                }
                16 => {
                    // Convert 24-bit to 16-bit (RGB565)
                    let r = ((color >> 16) & 0xFF) as u16;
                    let g = ((color >> 8) & 0xFF) as u16;
                    let b = (color & 0xFF) as u16;
                    let rgb565 = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3);
                    *(pixel as *mut u16) = rgb565;
                }
                _ => {}
            }
        }
    }
    
    /// Move to next line
    fn new_line(&mut self) {
        self.column = 0;
        self.row += 1;
        
        if self.row >= self.height {
            self.scroll();
        }
    }
    
    /// Scroll the screen up by one line
    fn scroll(&mut self) {
        match self.mode {
            DisplayMode::TextMode => {
                let buffer = self.framebuffer as *mut u16;
                unsafe {
                    // Move all lines up
                    core::ptr::copy(
                        buffer.add(self.width),
                        buffer,
                        self.width * (self.height - 1)
                    );
                    // Clear last line
                    let blank = (self.color.0 as u16) << 8 | b' ' as u16;
                    for i in 0..self.width {
                        *buffer.add((self.height - 1) * self.width + i) = blank;
                    }
                }
            }
            DisplayMode::Framebuffer => {
                let line_bytes = self.pitch * self.char_height;
                let total_lines = self.height;
                unsafe {
                    // Move all lines up
                    core::ptr::copy(
                        self.framebuffer.add(line_bytes),
                        self.framebuffer,
                        line_bytes * (total_lines - 1)
                    );
                    // Clear last line
                    core::ptr::write_bytes(
                        self.framebuffer.add(line_bytes * (total_lines - 1)),
                        0,
                        line_bytes
                    );
                }
            }
        }
        
        self.row = self.height - 1;
    }
    
    /// Write a string
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // Printable ASCII or newline
                0x20..=0x7E | b'\n' | b'\r' | b'\t' => self.write_byte(byte),
                // Non-printable, print a placeholder
                _ => self.write_byte(0xFE),
            }
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

/// Initialize VGA text mode
pub unsafe fn init_text_mode() {
    let mut writer = Writer::text_mode();
    writer.clear();
    WRITER = Some(writer);
}

/// Initialize VESA framebuffer mode
pub unsafe fn init_framebuffer(addr: u32, width: u32, height: u32, bpp: u32, pitch: u32) {
    let mut writer = Writer::framebuffer(addr, width, height, bpp, pitch);
    writer.clear();
    WRITER = Some(writer);
}

// Simple 8x16 bitmap font (subset for demo)
// In production, load a proper font file
fn get_font_char(c: u8) -> &'static [u8; 16] {
    // Basic font data - just enough to show text
    static FONT: [[u8; 16]; 128] = {
        let mut font = [[0u8; 16]; 128];
        
        // Space
        font[b' ' as usize] = [0; 16];
        
        // We'll define a minimal set of characters
        // In a real OS, you'd load a proper font
        
        font
    };
    
    // For now, return a simple pattern for any printable char
    static DEFAULT_CHAR: [u8; 16] = [
        0x00, 0x00, 0x7E, 0x81, 0xA5, 0x81, 0x81, 0xBD,
        0x99, 0x81, 0x81, 0x7E, 0x00, 0x00, 0x00, 0x00,
    ];
    
    // For most characters, use a simple block pattern
    static BLOCK_CHAR: [u8; 16] = [
        0x00, 0x00, 0x00, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
        0x3C, 0x3C, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    
    if c == b' ' {
        &[0; 16]
    } else if c >= 0x20 && c < 0x7F {
        &BLOCK_CHAR
    } else {
        &DEFAULT_CHAR
    }
}

// Macros for convenient printing
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        if let Some(writer) = unsafe { $crate::drivers::vga::WRITER.as_mut() } {
            let _ = write!(writer, $($arg)*);
        }
    }};
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
