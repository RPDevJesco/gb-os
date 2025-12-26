//! Graphics Output
//! 
//! Graphics Output Protocol (GOP) for framebuffer access.

use core::ffi::c_void;
use crate::uefi::*;
use crate::println;

/// Framebuffer information
#[derive(Clone, Copy, Debug)]
pub struct FramebufferInfo {
    pub base: u64,
    pub size: usize,
    pub width: u32,
    pub height: u32,
    pub stride: u32, // pixels per scan line
    pub pixel_format: PixelFormat,
}

/// Pixel format abstraction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb,  // R at lowest address
    Bgr,  // B at lowest address
    Mask { red: u32, green: u32, blue: u32 },
    Unknown,
}

impl PixelFormat {
    /// Convert RGB values to native pixel format
    pub fn encode(&self, r: u8, g: u8, b: u8) -> u32 {
        match self {
            PixelFormat::Rgb => ((r as u32) << 0) | ((g as u32) << 8) | ((b as u32) << 16),
            PixelFormat::Bgr => ((b as u32) << 0) | ((g as u32) << 8) | ((r as u32) << 16),
            PixelFormat::Mask { red, green, blue } => {
                // Simplified - assumes 8-bit channels
                let r_shift = red.trailing_zeros();
                let g_shift = green.trailing_zeros();
                let b_shift = blue.trailing_zeros();
                ((r as u32) << r_shift) | ((g as u32) << g_shift) | ((b as u32) << b_shift)
            }
            PixelFormat::Unknown => 0,
        }
    }
}

/// Graphics handle
pub struct Graphics {
    protocol: *mut EFI_GRAPHICS_OUTPUT_PROTOCOL,
    pub info: FramebufferInfo,
}

impl Graphics {
    /// Initialize graphics from boot services
    /// 
    /// # Safety
    /// Caller must ensure boot_services is a valid pointer
    pub unsafe fn new(boot_services: *mut EFI_BOOT_SERVICES) -> Result<Self, EFI_STATUS> {
        unsafe {
            let mut protocol: *mut c_void = core::ptr::null_mut();
            
            let status = ((*boot_services).locate_protocol)(
                &EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID,
                core::ptr::null_mut(),
                &mut protocol,
            );
            
            if status != EFI_SUCCESS {
                return Err(status);
            }
            
            let gop = protocol as *mut EFI_GRAPHICS_OUTPUT_PROTOCOL;
            let mode = (*gop).mode;
            let mode_info = (*mode).info;
            
            let pixel_format = match (*mode_info).pixel_format {
                EFI_GRAPHICS_PIXEL_FORMAT::PixelRedGreenBlueReserved8BitPerColor => PixelFormat::Rgb,
                EFI_GRAPHICS_PIXEL_FORMAT::PixelBlueGreenRedReserved8BitPerColor => PixelFormat::Bgr,
                EFI_GRAPHICS_PIXEL_FORMAT::PixelBitMask => {
                    let mask = (*mode_info).pixel_information;
                    PixelFormat::Mask {
                        red: mask.red_mask,
                        green: mask.green_mask,
                        blue: mask.blue_mask,
                    }
                }
                _ => PixelFormat::Unknown,
            };
            
            let info = FramebufferInfo {
                base: (*mode).frame_buffer_base,
                size: (*mode).frame_buffer_size,
                width: (*mode_info).horizontal_resolution,
                height: (*mode_info).vertical_resolution,
                stride: (*mode_info).pixels_per_scan_line,
                pixel_format,
            };
            
            Ok(Self { protocol: gop, info })
        }
    }
    
    /// List available video modes
    pub fn list_modes(&self) {
        unsafe {
            let mode = (*self.protocol).mode;
            let max_mode = (*mode).max_mode;
            
            println!("Available video modes:");
            
            for i in 0..max_mode {
                let mut size: UINTN = 0;
                let mut info: *mut EFI_GRAPHICS_OUTPUT_MODE_INFORMATION = core::ptr::null_mut();
                
                let status = ((*self.protocol).query_mode)(self.protocol, i, &mut size, &mut info);
                
                if status == EFI_SUCCESS {
                    let format = match (*info).pixel_format {
                        EFI_GRAPHICS_PIXEL_FORMAT::PixelRedGreenBlueReserved8BitPerColor => "RGB",
                        EFI_GRAPHICS_PIXEL_FORMAT::PixelBlueGreenRedReserved8BitPerColor => "BGR",
                        EFI_GRAPHICS_PIXEL_FORMAT::PixelBitMask => "Mask",
                        _ => "?",
                    };
                    
                    let current = if i == (*mode).mode { " *" } else { "" };
                    
                    println!(
                        "  [{:2}] {}x{} {}{}",
                        i,
                        (*info).horizontal_resolution,
                        (*info).vertical_resolution,
                        format,
                        current
                    );
                }
            }
        }
    }
    
    /// Set video mode by index
    #[allow(dead_code)]
    pub fn set_mode(&mut self, mode: u32) -> Result<(), EFI_STATUS> {
        let status = unsafe { ((*self.protocol).set_mode)(self.protocol, mode) };
        
        if status != EFI_SUCCESS {
            return Err(status);
        }
        
        // Update info
        unsafe {
            let gop_mode = (*self.protocol).mode;
            let mode_info = (*gop_mode).info;
            
            self.info.base = (*gop_mode).frame_buffer_base;
            self.info.size = (*gop_mode).frame_buffer_size;
            self.info.width = (*mode_info).horizontal_resolution;
            self.info.height = (*mode_info).vertical_resolution;
            self.info.stride = (*mode_info).pixels_per_scan_line;
        }
        
        Ok(())
    }
    
    /// Find and set best available mode
    #[allow(dead_code)]
    pub fn set_best_mode(&mut self) -> Result<(), EFI_STATUS> {
        let mut best_mode = 0u32;
        let mut best_pixels = 0u64;
        
        unsafe {
            let mode = (*self.protocol).mode;
            let max_mode = (*mode).max_mode;
            
            for i in 0..max_mode {
                let mut size: UINTN = 0;
                let mut info: *mut EFI_GRAPHICS_OUTPUT_MODE_INFORMATION = core::ptr::null_mut();
                
                if ((*self.protocol).query_mode)(self.protocol, i, &mut size, &mut info) == EFI_SUCCESS {
                    let pixels = (*info).horizontal_resolution as u64 * (*info).vertical_resolution as u64;
                    if pixels > best_pixels {
                        best_pixels = pixels;
                        best_mode = i;
                    }
                }
            }
        }
        
        self.set_mode(best_mode)
    }
    
    /// Get raw framebuffer pointer
    pub fn framebuffer(&self) -> *mut u32 {
        self.info.base as *mut u32
    }
    
    /// Plot a pixel
    pub fn put_pixel(&self, x: u32, y: u32, r: u8, g: u8, b: u8) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }
        
        let offset = (y * self.info.stride + x) as isize;
        let color = self.info.pixel_format.encode(r, g, b);
        
        unsafe {
            *self.framebuffer().offset(offset) = color;
        }
    }
    
    /// Fill entire screen with color
    pub fn clear(&self, r: u8, g: u8, b: u8) {
        let color = self.info.pixel_format.encode(r, g, b);
        let fb = self.framebuffer();
        
        for y in 0..self.info.height {
            for x in 0..self.info.width {
                let offset = (y * self.info.stride + x) as isize;
                unsafe {
                    *fb.offset(offset) = color;
                }
            }
        }
    }
    
    /// Draw a filled rectangle
    pub fn fill_rect(&self, x: u32, y: u32, w: u32, h: u32, r: u8, g: u8, b: u8) {
        let color = self.info.pixel_format.encode(r, g, b);
        let fb = self.framebuffer();
        
        let x_end = (x + w).min(self.info.width);
        let y_end = (y + h).min(self.info.height);
        
        for py in y..y_end {
            for px in x..x_end {
                let offset = (py * self.info.stride + px) as isize;
                unsafe {
                    *fb.offset(offset) = color;
                }
            }
        }
    }
    
    /// Draw a horizontal line
    #[allow(dead_code)]
    pub fn hline(&self, x: u32, y: u32, len: u32, r: u8, g: u8, b: u8) {
        if y >= self.info.height {
            return;
        }
        
        let color = self.info.pixel_format.encode(r, g, b);
        let fb = self.framebuffer();
        let x_end = (x + len).min(self.info.width);
        
        for px in x..x_end {
            let offset = (y * self.info.stride + px) as isize;
            unsafe {
                *fb.offset(offset) = color;
            }
        }
    }
    
    /// Draw a vertical line
    #[allow(dead_code)]
    pub fn vline(&self, x: u32, y: u32, len: u32, r: u8, g: u8, b: u8) {
        if x >= self.info.width {
            return;
        }
        
        let color = self.info.pixel_format.encode(r, g, b);
        let fb = self.framebuffer();
        let y_end = (y + len).min(self.info.height);
        
        for py in y..y_end {
            let offset = (py * self.info.stride + x) as isize;
            unsafe {
                *fb.offset(offset) = color;
            }
        }
    }
    
    /// Draw rectangle outline
    pub fn rect(&self, x: u32, y: u32, w: u32, h: u32, r: u8, g: u8, b: u8) {
        self.hline(x, y, w, r, g, b);
        self.hline(x, y + h - 1, w, r, g, b);
        self.vline(x, y, h, r, g, b);
        self.vline(x + w - 1, y, h, r, g, b);
    }
}

/// Simple 8x8 bitmap font for basic text rendering
pub static FONT_8X8: [[u8; 8]; 128] = {
    let mut font = [[0u8; 8]; 128];
    
    // Space
    font[32] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    
    // Numbers 0-9
    font[48] = [0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00]; // 0
    font[49] = [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00]; // 1
    font[50] = [0x3C, 0x66, 0x06, 0x1C, 0x30, 0x66, 0x7E, 0x00]; // 2
    font[51] = [0x3C, 0x66, 0x06, 0x1C, 0x06, 0x66, 0x3C, 0x00]; // 3
    font[52] = [0x0E, 0x1E, 0x36, 0x66, 0x7F, 0x06, 0x06, 0x00]; // 4
    font[53] = [0x7E, 0x60, 0x7C, 0x06, 0x06, 0x66, 0x3C, 0x00]; // 5
    font[54] = [0x1C, 0x30, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00]; // 6
    font[55] = [0x7E, 0x66, 0x06, 0x0C, 0x18, 0x18, 0x18, 0x00]; // 7
    font[56] = [0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00]; // 8
    font[57] = [0x3C, 0x66, 0x66, 0x3E, 0x06, 0x0C, 0x38, 0x00]; // 9
    
    // Letters A-Z
    font[65] = [0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x00]; // A
    font[66] = [0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00]; // B
    font[67] = [0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00]; // C
    font[68] = [0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00]; // D
    font[69] = [0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x7E, 0x00]; // E
    font[70] = [0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x60, 0x00]; // F
    font[71] = [0x3C, 0x66, 0x60, 0x6E, 0x66, 0x66, 0x3E, 0x00]; // G
    font[72] = [0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00]; // H
    font[73] = [0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00]; // I
    font[74] = [0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38, 0x00]; // J
    font[75] = [0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C, 0x66, 0x00]; // K
    font[76] = [0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7E, 0x00]; // L
    font[77] = [0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00]; // M
    font[78] = [0x66, 0x76, 0x7E, 0x7E, 0x6E, 0x66, 0x66, 0x00]; // N
    font[79] = [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00]; // O
    font[80] = [0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00]; // P
    font[81] = [0x3C, 0x66, 0x66, 0x66, 0x6A, 0x6C, 0x36, 0x00]; // Q
    font[82] = [0x7C, 0x66, 0x66, 0x7C, 0x6C, 0x66, 0x66, 0x00]; // R
    font[83] = [0x3C, 0x66, 0x60, 0x3C, 0x06, 0x66, 0x3C, 0x00]; // S
    font[84] = [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00]; // T
    font[85] = [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00]; // U
    font[86] = [0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00]; // V
    font[87] = [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00]; // W
    font[88] = [0x66, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x66, 0x00]; // X
    font[89] = [0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00]; // Y
    font[90] = [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x7E, 0x00]; // Z
    
    // Lowercase letters (same as uppercase for simplicity)
    font[97] = font[65]; font[98] = font[66]; font[99] = font[67];
    font[100] = font[68]; font[101] = font[69]; font[102] = font[70];
    font[103] = font[71]; font[104] = font[72]; font[105] = font[73];
    font[106] = font[74]; font[107] = font[75]; font[108] = font[76];
    font[109] = font[77]; font[110] = font[78]; font[111] = font[79];
    font[112] = font[80]; font[113] = font[81]; font[114] = font[82];
    font[115] = font[83]; font[116] = font[84]; font[117] = font[85];
    font[118] = font[86]; font[119] = font[87]; font[120] = font[88];
    font[121] = font[89]; font[122] = font[90];
    
    // Punctuation
    font[33] = [0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x00]; // !
    font[46] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00]; // .
    font[58] = [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00]; // :
    font[45] = [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00]; // -
    
    font
};

impl Graphics {
    /// Draw a character at position
    pub fn draw_char(&self, x: u32, y: u32, c: char, r: u8, g: u8, b: u8) {
        let idx = c as usize;
        if idx >= 128 {
            return;
        }
        
        let glyph = &FONT_8X8[idx];
        
        for row in 0..8 {
            let bits = glyph[row];
            for col in 0..8 {
                if (bits >> (7 - col)) & 1 == 1 {
                    self.put_pixel(x + col, y + row as u32, r, g, b);
                }
            }
        }
    }
    
    /// Draw a string at position
    pub fn draw_string(&self, x: u32, y: u32, s: &str, r: u8, g: u8, b: u8) {
        let mut cx = x;
        for c in s.chars() {
            if c == '\n' {
                continue;
            }
            self.draw_char(cx, y, c, r, g, b);
            cx += 8;
        }
    }
}
