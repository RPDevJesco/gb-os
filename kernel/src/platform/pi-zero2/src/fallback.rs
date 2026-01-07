//! Multicore Debug and Fallback Helpers
//!
//! Add these to main.rs to handle cores failing to start

// ============================================================================
// Global flag to track if multicore is actually working
// ============================================================================

static mut MULTICORE_USB_ACTIVE: bool = false;
static mut MULTICORE_GFX_ACTIVE: bool = false;

// ============================================================================
// Updated boot_main with fallback logic
// ============================================================================

#[no_mangle]
pub extern "C" fn boot_main() -> ! {
    ALLOCATOR.init();
    configure_gpio_for_dpi();

    let fb = match Framebuffer::new() {
        Some(f) => f,
        None => loop { unsafe { core::arch::asm!("wfe"); } }
    };

    fb.clear(DARK_BLUE);
    let mut con = Console::new(&fb, WHITE, DARK_BLUE);

    con.println("=== GB-OS Multi-Core Edition ===");
    con.newline();

    // Initialize USB on Core 0
    con.println("Initializing USB gamepad...");
    let usb = unsafe { &mut USB_HOST };

    match usb.init() {
        Ok(()) => {
            con.set_color(GREEN, DARK_BLUE);
            con.println("  USB controller initialized");

            if usb.wait_for_connection(3000) {
                delay_ms(150);
                if let Ok(()) = usb.reset_port() {
                    if let Ok(()) = usb.enumerate() {
                        unsafe { USB_INITIALIZED = true; }
                        con.set_color(GREEN, DARK_BLUE);
                        con.println("  Gamepad enumerated!");
                    }
                }
            } else {
                con.set_color(YELLOW, DARK_BLUE);
                con.println("  No gamepad detected");
            }
        }
        Err(e) => {
            con.set_color(RED, DARK_BLUE);
            let _ = write!(con, "  USB init failed: {}\n", e);
        }
    }

    // =========================================================================
    // Start secondary cores with debug output
    // =========================================================================
    con.set_color(WHITE, DARK_BLUE);
    con.println("Starting secondary cores...");

    // Initialize graphics info for Core 2
    multicore::init_gfx_core(fb.addr, fb.pitch);

    // Debug: Show spin table addresses before writing
    con.set_color(CYAN, DARK_BLUE);
    unsafe {
        let addr_e0 = core::ptr::read_volatile(0xE0 as *const u64);
        let addr_e8 = core::ptr::read_volatile(0xE8 as *const u64);
        let _ = write!(con, "  Spin[0xE0]={:016X}\n", addr_e0);
        let _ = write!(con, "  Spin[0xE8]={:016X}\n", addr_e8);
    }

    unsafe {
        // Try to start Core 1
        let core1_entry = core1_usb_entry as *const () as u64;
        let _ = write!(con, "  Core1 entry={:016X}\n", core1_entry);

        multicore::start_core(1, core1_usb_entry);

        // Wait up to 1 second for Core 1
        let mut core1_ok = false;
        for i in 0..100 {
            if multicore::CORE1_RUNNING.load(core::sync::atomic::Ordering::Acquire) {
                core1_ok = true;
                break;
            }
            delay_ms(10);
        }

        if core1_ok {
            con.set_color(GREEN, DARK_BLUE);
            con.println("  Core 1 (USB): Running!");
            MULTICORE_USB_ACTIVE = true;
        } else {
            con.set_color(RED, DARK_BLUE);
            con.println("  Core 1 (USB): FAILED - using fallback");
            // Check what's at spin address now
            let addr_e0 = core::ptr::read_volatile(0xE0 as *const u64);
            let _ = write!(con, "  Spin[0xE0] after={:016X}\n", addr_e0);
        }

        // Try to start Core 2
        let core2_entry = core2_gfx_entry as *const () as u64;
        let _ = write!(con, "  Core2 entry={:016X}\n", core2_entry);

        multicore::start_core(2, core2_gfx_entry);

        let mut core2_ok = false;
        for i in 0..100 {
            if multicore::CORE2_RUNNING.load(core::sync::atomic::Ordering::Acquire) {
                core2_ok = true;
                break;
            }
            delay_ms(10);
        }

        if core2_ok {
            con.set_color(GREEN, DARK_BLUE);
            con.println("  Core 2 (GFX): Running!");
            MULTICORE_GFX_ACTIVE = true;
        } else {
            con.set_color(RED, DARK_BLUE);
            con.println("  Core 2 (GFX): FAILED - using fallback");
        }
    }

    // Show final multicore status
    con.newline();
    unsafe {
        if MULTICORE_USB_ACTIVE && MULTICORE_GFX_ACTIVE {
            con.set_color(GREEN, DARK_BLUE);
            con.println("Multicore: FULL (3 cores active)");
        } else if MULTICORE_USB_ACTIVE || MULTICORE_GFX_ACTIVE {
            con.set_color(YELLOW, DARK_BLUE);
            con.println("Multicore: PARTIAL");
        } else {
            con.set_color(RED, DARK_BLUE);
            con.println("Multicore: DISABLED (single core mode)");
        }
    }

    delay_ms(2000); // Let user see the status

    // Mount SD card
    con.newline();
    con.set_color(WHITE, DARK_BLUE);
    con.println("Mounting SD card...");

    let mut fs = Fat32::new();

    match fs.mount() {
        Ok(()) => {
            con.set_color(GREEN, DARK_BLUE);
            con.println("SD card mounted!");
        }
        Err(e) => {
            con.set_color(RED, DARK_BLUE);
            let _ = write!(con, "Mount failed: {}\n", e);
            loop { unsafe { core::arch::asm!("wfe"); } }
        }
    }

    let rom_count = fs.count_roms();
    let _ = write!(con, "Found {} ROM(s)\n", rom_count);

    if rom_count == 0 {
        con.set_color(YELLOW, DARK_BLUE);
        con.println("No .gb or .gbc files found!");
        loop { unsafe { core::arch::asm!("wfe"); } }
    }

    // ROM browser (always uses Core 0 polling)
    if let Some(rom_index) = select_rom(&fb, &mut fs) {
        if let Some(rom_data) = load_rom(&fb, &mut fs, rom_index) {
            fb.clear(BLACK);

            unsafe {
                if MULTICORE_USB_ACTIVE {
                    // Hand off USB to Core 1
                    multicore::set_buttons(BUTTON_STATE.current);
                    multicore::USB_OWNED_BY_CORE1.store(true, core::sync::atomic::Ordering::Release);
                    multicore::dsb();
                    multicore::sev();
                    delay_ms(10);
                }

                // Choose emulator mode based on what's working
                if MULTICORE_USB_ACTIVE && MULTICORE_GFX_ACTIVE {
                    run_emulator_multicore_full(&fb, rom_data);
                } else if MULTICORE_GFX_ACTIVE {
                    run_emulator_multicore_gfx_only(&fb, rom_data);
                } else if MULTICORE_USB_ACTIVE {
                    run_emulator_multicore_usb_only(&fb, rom_data);
                } else {
                    // Fallback to original single-core emulator
                    run_emulator(&fb, rom_data);
                }
            }
        }
    }

    loop { unsafe { core::arch::asm!("wfe"); } }
}

// ============================================================================
// Emulator variants for different multicore configurations
// ============================================================================

/// Full multicore: Core 1 = USB, Core 2 = GFX
fn run_emulator_multicore_full(fb: &Framebuffer, rom_data: Vec<u8>) -> ! {
    let mut device = match Device::new_cgb(rom_data, true) {
        Ok(d) => d,
        Err(_) => {
            fb.clear(RED);
            draw_string(fb, 100, 200, "Emulator init failed!", WHITE, RED);
            loop { unsafe { core::arch::asm!("wfe"); } }
        }
    };

    unsafe { init_mmu(); }
    fb.draw_gb_border(GRAY);

    const CYCLES_PER_FRAME: u32 = 70224;
    const TARGET_FRAME_US: u32 = 16742;
    let mut last_frame_ticks = micros();

    loop {
        // Emulation
        let mut cycles: u32 = 0;
        while cycles < CYCLES_PER_FRAME {
            cycles += device.do_cycle();
        }

        // Graphics - signal Core 2
        if device.check_and_reset_gpu_updated() {
            let is_color = device.mode() == GbMode::Color;
            let screen_ptr = if is_color {
                device.get_gpu_data().as_ptr()
            } else {
                device.get_pal_data().as_ptr()
            };
            multicore::request_blit(screen_ptr, is_color);
        }

        // Input - from Core 1's shared state
        if multicore::button_just_pressed(BTN_RIGHT)  { device.keydown(KeypadKey::Right); }
        if multicore::button_just_released(BTN_RIGHT) { device.keyup(KeypadKey::Right); }
        if multicore::button_just_pressed(BTN_LEFT)   { device.keydown(KeypadKey::Left); }
        if multicore::button_just_released(BTN_LEFT)  { device.keyup(KeypadKey::Left); }
        if multicore::button_just_pressed(BTN_UP)     { device.keydown(KeypadKey::Up); }
        if multicore::button_just_released(BTN_UP)    { device.keyup(KeypadKey::Up); }
        if multicore::button_just_pressed(BTN_DOWN)   { device.keydown(KeypadKey::Down); }
        if multicore::button_just_released(BTN_DOWN)  { device.keyup(KeypadKey::Down); }
        if multicore::button_just_pressed(BTN_A)      { device.keydown(KeypadKey::A); }
        if multicore::button_just_released(BTN_A)     { device.keyup(KeypadKey::A); }
        if multicore::button_just_pressed(BTN_B)      { device.keydown(KeypadKey::B); }
        if multicore::button_just_released(BTN_B)     { device.keyup(KeypadKey::B); }
        if multicore::button_just_pressed(BTN_START)  { device.keydown(KeypadKey::Start); }
        if multicore::button_just_released(BTN_START) { device.keyup(KeypadKey::Start); }
        if multicore::button_just_pressed(BTN_SELECT) { device.keydown(KeypadKey::Select); }
        if multicore::button_just_released(BTN_SELECT){ device.keyup(KeypadKey::Select); }

        // Frame timing
        let target_ticks = last_frame_ticks.wrapping_add(TARGET_FRAME_US);
        while micros().wrapping_sub(target_ticks) > 0x8000_0000 {
            core::hint::spin_loop();
        }
        last_frame_ticks = target_ticks;
    }
}

/// Core 2 only: GFX offloaded, USB on Core 0
fn run_emulator_multicore_gfx_only(fb: &Framebuffer, rom_data: Vec<u8>) -> ! {
    let mut device = match Device::new_cgb(rom_data, true) {
        Ok(d) => d,
        Err(_) => {
            fb.clear(RED);
            loop { unsafe { core::arch::asm!("wfe"); } }
        }
    };

    unsafe { init_mmu(); }
    fb.draw_gb_border(GRAY);

    const CYCLES_PER_FRAME: u32 = 70224;
    const TARGET_FRAME_US: u32 = 16742;
    let mut last_frame_ticks = micros();

    loop {
        let mut cycles: u32 = 0;
        while cycles < CYCLES_PER_FRAME {
            cycles += device.do_cycle();
        }

        // Graphics - signal Core 2
        if device.check_and_reset_gpu_updated() {
            let is_color = device.mode() == GbMode::Color;
            let screen_ptr = if is_color {
                device.get_gpu_data().as_ptr()
            } else {
                device.get_pal_data().as_ptr()
            };
            multicore::request_blit(screen_ptr, is_color);
        }

        // Input - Core 0 polls USB directly
        poll_usb_input();

        unsafe {
            if BUTTON_STATE.just_pressed(BTN_RIGHT)  { device.keydown(KeypadKey::Right); }
            if BUTTON_STATE.just_released(BTN_RIGHT) { device.keyup(KeypadKey::Right); }
            if BUTTON_STATE.just_pressed(BTN_LEFT)   { device.keydown(KeypadKey::Left); }
            if BUTTON_STATE.just_released(BTN_LEFT)  { device.keyup(KeypadKey::Left); }
            if BUTTON_STATE.just_pressed(BTN_UP)     { device.keydown(KeypadKey::Up); }
            if BUTTON_STATE.just_released(BTN_UP)    { device.keyup(KeypadKey::Up); }
            if BUTTON_STATE.just_pressed(BTN_DOWN)   { device.keydown(KeypadKey::Down); }
            if BUTTON_STATE.just_released(BTN_DOWN)  { device.keyup(KeypadKey::Down); }
            if BUTTON_STATE.just_pressed(BTN_A)      { device.keydown(KeypadKey::A); }
            if BUTTON_STATE.just_released(BTN_A)     { device.keyup(KeypadKey::A); }
            if BUTTON_STATE.just_pressed(BTN_B)      { device.keydown(KeypadKey::B); }
            if BUTTON_STATE.just_released(BTN_B)     { device.keyup(KeypadKey::B); }
            if BUTTON_STATE.just_pressed(BTN_START)  { device.keydown(KeypadKey::Start); }
            if BUTTON_STATE.just_released(BTN_START) { device.keyup(KeypadKey::Start); }
            if BUTTON_STATE.just_pressed(BTN_SELECT) { device.keydown(KeypadKey::Select); }
            if BUTTON_STATE.just_released(BTN_SELECT){ device.keyup(KeypadKey::Select); }
        }

        let target_ticks = last_frame_ticks.wrapping_add(TARGET_FRAME_US);
        while micros().wrapping_sub(target_ticks) > 0x8000_0000 {
            core::hint::spin_loop();
        }
        last_frame_ticks = target_ticks;
    }
}

/// Core 1 only: USB offloaded, GFX on Core 0
fn run_emulator_multicore_usb_only(fb: &Framebuffer, rom_data: Vec<u8>) -> ! {
    let mut device = match Device::new_cgb(rom_data, true) {
        Ok(d) => d,
        Err(_) => {
            fb.clear(RED);
            loop { unsafe { core::arch::asm!("wfe"); } }
        }
    };

    unsafe { init_mmu(); }
    fb.draw_gb_border(GRAY);

    const CYCLES_PER_FRAME: u32 = 70224;
    const TARGET_FRAME_US: u32 = 16742;
    let mut last_frame_ticks = micros();

    loop {
        let mut cycles: u32 = 0;
        while cycles < CYCLES_PER_FRAME {
            cycles += device.do_cycle();
        }

        // Graphics - Core 0 does blit directly
        if device.check_and_reset_gpu_updated() {
            if device.mode() == GbMode::Color {
                fb.blit_gb_screen_gbc(device.get_gpu_data());
            } else {
                fb.blit_gb_screen_dmg(device.get_pal_data());
            }
        }

        // Input - from Core 1's shared state
        if multicore::button_just_pressed(BTN_RIGHT)  { device.keydown(KeypadKey::Right); }
        if multicore::button_just_released(BTN_RIGHT) { device.keyup(KeypadKey::Right); }
        if multicore::button_just_pressed(BTN_LEFT)   { device.keydown(KeypadKey::Left); }
        if multicore::button_just_released(BTN_LEFT)  { device.keyup(KeypadKey::Left); }
        if multicore::button_just_pressed(BTN_UP)     { device.keydown(KeypadKey::Up); }
        if multicore::button_just_released(BTN_UP)    { device.keyup(KeypadKey::Up); }
        if multicore::button_just_pressed(BTN_DOWN)   { device.keydown(KeypadKey::Down); }
        if multicore::button_just_released(BTN_DOWN)  { device.keyup(KeypadKey::Down); }
        if multicore::button_just_pressed(BTN_A)      { device.keydown(KeypadKey::A); }
        if multicore::button_just_released(BTN_A)     { device.keyup(KeypadKey::A); }
        if multicore::button_just_pressed(BTN_B)      { device.keydown(KeypadKey::B); }
        if multicore::button_just_released(BTN_B)     { device.keyup(KeypadKey::B); }
        if multicore::button_just_pressed(BTN_START)  { device.keydown(KeypadKey::Start); }
        if multicore::button_just_released(BTN_START) { device.keyup(KeypadKey::Start); }
        if multicore::button_just_pressed(BTN_SELECT) { device.keydown(KeypadKey::Select); }
        if multicore::button_just_released(BTN_SELECT){ device.keyup(KeypadKey::Select); }

        let target_ticks = last_frame_ticks.wrapping_add(TARGET_FRAME_US);
        while micros().wrapping_sub(target_ticks) > 0x8000_0000 {
            core::hint::spin_loop();
        }
        last_frame_ticks = target_ticks;
    }
}
