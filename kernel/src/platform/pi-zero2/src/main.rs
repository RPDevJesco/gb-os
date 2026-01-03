//! Raspberry Pi Zero 2 W Bootloader
//!
//! Entry point for the Pi Zero 2 W (BCM2710, Cortex-A53).
//! The GPU firmware loads this as kernel8.img at 0x80000.

#![no_std]
#![no_main]

mod gpio;
mod uart;
mod mmio;
mod memory_map;
mod entry;
mod mailbox;

use bootcore::Serial;
use core::fmt::Write;

/// Pi Zero 2 W peripheral base address.
pub const PERIPHERAL_BASE: usize = memory_map::PERIPHERAL_BASE;

/// Main boot entry point, called from assembly after stack setup.
#[unsafe(no_mangle)]
pub extern "C" fn boot_main(_core_id: u64) -> ! {
    // Initialize UART for debug output
    let mut uart = uart::MiniUart::new();
    uart.init(115200).expect("UART init failed");

    // Print banner
    uart.write_line("");
    uart.write_line("=====================================");
    uart.write_line(" rustboot - Pi Zero 2 W");
    uart.write_line("=====================================");
    uart.write_line("");

    // Print CPU info
    let el = arch_aarch64::current_el();
    let core = arch_aarch64::core_id();
    let midr = arch_aarch64::cpu::Midr::read();

    let _ = writeln!(
        bootcore::fmt::SerialWriter(&mut uart),
        "Exception Level: EL{}",
        el
    );
    let _ = writeln!(
        bootcore::fmt::SerialWriter(&mut uart),
        "Core ID: {}",
        core
    );

    if midr.is_cortex_a53() {
        uart.write_line("CPU: ARM Cortex-A53");
    }

    uart.write_line("");

    // ========================================================================
    // Query memory via mailbox
    // ========================================================================
    uart.write_line("Querying VideoCore mailbox...");

    let mbox = mailbox::get_mailbox();

    // Get ARM memory
    match mbox.get_arm_memory() {
        Ok((base, size)) => {
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  ARM Memory: 0x{:08X} - 0x{:08X} ({} MB)",
                base,
                base + size,
                size / 1024 / 1024
            );
        }
        Err(e) => {
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  ARM Memory: ERROR {:?}",
                e
            );
        }
    }

    // Get VC memory
    match mbox.get_vc_memory() {
        Ok((base, size)) => {
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  VC Memory:  0x{:08X} - 0x{:08X} ({} MB)",
                base,
                base + size,
                size / 1024 / 1024
            );
        }
        Err(e) => {
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  VC Memory: ERROR {:?}",
                e
            );
        }
    }

    // Get board revision
    match mbox.get_board_revision() {
        Ok(rev) => {
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  Board Rev:  0x{:08X}",
                rev
            );
        }
        Err(_) => {}
    }

    // Get clock rates
    uart.write_line("");
    uart.write_line("Clock rates:");

    for (name, id) in &[
        ("ARM", mailbox::clock::ARM),
        ("Core", mailbox::clock::CORE),
        ("EMMC", mailbox::clock::EMMC),
        ("UART", mailbox::clock::UART),
    ] {
        match mbox.get_clock_rate(*id) {
            Ok(rate) => {
                let _ = writeln!(
                    bootcore::fmt::SerialWriter(&mut uart),
                    "  {:6}: {} MHz",
                    name,
                    rate / 1_000_000
                );
            }
            Err(_) => {}
        }
    }

    // ========================================================================
    // Initialize framebuffer (optional - uncomment to test)
    // ========================================================================
    uart.write_line("");
    uart.write_line("Initializing framebuffer...");

    match mbox.init_framebuffer(640, 480, 32) {
        Ok(fb) => {
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  Resolution: {}x{}",
                fb.width,
                fb.height
            );
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  Depth:      {} bpp",
                fb.depth
            );
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  Pitch:      {} bytes",
                fb.pitch
            );
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  Address:    0x{:08X}",
                fb.address
            );
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  Size:       {} KB",
                fb.size / 1024
            );

            // Draw something to test
            uart.write_line("  Drawing test pattern...");
            draw_test_pattern(&fb);
            uart.write_line("  Done!");
        }
        Err(e) => {
            let _ = writeln!(
                bootcore::fmt::SerialWriter(&mut uart),
                "  Framebuffer ERROR: {:?}",
                e
            );
        }
    }

    // ========================================================================
    // Print memory layout
    // ========================================================================
    uart.write_line("");
    uart.write_line("Memory layout:");
    let _ = writeln!(
        bootcore::fmt::SerialWriter(&mut uart),
        "  Kernel: 0x{:08X} - 0x{:08X} ({} MB)",
        memory_map::KERNEL_BASE,
        memory_map::KERNEL_END,
        memory_map::KERNEL_REGION_SIZE / 1024 / 1024
    );
    let _ = writeln!(
        bootcore::fmt::SerialWriter(&mut uart),
        "  Heap:   0x{:08X} - 0x{:08X} ({} MB)",
        memory_map::HEAP_BASE,
        memory_map::HEAP_END,
        memory_map::HEAP_SIZE / 1024 / 1024
    );

    // ========================================================================
    // Enter echo loop
    // ========================================================================
    uart.write_line("");
    uart.write_line("Boot complete. Entering echo mode...");
    uart.write_line("");

    loop {
        let byte = uart.read_byte();
        uart.write_byte(byte);
        if byte == b'\r' {
            uart.write_byte(b'\n');
        }
    }
}

/// Draw a simple test pattern to verify framebuffer works
fn draw_test_pattern(fb: &mailbox::FramebufferInfo) {
    let ptr = fb.address as *mut u32;
    let pixels = (fb.size / 4) as usize;

    for i in 0..pixels {
        let x = i % (fb.pitch as usize / 4);
        let y = i / (fb.pitch as usize / 4);

        // Create colored stripes
        let color = if y < (fb.height as usize / 3) {
            0x00FF0000 // Red
        } else if y < (2 * fb.height as usize / 3) {
            0x0000FF00 // Green
        } else {
            0x000000FF // Blue
        };

        // Add vertical gradient
        let brightness = (x * 255 / fb.width as usize) as u32;
        let adjusted = match color {
            0x00FF0000 => brightness << 16,
            0x0000FF00 => brightness << 8,
            0x000000FF => brightness,
            _ => color,
        };

        unsafe {
            core::ptr::write_volatile(ptr.add(i), adjusted);
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Try to print panic info over UART
    let mut uart = uart::MiniUart::new();
    let _ = uart.init(115200);

    uart.write_line("");
    uart.write_line("!!! PANIC !!!");

    if let Some(location) = info.location() {
        let _ = writeln!(
            bootcore::fmt::SerialWriter(&mut uart),
            "at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    }

    if let Some(msg) = info.message().as_str() {
        uart.write_str("message: ");
        uart.write_line(msg);
    }

    bootcore::panic::halt_loop()
}
