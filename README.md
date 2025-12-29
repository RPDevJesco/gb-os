# gb-os

A bare-metal Game Boy emulator that runs directly on x86 hardware without an operating system. Built on top of Rustacean OS, it boots into VGA mode 13h and loads Game Boy ROMs.

## Boot Methods

gb-os now supports multiple boot methods without floppy emulation:

### 1. Floppy Disk (Legacy)
- Standard 1.44MB floppy image
- CHS addressing via INT 13h
- Boot drive: 0x00

### 2. CD-ROM (El Torito No-Emulation)
- ISO 9660 with El Torito boot specification
- **No floppy emulation** - boots directly from CD sectors
- LBA addressing via INT 13h extensions
- Boot drive: 0x80+ (typically 0xE0 for CD)

### 3. USB/HDD (Raw Image)
- Write floppy image directly to USB drive
- LBA addressing via INT 13h extensions
- Boot drive: 0x80+

### 4. UEFI (Coming Soon)
- EFI System Partition support
- 64-bit bootloader
- GOP framebuffer

## Building

### Docker (Recommended)

```bash
# Build GameBoy edition
docker build -t gb-os-builder .
docker run --rm -v $(pwd)/output:/output gb-os-builder

# Build with embedded ROM
docker run --rm \
    -v $(pwd)/output:/output \
    -v /path/to/game.gb:/input/game.gb:ro \
    gb-os-builder
```

### Native Build

```bash
# Requirements: nasm, rust nightly, xorriso/genisoimage

# Build GameBoy edition
make gameboy

# Build and run in QEMU
make run-gb      # Floppy boot
make run-gb-cd   # CD-ROM boot
```

## Output Files

```
output/
├── gameboy-system.img    # Floppy/USB disk image (1.44MB)
├── gameboy-system.iso    # CD image (no-emulation boot)
├── kernel.bin            # Raw kernel binary (debugging)
└── mkgamedisk            # ROM to floppy converter
```

## Running

### QEMU

```bash
# Floppy boot
qemu-system-i386 -fda output/gameboy-system.img -boot a -m 256M

# CD-ROM boot (no-emulation)
qemu-system-i386 -cdrom output/gameboy-system.iso -boot d -m 256M

# USB emulation
qemu-system-i386 -drive file=output/gameboy-system.img,format=raw -boot c -m 256M
```

### Real Hardware

**USB Drive:**
```bash
# Linux
sudo dd if=output/gameboy-system.img of=/dev/sdX bs=512
sync

# Windows (PowerShell as Admin)
.\floppy-writer.ps1 -USB 1
```

**CD-ROM:**
```bash
# Burn gameboy-system.iso to CD-R
# Boot from CD drive
```

## Boot Process

### Stage 1 (boot.asm - 512 bytes)
1. Detect boot media type (floppy/CD/HDD)
2. Load stage 2 using appropriate method:
  - Floppy: CHS addressing
  - CD/HDD: INT 13h extensions (LBA)
3. Verify stage 2 magic and jump

### Stage 2 (stage2.asm - 16KB)
1. Query E820 memory map
2. Enable A20 line
3. Set VGA mode 13h (320x200x256)
4. Load kernel (256KB) using boot media method
5. Load ROM if present
6. Switch to 32-bit protected mode
7. Copy kernel to 1MB, ROM to 3MB
8. Build boot info structure
9. Jump to kernel

### Boot Info Structure (0x500)

```
Offset  Size  Field
──────────────────────────────────
0x00    4     Magic ('GBOY')
0x04    4     E820 memory map pointer
0x08    4     VGA mode (0x13)
0x0C    4     Framebuffer (0xA0000)
0x10    4     Width (320)
0x14    4     Height (200)
0x18    4     BPP (8)
0x1C    4     Pitch (320)
0x20    4     ROM address (0x300000 if loaded)
0x24    4     ROM size
0x28    32    ROM title
0x48    4     Boot media type
0x4C    4     Boot drive number
```

## Installation Capability

When booted from CD-ROM, the system can be installed to a target drive:

1. Boot from CD
2. Kernel detects CD boot via boot_info
3. Installation menu presented
4. Write floppy image to selected drive
5. Reboot from installed media

## Technical Details

### No-Emulation Boot

Traditional CD boot uses "floppy emulation" where the BIOS pretends the CD is a floppy. This has limitations:
- Limited to 1.44MB boot image
- BIOS geometry emulation can be unreliable
- Not supported by all UEFI implementations

Our no-emulation boot:
- Loads boot code directly from CD sectors
- Uses INT 13h extensions for all disk access
- Supports larger boot images
- Better compatibility with modern systems
- Prepares path to UEFI boot

### Memory Map

```
Address         Size      Contents
──────────────────────────────────────────────
0x00000500      72        Boot info structure
0x00001000      ~2KB      E820 memory map
0x00007C00      512       Stage 1 bootloader
0x00007E00      16KB      Stage 2 bootloader
0x00020000      256KB     Kernel (temporary)
0x00040000      2MB       ROM (temporary)
0x000A0000      64KB      VGA framebuffer
0x00100000      256KB     Kernel (final)
0x00300000      2MB       ROM (final)
```

### INT 13h Extensions

For CD-ROM and HDD boot, we use the extended INT 13h functions:

```
AH=41h - Check Extensions Present
AH=42h - Extended Read Sectors
AH=48h - Get Drive Parameters (Extended)
```

The Disk Address Packet (DAP) structure:
```
Offset  Size  Field
──────────────────────────────────
0x00    1     Size (0x10)
0x01    1     Reserved (0)
0x02    2     Sector count
0x04    4     Buffer address (seg:off)
0x08    8     Starting LBA (64-bit)
```

## UEFI Preparation

This release prepares for UEFI boot by:
1. Removing floppy emulation dependency
2. Using LBA addressing throughout
3. Detecting boot media type
4. Passing boot info to kernel
5. Structuring ISO for future EFI partition

Next phase will add:
- EFI System Partition (FAT32)
- UEFI bootloader (PE32+)
- GOP framebuffer support
- Memory map via EFI services

## Controls

| Keyboard     | Game Boy | Alternate |
|--------------|----------|-----------|
| Arrow Keys   | D-Pad    | WASD      |
| A            | A Button | Z         |
| S            | B Button | X         |
| Enter        | Start    | —         |
| Space        | Select   | —         |

## License

MIT License — See LICENSE file for details.
