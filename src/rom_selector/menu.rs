use super::{FileSystem, Display, Input, RomEntry, Selection, ButtonEvent, PAGE_SIZE};

// ============================================================================
// Colors (matching framebuffer::color)
// ============================================================================

mod colors {
    pub const BLACK: u32 = 0xFF00_0000;
    pub const WHITE: u32 = 0xFFFF_FFFF;
    pub const MENU_BG: u32 = 0xFF10_1020;
    pub const MENU_HIGHLIGHT: u32 = 0xFF30_3060;
    pub const MENU_TEXT: u32 = 0xFFE0_E0E0;
    pub const MENU_TEXT_DIM: u32 = 0xFF80_8080;
    pub const MENU_ACCENT: u32 = 0xFF40_80FF;
    pub const RED: u32 = 0xFFFF_0000;
}

// ============================================================================
// Layout Constants
// ============================================================================

const CHAR_WIDTH: u32 = 8;
const CHAR_HEIGHT: u32 = 8;
const LINE_HEIGHT: u32 = 12;
const MARGIN_X: u32 = 16;
const MARGIN_Y: u32 = 16;
const TITLE_Y: u32 = 16;
const LIST_START_Y: u32 = 48;

// ============================================================================
// Menu State
// ============================================================================

/// Menu state - lives on stack
pub struct MenuState {
    /// Currently selected index (global)
    pub selected: usize,
    /// Total ROM count
    pub total: usize,
    /// Current page start index
    pub page_start: usize,
    /// Entries for current page (stack allocated)
    pub entries: [RomEntry; PAGE_SIZE],
    /// How many entries are valid in current page
    pub page_count: usize,
}

impl MenuState {
    pub const fn new() -> Self {
        Self {
            selected: 0,
            total: 0,
            page_start: 0,
            entries: [RomEntry::empty(); PAGE_SIZE],
            page_count: 0,
        }
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

/// Run the ROM selector
///
/// Returns Some(Selection) when user picks a ROM, None if cancelled or no ROMs
pub fn run_selector<F: FileSystem, D: Display, I: Input>(
    fs: &mut F,
    display: &mut D,
    input: &mut I,
) -> Option<Selection> {
    let mut state = MenuState::new();

    // Count ROMs and load first page
    state.total = count_roms(fs);
    if state.total == 0 {
        draw_no_roms(display);
        display.present();

        // Wait for any button press
        loop {
            match input.poll() {
                ButtonEvent::None => {}
                _ => return None,
            }
        }
    }

    load_page(fs, &mut state, 0);

    let mut needs_redraw = true;  // Draw on first iteration

    loop {
        // Only render when something changed
        if needs_redraw {
            draw_menu(display, &state);
            display.present();
            display.present();
            needs_redraw = false;
        }

        // Handle input
        match input.poll() {
            ButtonEvent::Up => {
                if state.selected > 0 {
                    state.selected -= 1;
                    let selected = state.selected;
                    if selected < state.page_start {
                        load_page(fs, &mut state, selected);
                    }
                    needs_redraw = true;
                }
            }
            ButtonEvent::Down => {
                if state.selected + 1 < state.total {
                    state.selected += 1;
                    let selected = state.selected;
                    if selected >= state.page_start + state.page_count {
                        load_page(fs, &mut state, selected);
                    }
                    needs_redraw = true;
                }
            }
            ButtonEvent::Left => {
                if state.selected >= PAGE_SIZE {
                    state.selected -= PAGE_SIZE;
                } else {
                    state.selected = 0;
                }
                let selected = state.selected;
                if selected < state.page_start {
                    load_page(fs, &mut state, selected);
                }
                needs_redraw = true;
            }
            ButtonEvent::Right => {
                state.selected += PAGE_SIZE;
                if state.selected >= state.total {
                    state.selected = state.total - 1;
                }
                let selected = state.selected;
                if selected >= state.page_start + state.page_count {
                    load_page(fs, &mut state, selected);
                }
                needs_redraw = true;
            }
            ButtonEvent::Select => {
                let idx = state.selected - state.page_start;
                let entry = &state.entries[idx];
                return Some(Selection {
                    cluster: entry.cluster,
                    size: entry.size,
                });
            }
            ButtonEvent::Back => {
                return None;
            }
            ButtonEvent::None => {
                // No input - don't redraw, but wait for vblank to throttle loop
                display.wait_vblank();
            }
        }
    }
}

// ============================================================================
// ROM Enumeration
// ============================================================================

fn count_roms<F: FileSystem>(fs: &mut F) -> usize {
    fs.reset_enumeration();
    let mut count = 0;
    let mut dummy = RomEntry::empty();
    while fs.next_rom(&mut dummy) {
        count += 1;
    }
    count
}

fn load_page<F: FileSystem>(fs: &mut F, state: &mut MenuState, around_index: usize) {
    // Calculate page start (align to page boundary)
    state.page_start = (around_index / PAGE_SIZE) * PAGE_SIZE;

    // Seek to page start
    fs.reset_enumeration();
    let mut dummy = RomEntry::empty();
    for _ in 0..state.page_start {
        if !fs.next_rom(&mut dummy) {
            break;
        }
    }

    // Load page entries
    state.page_count = 0;
    for i in 0..PAGE_SIZE {
        if fs.next_rom(&mut state.entries[i]) {
            state.page_count += 1;
        } else {
            break;
        }
    }
}

// ============================================================================
// Drawing
// ============================================================================

fn draw_menu<D: Display>(display: &mut D, state: &MenuState) {
    let width = display.width();

    // Clear background
    display.clear(colors::MENU_BG);

    // Draw title
    draw_text_centered(display, TITLE_Y, b"GB-OS ROM Selector", colors::MENU_ACCENT, colors::MENU_BG);

    // Draw ROM count
    let mut count_buf = [0u8; 32];
    let count_len = format_count(&mut count_buf, state.selected + 1, state.total);
    draw_text_centered(display, TITLE_Y + LINE_HEIGHT + 4, &count_buf[..count_len], colors::MENU_TEXT_DIM, colors::MENU_BG);

    // Draw ROM list
    for i in 0..state.page_count {
        let global_idx = state.page_start + i;
        let y = LIST_START_Y + (i as u32) * LINE_HEIGHT;
        let is_selected = global_idx == state.selected;

        // Highlight bar for selected item
        if is_selected {
            display.fill_rect(MARGIN_X - 4, y - 2, width - MARGIN_X * 2 + 8, LINE_HEIGHT, colors::MENU_HIGHLIGHT);
        }

        // Selection indicator
        let indicator = if is_selected { b"> " } else { b"  " };
        let text_color = if is_selected { colors::WHITE } else { colors::MENU_TEXT };
        let bg_color = if is_selected { colors::MENU_HIGHLIGHT } else { colors::MENU_BG };

        display.draw_text(MARGIN_X, y, indicator, colors::MENU_ACCENT, bg_color);

        // ROM name (truncated if needed)
        let entry = &state.entries[i];
        let name = &entry.name[..entry.name_len.min(64)];
        display.draw_text(MARGIN_X + CHAR_WIDTH * 2, y, name, text_color, bg_color);

        // GBC indicator
        if entry.is_gbc {
            let gbc_x = width - MARGIN_X - CHAR_WIDTH * 5;
            display.draw_text(gbc_x, y, b"[GBC]", colors::MENU_ACCENT, bg_color);
        }
    }

    // Draw scrollbar if needed
    if state.total > PAGE_SIZE {
        draw_scrollbar(display, state);
    }

    // Draw help text at bottom
    let help_y = display.height() - MARGIN_Y - CHAR_HEIGHT;
    draw_text_centered(display, help_y, b"A:Select  B:Back  L/R:Page", colors::MENU_TEXT_DIM, colors::MENU_BG);
}

fn draw_no_roms<D: Display>(display: &mut D) {
    display.clear(colors::MENU_BG);

    let center_y = display.height() / 2;

    draw_text_centered(display, center_y - LINE_HEIGHT, b"No ROM files found!", colors::RED, colors::MENU_BG);
    draw_text_centered(display, center_y + LINE_HEIGHT, b"Place .gb or .gbc files", colors::MENU_TEXT_DIM, colors::MENU_BG);
    draw_text_centered(display, center_y + LINE_HEIGHT * 2, b"on your SD card.", colors::MENU_TEXT_DIM, colors::MENU_BG);
}

fn draw_scrollbar<D: Display>(display: &mut D, state: &MenuState) {
    let x = display.width() - MARGIN_X / 2 - 2;
    let track_y = LIST_START_Y;
    let track_height = (PAGE_SIZE as u32) * LINE_HEIGHT;

    // Track
    display.fill_rect(x, track_y, 4, track_height, colors::MENU_TEXT_DIM);

    // Thumb
    let thumb_height = ((PAGE_SIZE as u32) * track_height / state.total as u32).max(8);
    let thumb_y = track_y + (state.selected as u32 * (track_height - thumb_height) / (state.total as u32 - 1).max(1));
    display.fill_rect(x, thumb_y, 4, thumb_height, colors::MENU_ACCENT);
}

fn draw_text_centered<D: Display>(display: &mut D, y: u32, text: &[u8], fg: u32, bg: u32) {
    let text_width = text.len() as u32 * CHAR_WIDTH;
    let x = (display.width().saturating_sub(text_width)) / 2;
    display.draw_text(x, y, text, fg, bg);
}

/// Format "N / M" into buffer, return length
fn format_count(buf: &mut [u8], current: usize, total: usize) -> usize {
    let mut pos = 0;

    // Write current number
    pos += write_number(&mut buf[pos..], current);

    // Write " / "
    buf[pos..pos + 3].copy_from_slice(b" / ");
    pos += 3;

    // Write total
    pos += write_number(&mut buf[pos..], total);

    pos
}

/// Write a number to buffer, return bytes written
fn write_number(buf: &mut [u8], mut n: usize) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }

    // Count digits
    let mut temp = n;
    let mut digits = 0;
    while temp > 0 {
        digits += 1;
        temp /= 10;
    }

    // Write digits in reverse
    let mut pos = digits;
    while n > 0 {
        pos -= 1;
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    digits
}
