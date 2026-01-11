# gb-os-core

A zero-dependency, `no_std`-compatible Game Boy and Game Boy Color emulator core library, refactored from [rboy](https://github.com/mvdnes/rboy).

## Features

- **Zero external dependencies** - Only uses `alloc` for `Vec` and `Box`
- **`no_std` compatible** - Perfect for embedded systems and bare-metal development
- **Full DMG and CGB support** - Both Game Boy and Game Boy Color emulation
- **Platform abstraction through traits** - Easy integration with any platform
- **Complete CPU emulation** - Full LR35902 instruction set with accurate timing
- **GPU/PPU rendering** - Background, window, sprite support with CGB palettes
- **Audio synthesis** - 4-channel sound with band-limited synthesis
- **Cartridge support** - MBC0, MBC1, MBC2, MBC3 (with RTC), MBC5
- **Save state support** - Full serialization/deserialization of emulator state
- **Battery-backed RAM** - Auto-save detection for cartridge saves

## Architecture

```
gameboy/
├── lib.rs          # Main API: Emulator struct, config, error handling
├── cpu.rs          # LR35902 CPU with full instruction set
├── gpu.rs          # PPU with background, window, sprite rendering
├── mmu.rs          # Memory management with DMA support
├── sound.rs        # 4-channel audio with frame sequencer
├── audio.rs        # Platform-independent audio utilities (BlipBuf, resampler)
├── cartridge.rs    # MBC implementations (MBC0-MBC5)
├── keypad.rs       # Input handling
├── timer.rs        # Timer and divider registers
├── serial.rs       # Serial port with Game Boy Printer support
├── register.rs     # CPU register definitions
└── gbmode.rs       # DMG/CGB mode definitions
```

## Usage

```rust
use gameboy::{Emulator, EmulatorConfig, KeypadKey};

// Load ROM
let rom_data = include_bytes!("game.gb");

// Create emulator
let config = EmulatorConfig::default();
let mut emu = Emulator::new(rom_data, config)?;

// Main loop
loop {
    // Run one frame
    emu.step_frame();
    
    // Check if frame is ready
    if emu.frame_ready() {
        let framebuffer = emu.framebuffer(); // RGB888, 160x144
        // Render framebuffer...
    }
    
    // Handle input
    emu.key_down(KeypadKey::A);
    emu.key_up(KeypadKey::A);
}
```

## Platform Abstraction

Implement these traits for your platform:

```rust
/// Audio output handler
pub trait AudioOutput: Send {
    fn play(&mut self, left: &[f32], right: &[f32]);
    fn sample_rate(&self) -> u32;
    fn underflowed(&self) -> bool;
}

/// Time source for RTC support
pub trait TimeSource {
    fn current_time(&self) -> u64; // Unix timestamp in seconds
}

/// Serial link handler (optional)
pub trait SerialLink: Send {
    fn transfer(&mut self, value: u8) -> Option<u8>;
}
```

## Building

```bash
cargo build --release
```

For `no_std` targets:
```bash
cargo build --release --target aarch64-unknown-none
```

## Testing

```bash
cargo test
```

## License

Based on rboy by Mathijs van de Nes. See original repository for license details.
