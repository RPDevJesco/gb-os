# gb-os

A bare-metal Game Boy emulator that runs directly on x86 hardware without an operating system. Built on top of Rustacean OS, it boots into VGA mode 13h and loads Game Boy ROMs from floppy disk.

## Overview

gb-os is a unique intersection of retro computing and emulation. The emulator boots directly from floppy disk (or CD image), sets up protected mode, and runs Game Boy games at native resolution on vintage hardware like a Pentium III laptop.

**Target Hardware:** Pentium III Compaq Armada E500 (and compatible x86 systems)

## Features

- **Bare-metal execution** — No underlying OS required
- **VGA Mode 13h** — 320×200×256 color display, perfect for 160×144 Game Boy screen
- **Floppy disk boot** — ROM loaded from 1.44MB floppy or embedded in disk image
- **Full LR35902 CPU emulation** — Complete Sharp LR35902 (Z80-derivative) instruction set
- **PPU emulation** — Background, window, and sprite rendering with proper timing
- **Memory Bank Controllers** — MBC0, MBC1, MBC2, MBC3, and MBC5 support
- **PS/2 keyboard input** — Arrow keys for D-pad, A/S for buttons, Enter/Space for Start/Select
- **Game Boy Color detection** — Automatically detects CGB-compatible ROMs

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    gb-os Kernel                     │
├─────────────────────────────────────────────────────────────┤
│  boot.asm       │  Stage 1 bootloader (512 bytes)           │
│  stage2.asm     │  Stage 2: E820, A20, VGA 13h, ROM load    │
├─────────────────────────────────────────────────────────────┤
│  boot_info.rs   │  Boot info structure at 0x500             │
│  main.rs        │  Kernel entry, emulation loop             │
├─────────────────────────────────────────────────────────────┤
│  gameboy/       │  Emulator core (ported from rboy)         │
│    ├── cpu.rs   │  LR35902 CPU with all opcodes             │
│    ├── gpu.rs   │  PPU: 160×144 display, tiles, sprites     │
│    ├── mmu.rs   │  Memory mapping and I/O registers         │
│    ├── mbc/     │  MBC0/1/2/3/5 cartridge controllers       │
│    ├── timer.rs │  DIV and TIMA timer emulation             │
│    ├── keypad.rs│  Joypad register (0xFF00)                 │
│    └── input.rs │  Maps PS/2 KeyCode → KeypadKey            │
├─────────────────────────────────────────────────────────────┤
│  drivers/       │  Hardware abstraction                     │
│    ├── keyboard │  PS/2 keyboard with IRQ handler           │
│    └── vga      │  VGA mode 13h framebuffer                 │
├─────────────────────────────────────────────────────────────┤
│  arch/x86/      │  x86 architecture support                 │
│    ├── gdt      │  Global Descriptor Table                  │
│    ├── idt      │  Interrupt Descriptor Table               │
│    └── pic      │  8259 PIC configuration                   │
└─────────────────────────────────────────────────────────────┘
```

## Memory Map

```
Address         Size      Contents
────────────────────────────────────────────
0x00000500      72        Boot info structure
0x00007C00      512       Stage 1 bootloader
0x00007E00      16KB      Stage 2 bootloader
0x000A0000      64KB      VGA framebuffer (mode 13h)
0x00100000      ~256KB    Kernel code and data
0x00300000      2MB       ROM data (loaded by stage 2)
```

## Boot Info Structure

The bootloader passes information to the kernel at address 0x500:

```
Offset  Size  Field
──────────────────────────────────
0x00    4     Magic ('GBOY' = 0x594F4247)
0x04    4     E820 memory map address
0x08    4     VGA mode (0x13)
0x0C    4     Framebuffer address (0xA0000)
0x10    4     Screen width (320)
0x14    4     Screen height (200)
0x18    4     Bits per pixel (8)
0x1C    4     Pitch (320)
0x20    4     ROM address (0x300000 if loaded)
0x24    4     ROM size in bytes
0x28    32    ROM title (from cartridge header)
```

## Controls

| Keyboard     | Game Boy | Alternate |
|--------------|----------|-----------|
| Arrow Keys   | D-Pad    | —         |
| A            | A Button | Z         |
| S            | B Button | X         |
| Enter        | Start    | —         |
| Space        | Select   | —         |

WASD controls are also available via alternate input configuration.

## Building

### Prerequisites

- Docker (recommended) or local toolchain:
   - Rust nightly with `rust-src` component
   - NASM assembler
   - GNU binutils (objcopy)

### Using Docker (Recommended)

```bash
# Build the Docker image
docker build -t gb-os-builder .

# Build GameBoy edition
docker run --rm -v $(pwd)/output:/output gb-os-builder /build.sh --gameboy

# Build with embedded ROM
docker run --rm \
    -v $(pwd)/output:/output \
    -v /path/to/game.gb:/input/game.gb:ro \
    gb-os-builder /build.sh --gameboy
```

### Using Make

```bash
# Build GameBoy edition
make gameboy

# Create game floppy from ROM
make game ROM=path/to/game.gb

# Run in QEMU
make run-gb
```

### Output Files

```
output/
├── gameboy-system.img    # Floppy disk image (1.44MB)
├── gameboy-system.iso    # CD image with floppy emulation
├── kernel.bin            # Raw kernel binary (for debugging)
└── mkgamedisk            # Tool to create game floppies
```

## Game Floppy Format

ROMs can be embedded in the system image or loaded from a separate game floppy:

```
Sector 0 (512 bytes): Header
  Offset 0x00: Magic "GBOY" (4 bytes)
  Offset 0x04: ROM size in bytes (4 bytes, little-endian)
  Offset 0x08: ROM title (32 bytes, null-padded)
  Offset 0x28: Reserved (472 bytes)

Sectors 1+: Raw Game Boy ROM data
```

Create a game floppy:
```bash
./output/mkgamedisk game.gb output/game.img
```

## Running

### QEMU

```bash
# Floppy image
qemu-system-i386 -fda output/gameboy-system.img -boot a -m 256M

# CD image
qemu-system-i386 -cdrom output/gameboy-system.iso -boot d -m 256M

# With separate game floppy (swap in QEMU monitor)
qemu-system-i386 -fda output/gameboy-system.img -boot a -m 256M
# In QEMU monitor (Ctrl+Alt+2): change floppy0 output/game.img
```

### Real Hardware

Write the image to a USB floppy drive or create a bootable USB:

**Windows (PowerShell as Admin):**
```powershell
.\floppy-writer.ps1 -Floppy           # Write to A:
.\floppy-writer.ps1 -USB 1            # Write to USB drive
.\floppy-writer.ps1 -ListDisks        # List available drives
```

**Linux:**
```bash
sudo dd if=output/gameboy-system.img of=/dev/fd0 bs=512
# or for USB
sudo dd if=output/gameboy-system.img of=/dev/sdX bs=512
```

## Supported Cartridge Types

| MBC Type | Cartridge Byte | ROM Size | RAM Size | Examples |
|----------|----------------|----------|----------|----------|
| MBC0     | 0x00           | 32KB     | None     | Tetris   |
| MBC1     | 0x01-0x03      | Up to 2MB| Up to 32KB | Super Mario Land |
| MBC2     | 0x05-0x06      | Up to 256KB | 512×4 bits | —   |
| MBC3     | 0x0F-0x13      | Up to 2MB| Up to 32KB | Pokémon Gold/Silver |
| MBC5     | 0x19-0x1E      | Up to 8MB| Up to 128KB | Pokémon Red/Blue (later prints) |

## Display

The Game Boy's 160×144 display is rendered at 1:1 scale centered on the 320×200 VGA screen:

```
┌─────────────────320─────────────────┐
│▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒│ 28px
│▒▒▒▒┌─────────160─────────┐▒▒▒▒▒▒▒▒▒│
│▒▒▒▒│                     │▒▒▒▒▒▒▒▒▒│
│▒▒▒▒│   Game Boy Screen   │▒▒▒▒▒▒▒▒▒│ 144px
│▒▒▒▒│       (1:1)         │▒▒▒▒▒▒▒▒▒│
│▒▒▒▒│                     │▒▒▒▒▒▒▒▒▒│
│▒▒▒▒└─────────────────────┘▒▒▒▒▒▒▒▒▒│
│▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒│ 28px
└────────────────────────────────────┘
 80px                           80px
```

RGB output from the GPU is converted to VGA palette grayscale (indices 16-31).

## Technical Details

### CPU Emulation

The Sharp LR35902 is a Z80-derivative running at 4.19 MHz. The emulator implements:

- All 256 base opcodes plus CB-prefixed instructions
- Accurate flag handling (Z, N, H, C)
- Interrupt handling (VBlank, LCD STAT, Timer, Serial, Joypad)
- HALT and STOP instructions
- DI/EI with proper 1-instruction delay

### PPU Emulation

- Mode 0: HBlank (204 cycles)
- Mode 1: VBlank (4560 cycles, 10 scanlines)
- Mode 2: OAM Search (80 cycles)
- Mode 3: Pixel Transfer (172 cycles)

Renders at ~59.7 fps (70224 T-cycles per frame).

### Timing

The emulation loop runs one frame (70224 cycles) then blits to the VGA framebuffer. The kernel uses HLT to wait for timer interrupts between frames.

## Project Structure

```
gb-os/
├── boot/
│   ├── boot.asm          # Stage 1 bootloader
│   └── stage2.asm        # Stage 2 (memory map, A20, VGA, ROM load)
├── kernel/
│   ├── src/
│   │   ├── main.rs       # Kernel entry point
│   │   ├── boot_info.rs  # Boot info parser
│   │   ├── gameboy/      # Emulator core
│   │   ├── drivers/      # Hardware drivers
│   │   ├── arch/         # x86 architecture code
│   │   └── mm/           # Memory management
│   └── Cargo.toml
├── tools/
│   └── mkgamedisk/       # ROM to floppy converter
├── Dockerfile
├── Makefile
└── docker-build.sh
```

## Known Limitations

- **No audio** — Sound emulation not implemented (PPU only)
- **No save states** — Battery-backed RAM not persisted to disk
- **DMG only** — Game Boy Color enhanced features not supported
- **No link cable** — Serial communication stub only

## Credits

- Emulator core based on [rboy](https://github.com/mvdnes/rboy) by mvdnes
- Kernel infrastructure from [Rustacean OS](https://github.com/RPDevJesco/rustacean-os)

## License

MIT License — See LICENSE file for details.

https://github.com/user-attachments/assets/3a013bdd-057a-4532-b4c4-2e3edb64e180

<img width="1900" height="1517" alt="image" src="https://github.com/user-attachments/assets/72650c59-ec70-40de-8877-8544439db732" />

