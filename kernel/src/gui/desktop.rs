//! Desktop / Window Manager
//!
//! Manages windows, mouse cursor, and desktop events.
//!
//! Uses double buffering for windows (no flicker) but draws cursor
//! directly to screen for maximum responsiveness.
//!
//! # EventChain Integration
//!
//! Discrete window lifecycle events (create, destroy, focus, z-order) are
//! dispatched through the Window Manager EventChain. Continuous events
//! (mouse tracking, rendering) remain as direct calls for performance.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use crate::gui::wm_events::{WmEventDispatcher, z_order};
use super::{Window, Framebuffer, Color, Rect, Point, theme, MouseButton};

/// Maximum number of windows
const MAX_WINDOWS: usize = 32;

// =============================================================================
// Terminal Application (Heap Allocated)
// =============================================================================

/// Terminal state - lives on the HEAP via Box
pub struct Terminal {
    /// Output lines
    lines: Vec<String>,
    /// Maximum lines to keep
    max_lines: usize,
    /// Current input buffer
    input: String,
}

impl Terminal {
    /// Create a new terminal
    pub fn new() -> Box<Self> {
        let mut term = Box::new(Self {
            lines: Vec::with_capacity(8),
            max_lines: 8,
            input: String::with_capacity(48),
        });

        // Welcome message
        term.print("Rustacean OS v0.1.0");
        term.print("Type 'help' for commands");
        term.print("");

        term
    }

    /// Print a line to the terminal
    pub fn print(&mut self, text: &str) {
        if self.lines.len() >= self.max_lines {
            self.lines.remove(0);
        }
        self.lines.push(String::from(text));
    }

    /// Handle a character input
    pub fn key_input(&mut self, c: char) {
        if self.input.len() < 40 {
            self.input.push(c);
        }
    }

    /// Handle backspace
    pub fn backspace(&mut self) {
        self.input.pop();
    }

    /// Handle enter - execute command
    pub fn enter(&mut self) {
        // Echo command
        let mut echo = String::from("> ");
        echo.push_str(&self.input);
        self.print(&echo);

        // Execute
        let cmd: String = self.input.trim().chars().collect();
        self.execute(&cmd);

        // Clear input
        self.input.clear();
    }

    /// Execute a command
    fn execute(&mut self, cmd: &str) {
        match cmd {
            "help" => {
                self.print("Commands: help ls clear info heap");
            }
            "ls" => {
                self.print("Documents/ Projects/ Downloads/");
                self.print("notes.txt main.rs Cargo.toml");
            }
            "clear" => {
                self.lines.clear();
            }
            "info" => {
                self.print("CPU: Pentium III 450MHz");
                self.print("RAM: 256 MB");
                self.print("GPU: ATI Rage Mobility P");
            }
            "heap" => {
                let stats = crate::mm::heap::stats();
                let mut buf = String::new();
                let _ = write!(buf, "Used: {} bytes", stats.used);
                self.print(&buf);
                buf.clear();
                let _ = write!(buf, "Free: {} bytes", stats.free);
                self.print(&buf);
            }
            "" => {}
            _ => {
                self.print("Unknown cmd. Try 'help'");
            }
        }
    }

    /// Get lines for rendering
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Get current input
    pub fn input(&self) -> &str {
        &self.input
    }
}

// =============================================================================
// Mouse Cursor
// =============================================================================

/// Mouse cursor bitmap (16x16)
const CURSOR_WIDTH: u32 = 16;
const CURSOR_HEIGHT: u32 = 16;

static CURSOR_BITMAP: [u16; 16] = [
    0b1000000000000000,
    0b1100000000000000,
    0b1110000000000000,
    0b1111000000000000,
    0b1111100000000000,
    0b1111110000000000,
    0b1111111000000000,
    0b1111111100000000,
    0b1111111110000000,
    0b1111111111000000,
    0b1111110000000000,
    0b1101111000000000,
    0b1000111100000000,
    0b0000011110000000,
    0b0000011110000000,
    0b0000001100000000,
];

static CURSOR_MASK: [u16; 16] = [
    0b1100000000000000,
    0b1110000000000000,
    0b1111000000000000,
    0b1111100000000000,
    0b1111110000000000,
    0b1111111000000000,
    0b1111111100000000,
    0b1111111110000000,
    0b1111111111000000,
    0b1111111111100000,
    0b1111111111000000,
    0b1111111110000000,
    0b1101111111000000,
    0b1000111111000000,
    0b0000111111000000,
    0b0000011110000000,
];

/// Desktop state
pub struct Desktop {
    /// All windows
    windows: [Option<Window>; MAX_WINDOWS],
    /// Window Z-order (indices into windows array, front to back)
    z_order: [usize; MAX_WINDOWS],
    /// Number of windows in z_order
    window_count: usize,
    /// Currently focused window index
    focused: Option<usize>,
    /// Mouse position
    mouse_x: i32,
    mouse_y: i32,
    /// Mouse buttons state
    mouse_buttons: u8,
    /// Window being dragged
    dragging: Option<usize>,
    /// Drag offset from window corner
    drag_offset: Point,
    /// Drag start position (for EventChain completion event)
    drag_start_x: i32,
    drag_start_y: i32,
    /// Screen dimensions
    screen_width: u32,
    screen_height: u32,
    /// Next window ID
    next_id: u32,
    /// Desktop needs full redraw (windows changed)
    dirty: bool,
    /// Using hardware cursor (skip software cursor drawing)
    hw_cursor: bool,
    /// Saved pixels under cursor (from front buffer)
    cursor_save: [Color; 256], // 16x16
    cursor_save_x: i32,
    cursor_save_y: i32,
    /// Terminal application (heap allocated)
    terminal: Option<Box<Terminal>>,
    /// Terminal window ID
    term_window_id: Option<u32>,
}

impl Desktop {
    /// Create a new desktop
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        const NONE_WINDOW: Option<Window> = None;

        Self {
            windows: [NONE_WINDOW; MAX_WINDOWS],
            z_order: [0; MAX_WINDOWS],
            window_count: 0,
            focused: None,
            mouse_x: (screen_width / 2) as i32,
            mouse_y: (screen_height / 2) as i32,
            mouse_buttons: 0,
            dragging: None,
            drag_offset: Point::new(0, 0),
            drag_start_x: 0,
            drag_start_y: 0,
            screen_width,
            screen_height,
            next_id: 1,
            dirty: true,
            hw_cursor: false,
            cursor_save: [Color::BLACK; 256],
            cursor_save_x: -1,
            cursor_save_y: -1,
            terminal: None,
            term_window_id: None,
        }
    }

    /// Enable or disable hardware cursor mode
    ///
    /// When hw_cursor is true, software cursor drawing is skipped
    /// (assumes hardware cursor is being used instead)
    pub fn set_hw_cursor(&mut self, enabled: bool) {
        self.hw_cursor = enabled;
    }

    /// Find window at screen coordinates (front to back)
    pub fn window_at(&self, x: i32, y: i32) -> Option<usize> {
        for i in 0..self.window_count {
            let slot = self.z_order[i];
            if let Some(ref window) = self.windows[slot] {
                if window.flags.visible && window.contains(x, y) {
                    return Some(slot);
                }
            }
        }
        None
    }

    /// Handle mouse movement (direct - hot path)
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) {
        self.mouse_x = x.max(0).min(self.screen_width as i32 - 1);
        self.mouse_y = y.max(0).min(self.screen_height as i32 - 1);

        // Handle window dragging only
        if let Some(slot) = self.dragging {
            if let Some(ref mut window) = self.windows[slot] {
                let new_x = self.mouse_x - self.drag_offset.x;
                let new_y = self.mouse_y - self.drag_offset.y;
                window.move_to(new_x, new_y);
                self.dirty = true;
            }
        }
        // Note: Sketch drawing only happens on click, not drag
        // This keeps the mouse driver interaction simple and safe
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, _key: char, _pressed: bool) {
        // Forward to focused window
        if let Some(_slot) = self.focused {
            // In a full implementation, dispatch to window's event handler
        }
    }

    /// Save pixels under cursor from front buffer
    fn save_cursor_area(&mut self, fb: &Framebuffer) {
        let x = self.mouse_x;
        let y = self.mouse_y;

        for cy in 0..CURSOR_HEIGHT as i32 {
            for cx in 0..CURSOR_WIDTH as i32 {
                let idx = (cy * CURSOR_WIDTH as i32 + cx) as usize;
                if idx < 256 {
                    self.cursor_save[idx] = fb.get_pixel(x + cx, y + cy)
                        .unwrap_or(Color::BLACK);
                }
            }
        }

        self.cursor_save_x = x;
        self.cursor_save_y = y;
    }

    /// Restore pixels under cursor to front buffer
    fn restore_cursor_area(&self, fb: &mut Framebuffer) {
        if self.cursor_save_x < 0 {
            return;
        }

        let x = self.cursor_save_x;
        let y = self.cursor_save_y;

        for cy in 0..CURSOR_HEIGHT as i32 {
            for cx in 0..CURSOR_WIDTH as i32 {
                let idx = (cy * CURSOR_WIDTH as i32 + cx) as usize;
                if idx < 256 {
                    fb.set_pixel(x + cx, y + cy, self.cursor_save[idx]);
                }
            }
        }
    }

    /// Draw cursor at current position to front buffer
    fn draw_cursor(&mut self, fb: &mut Framebuffer) {
        self.save_cursor_area(fb);

        let x = self.mouse_x;
        let y = self.mouse_y;

        for cy in 0..16i32 {
            let bitmap_row = CURSOR_BITMAP[cy as usize];
            let mask_row = CURSOR_MASK[cy as usize];

            for cx in 0..16i32 {
                let bit = 15 - cx;
                let mask_bit = (mask_row >> bit) & 1;
                let color_bit = (bitmap_row >> bit) & 1;

                if mask_bit != 0 {
                    let color = if color_bit != 0 {
                        Color::WHITE
                    } else {
                        Color::BLACK
                    };
                    fb.set_pixel(x + cx, y + cy, color);
                }
            }
        }
    }

    /// Render windows to back buffer (no cursor)
    fn render_to_back_buffer(&self, back_buffer: &mut Framebuffer) {
        let theme = theme::current();

        // Clear background
        back_buffer.clear(theme.desktop_bg);

        // Draw windows back to front
        for i in (0..self.window_count).rev() {
            let slot = self.z_order[i];
            if let Some(ref window) = self.windows[slot] {
                if window.flags.visible {
                    window.draw(back_buffer);

                    // Draw window content based on title
                    self.draw_window_content(back_buffer, window);
                }
            }
        }
    }

    /// Draw content for a window based on its type
    fn draw_window_content(&self, fb: &mut Framebuffer, window: &Window) {
        let title = window.title();

        if title.contains("Welcome") {
            self.draw_welcome_content(fb, window);
        } else if title.contains("Terminal") {
            self.draw_terminal_content(fb, window);
        } else if title.contains("Files") {
            self.draw_files_content(fb, window);
        }
    }

    /// Draw Welcome window content
    fn draw_welcome_content(&self, fb: &mut Framebuffer, window: &Window) {
        let theme = theme::current();
        window.draw_text(fb, 8, 8, "Welcome to Rustacean OS!", theme.text);
        window.draw_text(fb, 8, 28, "A Plan 9 inspired OS written in Rust", theme.text);
        window.draw_text(fb, 8, 56, "Drag windows by title bar!", theme.text);
        window.draw_text(fb, 8, 76, "Click windows to focus.", theme.text);
    }

    /// Draw Terminal window content
    fn draw_terminal_content(&self, fb: &mut Framebuffer, window: &Window) {
        // Dark terminal background
        let content = window.content_rect_abs();
        let bg = Color::rgb(20, 20, 30);
        fb.fill_rect(content.x, content.y, content.width, content.height, bg);

        let green = Color::rgb(0, 255, 100);
        let prompt_color = Color::rgb(100, 200, 255);

        // Render from heap-allocated terminal state
        if let Some(ref term) = self.terminal {
            for (i, line) in term.lines().iter().enumerate() {
                window.draw_text_color(fb, 8, 8 + (i as i32 * 16), line, green, bg);
            }

            let input_y = 8 + (term.lines().len() as i32 * 16);
            window.draw_text_color(fb, 8, input_y, "> ", prompt_color, bg);
            window.draw_text_color(fb, 24, input_y, term.input(), green, bg);

            // Blinking cursor
            let cursor_x = 24 + (term.input().len() as i32 * 8);
            window.draw_text_color(fb, cursor_x, input_y, "_", green, bg);
        } else {
            // Fallback if terminal not created
            window.draw_text_color(fb, 8, 8, "Terminal not initialized", green, bg);
        }
    }

    /// Draw Files window content
    fn draw_files_content(&self, fb: &mut Framebuffer, window: &Window) {
        let theme = theme::current();
        let folder = Color::rgb(255, 200, 100);
        let file = theme.text;

        window.draw_text(fb, 8, 8, "/home/user", theme.text);
        window.draw_text(fb, 8, 28, "----------------", theme.text);
        window.draw_text_color(fb, 8, 48, "[dir]  Documents", folder, theme.window_bg);
        window.draw_text_color(fb, 8, 68, "[dir]  Projects", folder, theme.window_bg);
        window.draw_text_color(fb, 8, 88, "[dir]  Downloads", folder, theme.window_bg);
        window.draw_text(fb, 8, 108, "[txt]  notes.txt", file);
        window.draw_text(fb, 8, 128, "[rs]   main.rs", file);
        window.draw_text(fb, 8, 148, "[toml] Cargo.toml", file);
    }

    // =========================================================================
    // Terminal Application Methods
    // =========================================================================

    /// Create terminal window with heap-allocated state
    pub fn create_terminal_window(&mut self, x: i32, y: i32, w: u32, h: u32) -> Option<u32> {
        let id = self.create_window("Terminal", x, y, w, h)?;
        self.term_window_id = Some(id);
        self.terminal = Some(Terminal::new());
        Some(id)
    }

    /// Check if terminal is focused
    pub fn is_terminal_focused(&self) -> bool {
        if let (Some(term_id), Some(focus_slot)) = (self.term_window_id, self.focused) {
            if let Some(ref window) = self.windows[focus_slot] {
                return window.id == term_id;
            }
        }
        false
    }

    /// Terminal key input
    pub fn term_key_input(&mut self, c: char) {
        if let Some(ref mut term) = self.terminal {
            term.key_input(c);
            self.dirty = true;
        }
    }

    /// Terminal backspace
    pub fn term_backspace(&mut self) {
        if let Some(ref mut term) = self.terminal {
            term.backspace();
            self.dirty = true;
        }
    }

    /// Terminal enter
    pub fn term_enter(&mut self) {
        if let Some(ref mut term) = self.terminal {
            term.enter();
            self.dirty = true;
        }
    }

    /// Draw with double buffering for windows, direct draw for cursor
    pub fn draw(&mut self, back_buffer: &mut Framebuffer, front_buffer: &mut Framebuffer) {
        // Step 1: Restore old cursor area on front buffer (software cursor only)
        if !self.hw_cursor {
            self.restore_cursor_area(front_buffer);
        }

        // Step 2: If windows changed, re-render to back buffer and copy
        if self.dirty {
            self.render_to_back_buffer(back_buffer);
            front_buffer.copy_from(back_buffer);
            self.dirty = false;
        }

        // Step 3: Draw cursor directly to front buffer (software cursor only)
        if !self.hw_cursor {
            self.draw_cursor(front_buffer);
        }
    }

    /// Mark desktop as dirty (windows need redraw)
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Get mouse position
    pub fn mouse_pos(&self) -> (i32, i32) {
        (self.mouse_x, self.mouse_y)
    }

    /// Get focused window ID
    pub fn focused_window(&self) -> Option<u32> {
        self.focused.and_then(|slot| {
            self.windows[slot].as_ref().map(|w| w.id)
        })
    }

    /// Get a window by ID for content drawing
    pub fn get_window(&mut self, id: u32) -> Option<&mut Window> {
        for window in self.windows.iter_mut() {
            if let Some(ref mut w) = window {
                if w.id == id {
                    return Some(w);
                }
            }
        }
        None
    }

    /// Get screen dimensions
    pub fn screen_size(&self) -> (u32, u32) {
        (self.screen_width, self.screen_height)
    }

    // =========================================================================
    // Window Creation (via EventChain)
    // =========================================================================

    /// Create a new window
    ///
    /// Dispatches through WM EventChain for validation and audit.
    pub fn create_window(&mut self, title: &str, x: i32, y: i32, width: u32, height: u32) -> Option<u32> {
        // Dispatch through EventChain for validation
        if !WmEventDispatcher::dispatch_create(x, y, width, height) {
            return None;
        }

        // Find free slot
        let slot = self.windows.iter().position(|w| w.is_none())?;

        // Generate window ID
        let id = self.next_id;
        self.next_id += 1;

        // Create the window
        let window = Window::new(id, title, x, y, width, height);
        self.windows[slot] = Some(window);

        // Add to z-order
        if self.window_count < MAX_WINDOWS {
            self.z_order[self.window_count] = slot;
            self.window_count += 1;
        }

        // Dispatch focus change through EventChain
        let old_focus = self.focused.and_then(|s| {
            self.windows[s].as_ref().map(|w| w.id)
        });
        WmEventDispatcher::dispatch_focus_change(old_focus, Some(id));

        // Set as focused (top of z-order)
        self.focus_window(slot);

        self.dirty = true;
        Some(id)
    }

    /// Destroy a window by slot index
    ///
    /// Dispatches through WM EventChain for cleanup and audit.
    pub fn destroy_window(&mut self, slot: usize) -> bool {
        if slot >= MAX_WINDOWS {
            return false;
        }

        let window_id = match &self.windows[slot] {
            Some(w) => w.id,
            None => return false,
        };

        // Dispatch through EventChain
        if !WmEventDispatcher::dispatch_destroy(window_id) {
            return false;
        }

        // If this was focused, clear focus
        if self.focused == Some(slot) {
            WmEventDispatcher::dispatch_focus_change(Some(window_id), None);
            self.focused = None;
        }

        // Remove from z-order
        if let Some(pos) = self.z_order[..self.window_count].iter().position(|&s| s == slot) {
            for i in pos..self.window_count - 1 {
                self.z_order[i] = self.z_order[i + 1];
            }
            self.window_count = self.window_count.saturating_sub(1);
        }

        self.windows[slot] = None;
        self.dirty = true;
        true
    }

    // =========================================================================
    // Focus Management (via EventChain)
    // =========================================================================

    /// Focus a window by slot index
    fn focus_window(&mut self, slot: usize) {
        if slot >= MAX_WINDOWS || self.windows[slot].is_none() {
            return;
        }

        let new_id = self.windows[slot].as_ref().unwrap().id;
        let old_id = self.focused.and_then(|s| {
            self.windows[s].as_ref().map(|w| w.id)
        });

        // Dispatch through EventChain (could be blocked by policy)
        if !WmEventDispatcher::dispatch_focus_change(old_id, Some(new_id)) {
            return;
        }

        // Unfocus old window
        if let Some(old_slot) = self.focused {
            if let Some(ref mut old_win) = self.windows[old_slot] {
                old_win.flags.focused = false;
            }
        }

        // Focus new window
        if let Some(ref mut win) = self.windows[slot] {
            win.flags.focused = true;
        }
        self.focused = Some(slot);

        // Bring to front
        self.bring_to_front(slot);

        self.dirty = true;
    }

    // =========================================================================
    // Z-Order Management (via EventChain)
    // =========================================================================

    /// Bring a window to the front of the z-order
    fn bring_to_front(&mut self, slot: usize) {
        let window_id = match &self.windows[slot] {
            Some(w) => w.id,
            None => return,
        };

        // Dispatch through EventChain
        if !WmEventDispatcher::dispatch_z_order_change(window_id, z_order::BRING_TO_FRONT) {
            return;
        }

        // Find current position
        let current_pos = match self.z_order[..self.window_count].iter().position(|&s| s == slot) {
            Some(p) => p,
            None => return,
        };

        // Already at front (index 0)?
        if current_pos == 0 {
            return;
        }

        // Shift others up to make room at front
        for i in (1..=current_pos).rev() {
            self.z_order[i] = self.z_order[i - 1];
        }

        // Put at front (index 0 = topmost, drawn last)
        self.z_order[0] = slot;
        self.dirty = true;
    }

    // =========================================================================
    // Window Move Completion (via EventChain)
    // =========================================================================

    /// Called when a drag operation completes
    fn complete_drag(&mut self, slot: usize, old_x: i32, old_y: i32, new_x: i32, new_y: i32) {
        let window_id = match &self.windows[slot] {
            Some(w) => w.id,
            None => return,
        };

        // Dispatch move event for audit
        WmEventDispatcher::dispatch_move(window_id, old_x, old_y, new_x, new_y);
    }

    // =========================================================================
    // Mouse Button Handler
    // =========================================================================

    /// Handle mouse button press/release
    pub fn handle_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        let bit = match button {
            MouseButton::Left => 0x01,
            MouseButton::Right => 0x02,
            MouseButton::Middle => 0x04,
        };

        if pressed {
            self.mouse_buttons |= bit;

            if button == MouseButton::Left {
                // Check for window click (front to back in z-order)
                // First, find the clicked window and gather needed data
                let mut click_info: Option<(usize, bool, i32, i32)> = None;

                for i in 0..self.window_count {
                    let slot = self.z_order[i];
                    if let Some(ref window) = self.windows[slot] {
                        if window.contains(self.mouse_x, self.mouse_y) {
                            let in_title = window.in_title_bar(self.mouse_x, self.mouse_y);
                            click_info = Some((slot, in_title, window.bounds.x, window.bounds.y));
                            break;
                        }
                    }
                }

                // Now handle the click with no outstanding borrows
                if let Some((slot, in_title, win_x, win_y)) = click_info {
                    // Focus this window (through EventChain)
                    if self.focused != Some(slot) {
                        self.focus_window(slot);
                    }

                    // Check if in title bar for drag
                    if in_title {
                        self.dragging = Some(slot);
                        self.drag_start_x = win_x;
                        self.drag_start_y = win_y;
                        self.drag_offset = Point::new(
                            self.mouse_x - win_x,
                            self.mouse_y - win_y,
                        );
                    }
                }
            }
        } else {
            self.mouse_buttons &= !bit;

            // Stop dragging and dispatch completion event
            if button == MouseButton::Left {
                if let Some(slot) = self.dragging {
                    // Extract position before mutable borrow
                    let new_pos = self.windows[slot]
                        .as_ref()
                        .map(|w| (w.bounds.x, w.bounds.y));

                    if let Some((new_x, new_y)) = new_pos {
                        self.complete_drag(
                            slot,
                            self.drag_start_x,
                            self.drag_start_y,
                            new_x,
                            new_y,
                        );
                    }
                }
                self.dragging = None;
            }
        }
    }
}

// =============================================================================
// Global Instance
// =============================================================================

static mut DESKTOP: Option<Desktop> = None;

/// Initialize the global desktop
pub fn init(width: u32, height: u32) {
    unsafe {
        DESKTOP = Some(Desktop::new(width, height));
    }
}

/// Initialize the global desktop with hardware cursor support
pub fn init_with_hw_cursor(width: u32, height: u32, hw_cursor: bool) {
    unsafe {
        let mut desktop = Desktop::new(width, height);
        desktop.hw_cursor = hw_cursor;
        DESKTOP = Some(desktop);
    }
}

/// Get the global desktop
pub fn get() -> Option<&'static mut Desktop> {
    unsafe { DESKTOP.as_mut() }
}
