//! Framebuffer Graphics
//!
//! Low-level graphics primitives for the linear framebuffer.

use super::{Color, Rect, Point, font};

/// Framebuffer for direct pixel manipulation
pub struct Framebuffer {
    /// Pointer to framebuffer memory
    buffer: *mut u8,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Bytes per pixel (2, 3, or 4)
    pub bpp: u32,
    /// Bytes per scanline
    pub pitch: u32,
}

impl Framebuffer {
    /// Create a new framebuffer wrapper
    /// 
    /// # Safety
    /// Caller must ensure the buffer pointer and dimensions are valid
    pub unsafe fn new(buffer: *mut u8, width: u32, height: u32, bpp: u32, pitch: u32) -> Self {
        Self {
            buffer,
            width,
            height,
            bpp,
            pitch,
        }
    }
    
    /// Set a single pixel
    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        
        let offset = (y as u32 * self.pitch + x as u32 * self.bpp) as usize;
        
        unsafe {
            let pixel = self.buffer.add(offset);
            match self.bpp {
                4 => {
                    // 32-bit BGRA
                    *pixel = color.b;
                    *pixel.add(1) = color.g;
                    *pixel.add(2) = color.r;
                    *pixel.add(3) = 0xFF;
                }
                3 => {
                    // 24-bit BGR
                    *pixel = color.b;
                    *pixel.add(1) = color.g;
                    *pixel.add(2) = color.r;
                }
                2 => {
                    // 16-bit RGB565
                    let r = (color.r >> 3) as u16;
                    let g = (color.g >> 2) as u16;
                    let b = (color.b >> 3) as u16;
                    let rgb565 = (r << 11) | (g << 5) | b;
                    *(pixel as *mut u16) = rgb565;
                }
                _ => {}
            }
        }
    }
    
    /// Get a pixel color (for reading back)
    pub fn get_pixel(&self, x: i32, y: i32) -> Option<Color> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        
        let offset = (y as u32 * self.pitch + x as u32 * self.bpp) as usize;
        
        unsafe {
            let pixel = self.buffer.add(offset);
            let color = match self.bpp {
                4 | 3 => Color::rgb(*pixel.add(2), *pixel.add(1), *pixel),
                2 => {
                    let rgb565 = *(pixel as *const u16);
                    let r = ((rgb565 >> 11) & 0x1F) as u8;
                    let g = ((rgb565 >> 5) & 0x3F) as u8;
                    let b = (rgb565 & 0x1F) as u8;
                    Color::rgb(r << 3, g << 2, b << 3)
                }
                _ => Color::BLACK,
            };
            Some(color)
        }
    }
    
    /// Fill entire screen with a color
    pub fn clear(&mut self, color: Color) {
        self.fill_rect(0, 0, self.width, self.height, color);
    }
    
    /// Fill a rectangle with a solid color
    pub fn fill_rect(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        // Clip to screen bounds
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let x1 = ((x + width as i32) as u32).min(self.width);
        let y1 = ((y + height as i32) as u32).min(self.height);
        
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        
        for py in y0..y1 {
            for px in x0..x1 {
                self.set_pixel(px as i32, py as i32, color);
            }
        }
    }
    
    /// Draw a rectangle outline
    pub fn draw_rect(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        // Top
        self.draw_hline(x, y, width, color);
        // Bottom
        self.draw_hline(x, y + height as i32 - 1, width, color);
        // Left
        self.draw_vline(x, y, height, color);
        // Right
        self.draw_vline(x + width as i32 - 1, y, height, color);
    }
    
    /// Draw a horizontal line
    pub fn draw_hline(&mut self, x: i32, y: i32, length: u32, color: Color) {
        for i in 0..length as i32 {
            self.set_pixel(x + i, y, color);
        }
    }
    
    /// Draw a vertical line
    pub fn draw_vline(&mut self, x: i32, y: i32, length: u32, color: Color) {
        for i in 0..length as i32 {
            self.set_pixel(x, y + i, color);
        }
    }
    
    /// Draw a line (Bresenham's algorithm)
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        
        let mut x = x0;
        let mut y = y0;
        
        loop {
            self.set_pixel(x, y, color);
            
            if x == x1 && y == y1 {
                break;
            }
            
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    
    /// Draw a single character at position
    pub fn draw_char(&mut self, x: i32, y: i32, c: char, fg: Color, bg: Option<Color>) {
        let glyph = font::get_char(c as u8);
        
        for (row_idx, &row) in glyph.iter().enumerate() {
            for col_idx in 0..8 {
                let px = x + col_idx;
                let py = y + row_idx as i32;
                
                let bit_set = (row >> (7 - col_idx)) & 1 != 0;
                
                if bit_set {
                    self.set_pixel(px, py, fg);
                } else if let Some(bg_color) = bg {
                    self.set_pixel(px, py, bg_color);
                }
            }
        }
    }
    
    /// Draw a string at position
    pub fn draw_string(&mut self, x: i32, y: i32, s: &str, fg: Color, bg: Option<Color>) {
        let mut cx = x;
        let mut cy = y;
        
        for c in s.chars() {
            match c {
                '\n' => {
                    cx = x;
                    cy += font::FONT_HEIGHT as i32;
                }
                '\r' => {
                    cx = x;
                }
                '\t' => {
                    cx += (font::FONT_WIDTH * 4) as i32;
                }
                _ => {
                    if cx + font::FONT_WIDTH as i32 <= self.width as i32 {
                        self.draw_char(cx, cy, c, fg, bg);
                    }
                    cx += font::FONT_WIDTH as i32;
                }
            }
        }
    }
    
    /// Measure string width in pixels
    pub fn measure_string(&self, s: &str) -> u32 {
        let mut width = 0u32;
        let mut max_width = 0u32;
        
        for c in s.chars() {
            match c {
                '\n' => {
                    max_width = max_width.max(width);
                    width = 0;
                }
                '\t' => {
                    width += (font::FONT_WIDTH * 4) as u32;
                }
                _ => {
                    width += font::FONT_WIDTH as u32;
                }
            }
        }
        
        max_width.max(width)
    }
    
    /// Copy a rectangular region (blit)
    pub fn blit(&mut self, src: &Framebuffer, src_rect: Rect, dst_x: i32, dst_y: i32) {
        for sy in 0..src_rect.height as i32 {
            for sx in 0..src_rect.width as i32 {
                if let Some(color) = src.get_pixel(src_rect.x + sx, src_rect.y + sy) {
                    self.set_pixel(dst_x + sx, dst_y + sy, color);
                }
            }
        }
    }
    
    /// Draw a 3D-style border (raised or sunken)
    pub fn draw_3d_rect(&mut self, x: i32, y: i32, w: u32, h: u32, raised: bool) {
        let (tl, br) = if raised {
            (Color::WHITE, Color::DARKGREY)
        } else {
            (Color::DARKGREY, Color::WHITE)
        };
        
        // Top-left edges (light)
        self.draw_hline(x, y, w, tl);
        self.draw_vline(x, y, h, tl);
        
        // Bottom-right edges (dark)
        self.draw_hline(x, y + h as i32 - 1, w, br);
        self.draw_vline(x + w as i32 - 1, y, h, br);
    }

    /// Fast copy entire contents from another framebuffer
    ///
    /// Used for double buffering - copy back buffer to front buffer in one go.
    /// Both framebuffers must have the same dimensions and format.
    pub fn copy_from(&mut self, src: &Framebuffer) {
        // Safety check
        if self.width != src.width || self.height != src.height || self.bpp != src.bpp {
            return;
        }

        // Fast path: if pitch matches, single memcpy
        if self.pitch == src.pitch {
            let total_bytes = (self.pitch * self.height) as usize;
            unsafe {
                core::ptr::copy_nonoverlapping(src.buffer, self.buffer, total_bytes);
            }
        } else {
            // Slow path: copy row by row (handles different padding)
            let row_bytes = (self.width * self.bpp) as usize;
            for y in 0..self.height {
                let src_offset = (y * src.pitch) as usize;
                let dst_offset = (y * self.pitch) as usize;
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        src.buffer.add(src_offset),
                        self.buffer.add(dst_offset),
                        row_bytes,
                    );
                }
            }
        }
    }

    /// Copy a rectangular region from another framebuffer
    /// Useful for partial updates
    pub fn copy_rect_from(&mut self, src: &Framebuffer, rect: Rect) {
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = ((rect.x + rect.width as i32) as u32).min(self.width).min(src.width);
        let y1 = ((rect.y + rect.height as i32) as u32).min(self.height).min(src.height);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let copy_width = ((x1 - x0) * self.bpp) as usize;

        for y in y0..y1 {
            let src_offset = (y * src.pitch + x0 * src.bpp) as usize;
            let dst_offset = (y * self.pitch + x0 * self.bpp) as usize;
            unsafe {
                core::ptr::copy_nonoverlapping(
                    src.buffer.add(src_offset),
                    self.buffer.add(dst_offset),
                    copy_width
                );
            }
        }
    }
}

// Global framebuffer instance
static mut FRAMEBUFFER: Option<Framebuffer> = None;

/// Initialize the global framebuffer
/// 
/// # Safety
/// Must be called only once with valid framebuffer parameters
pub unsafe fn init(buffer: *mut u8, width: u32, height: u32, bpp: u32, pitch: u32) {
    FRAMEBUFFER = Some(Framebuffer::new(buffer, width, height, bpp, pitch));
}

/// Get mutable reference to global framebuffer
pub fn get() -> Option<&'static mut Framebuffer> {
    unsafe { FRAMEBUFFER.as_mut() }
}
