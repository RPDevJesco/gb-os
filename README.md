# Rustacean OS + GameBoy Integration

This directory contains the files needed to integrate GameBoy emulation into Rustacean OS.

## Overview

Rather than creating a parallel/duplicate kernel, this integration:
- **Extends** the existing `boot_info.rs` with ROM fields
- **Adds** a `gameboy/` module to the existing kernel
- **Modifies** `main.rs` to detect GameBoy mode and branch accordingly
- **Uses** existing infrastructure: `drivers::keyboard`, `mm::heap`, `arch::x86`, etc.

## Integration Strategy

```
┌─────────────────────────────────────────────────────────────┐
│                    Rustacean OS Kernel                      │
├─────────────────────────────────────────────────────────────┤
│  boot_info.rs  │  Extended with rom_addr, rom_size, title   │
├────────────────┼────────────────────────────────────────────┤
│  main.rs       │  Checks is_gameboy_mode(), branches        │
│                │  - GameBoy: run_gameboy_mode()             │
│                │  - Normal:  run_gui()                      │
├────────────────┼────────────────────────────────────────────┤
│  gameboy/      │  NEW MODULE - rboy emulator (no_std)       │
│    ├── mod.rs  │  Module root, re-exports                   │
│    ├── cpu.rs  │  LR35902 CPU (all opcodes)                 │
│    ├── gpu.rs  │  PPU with DMG/CGB support                  │
│    ├── mmu.rs  │  Memory mapping, banking                   │
│    ├── mbc/    │  MBC0/1/2/3/5 cartridge controllers        │
│    ├── input.rs│  Maps KeyCode → KeypadKey                  │
│    └── display.rs│ Blits 160x144 → 640x576 scaled          │
├────────────────┼────────────────────────────────────────────┤
│  EXISTING:     │  Used directly, no changes needed          │
│  drivers/      │  keyboard, mouse, vga, ati_rage           │
│  mm/           │  heap allocator, PMM                       │
│  arch/x86/     │  gdt, idt, pic, pit                        │
│  gui/          │  Framebuffer (optional integration)        │
└────────────────┴────────────────────────────────────────────┘
```

## Files to Integrate

### Copy These Files

```
integration/kernel/src/
├── boot_info.rs          → Replace kernel/src/boot_info.rs
├── main.rs               → Replace kernel/src/main.rs  
└── gameboy/              → Copy to kernel/src/gameboy/
    ├── mod.rs
    ├── cpu.rs
    ├── gpu.rs
    ├── mmu.rs
    ├── device.rs
    ├── register.rs
    ├── keypad.rs
    ├── timer.rs
    ├── serial.rs
    ├── gbmode.rs
    ├── display.rs
    ├── input.rs
    └── mbc/
        ├── mod.rs
        ├── mbc0.rs
        ├── mbc1.rs
        ├── mbc2.rs
        ├── mbc3.rs
        └── mbc5.rs

integration/boot/
└── stage2.asm            → Replace boot/stage2.asm
                            (supports -DGAMEBOY_MODE flag)

integration/tools/
└── mkgamedisk/           → Copy to tools/mkgamedisk/
```

### Integration Steps

1. **Backup existing files**
   ```bash
   cp kernel/src/boot_info.rs kernel/src/boot_info.rs.bak
   cp kernel/src/main.rs kernel/src/main.rs.bak
   cp boot/stage2.asm boot/stage2.asm.bak
   ```

2. **Copy integration files**
   ```bash
   cp integration/kernel/src/boot_info.rs kernel/src/
   cp integration/kernel/src/main.rs kernel/src/
   cp -r integration/kernel/src/gameboy kernel/src/
   cp integration/boot/stage2.asm boot/
   cp -r integration/tools/mkgamedisk tools/
   ```

3. **Update Makefile** (add GameBoy targets)
   ```makefile
   # Add these targets to existing Makefile:
   
   gameboy: $(GAMEBOY_IMG)
   
   $(STAGE2_GB_BIN): boot/stage2.asm
   	nasm -f bin -DGAMEBOY_MODE -o $@ $<
   
   $(GAMEBOY_IMG): $(BOOT_BIN) $(STAGE2_GB_BIN) $(KERNEL_BIN)
   	# ... (see integration/Makefile)
   ```

4. **Build and test**
   ```bash
   # Build normal Rustacean OS
   make
   
   # Build GameBoy edition
   make gameboy
   
   # Create game floppy
   make game ROM=tetris.gb
   
   # Run
   make run-gb
   ```

## Boot Info Structure

The boot info at 0x500 is extended:

```
Offset  Size  Field           Normal Mode    GameBoy Mode
──────────────────────────────────────────────────────────
0x00    4     Magic           'RUST'         'GBOY'
0x04    4     E820 map addr   ✓              ✓
0x08    4     VESA enabled    ✓              ✓
0x0C    4     Framebuffer     ✓              ✓
0x10    4     Width           ✓              ✓
0x14    4     Height          ✓              ✓
0x18    4     BPP             ✓              ✓
0x1C    4     Pitch           ✓              ✓
0x20    4     ROM address     0              0x02000000
0x24    4     ROM size        0              (actual size)
0x28    32    ROM title       (unused)       "TETRIS" etc
```

## Mode Detection

```rust
// In kernel main.rs:
let boot_info = unsafe { BootInfo::from_ptr(0x500 as *const u8) };

if boot_info.is_gameboy_mode() {
    // Magic == 'GBOY' && rom_addr != 0
    run_gameboy_mode(boot_info);
} else {
    // Magic == 'RUST' (normal Rustacean OS)
    run_gui(drv_result);
}
```

## Input Mapping

Uses existing `drivers::keyboard::KeyCode`:

| Keyboard | GameBoy | KeyCode |
|----------|---------|---------|
| ↑ | D-Up | `KeyCode::Up` |
| ↓ | D-Down | `KeyCode::Down` |
| ← | D-Left | `KeyCode::Left` |
| → | D-Right | `KeyCode::Right` |
| A | A | `KeyCode::A` |
| S | B | `KeyCode::S` |
| Enter | Start | `KeyCode::Enter` |
| Space | Select | `KeyCode::Space` |

## Display Output

GameBoy (160×144) is scaled 4× to 640×576 and centered on 800×600:

```
┌──────────────────────800──────────────────────┐
│▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│ 12px
│▓▓┌────────────────640────────────────┐▓▓▓▓▓▓▓│
│▓▓│                                   │▓▓▓▓▓▓▓│
│▓▓│         GameBoy Screen            │▓▓▓▓▓▓▓│ 576px
│▓▓│           (4× scaled)             │▓▓▓▓▓▓▓│
│▓▓│                                   │▓▓▓▓▓▓▓│
│▓▓└───────────────────────────────────┘▓▓▓▓▓▓▓│
│▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│ 12px
└───────────────────────────────────────────────┘
 80px                                      80px
```

## Dependencies

The GameBoy module uses only existing kernel infrastructure:

- `alloc` - Already provided by kernel
- `drivers::keyboard` - Existing PS/2 driver
- `mm::heap` - Existing allocator
- `arch::x86::idt::ticks()` - Existing PIT timer

No new crates or external dependencies required.

## Testing

1. **QEMU** - Primary testing environment
2. **Real Hardware** - Compaq Armada E500 (target platform)

```bash
# QEMU with game
make run-game ROM=tetris.gb

# Create physical floppies
dd if=build/gameboy-system.img of=/dev/fd0 bs=512
dd if=build/game.img of=/dev/fd0 bs=512  # second floppy
```

## Known Limitations

- No save states (removed serde/typetag)
- No audio yet (future: PC speaker or SB16)
- RTC stubbed in MBC3 (no system time in no_std)
- Single game per boot (no hot-swap yet)
