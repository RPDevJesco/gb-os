# GB-OS Bare-Metal Bootloader for Raspberry Pi Zero 2W

This directory contains the bare-metal bootloader and runtime for running the Game Boy emulator directly on the Raspberry Pi Zero 2W hardware without an operating system.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    RPi Zero 2W Boot Process                      │
└─────────────────────────────────────────────────────────────────┘

1. Power On
      │
      ▼
┌─────────────────┐
│  VideoCore GPU  │  ← First-stage bootloader (ROM)
│  Loads from SD  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  bootcode.bin   │  ← Second-stage bootloader (SD card)
│  (GPU firmware) │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   start.elf     │  ← GPU firmware, reads config.txt
│   + config.txt  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   kernel8.img   │  ← Our bare-metal kernel!
│   @ 0x80000     │     (64-bit, AArch64)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│    boot.S       │  ← Assembly startup code
│  (_start entry) │     - Parks cores 1-3
│                 │     - Sets up EL1
│                 │     - Initializes stack
│                 │     - Clears BSS
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   main.rs       │  ← Rust entry point
│ (kernel_main)   │     - Sets up exception vectors
│                 │     - Initializes UART
│                 │     - Enters emulator loop
└─────────────────┘
```

## Memory Map

```
Address Range        Size      Description
─────────────────────────────────────────────────────────────
0x0000_0000 - 0x0007_FFFF    512 KB    Reserved (GPU/VideoCore)
0x0008_0000 - ............    ...       Kernel load address
  .text.boot                            Boot stub (must be first!)
  .text.vectors                         Exception vector table
  .text                                 Code
  .rodata                               Read-only data
  .data                                 Initialized data
  .bss                                  Uninitialized data (zeroed)
  .stack                     64 KB      Stack (grows down)
  .heap                      ...        Dynamic memory
............  - 0x3AFF_FFFF    ...       Available RAM (~448 MB)
0x3F00_0000 - 0x3FFF_FFFF    16 MB     Peripheral registers (MMIO)
0x4000_0000 - 0x40FF_FFFF    16 MB     Local peripherals
```

## File Structure

```
src/rpi-zero2/
├── boot.S          # Assembly startup code
│                   # - Entry point (_start)
│                   # - Exception level transitions
│                   # - Stack setup
│                   # - BSS clearing
│                   # - Exception vector table
│
├── linker.ld       # Linker script
│                   # - Memory layout
│                   # - Section placement
│                   # - Symbol definitions
│
└── main.rs         # Rust kernel entry
                    # - kernel_main()
                    # - Exception handlers
                    # - UART driver
                    # - Panic handler
```

## Exception Levels

The ARM Cortex-A53 has multiple exception levels:

```
EL3 - Secure Monitor (not used on RPi)
 │
 ▼
EL2 - Hypervisor (RPi boots here)
 │
 └──► Our boot.S transitions to EL1
      │
      ▼
EL1 - OS Kernel ◄── We run here
 │
 ▼
EL0 - User mode (not used, no OS)
```

## Building

### Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# The rust-toolchain.toml will automatically install:
# - Nightly Rust
# - rust-src (for build-std)
# - llvm-tools (for objcopy)
# - aarch64-unknown-none-softfloat target
```

### Build Commands

```bash
# Build release kernel
./build.sh

# Build debug kernel (with symbols)
./build.sh debug

# Clean build artifacts
./build.sh clean

# Create complete SD card directory
./build.sh sdcard
```

### Manual Build

```bash
# Build with cargo
cargo build --release --target aarch64-unknown-none-softfloat \
    -Zbuild-std=core,alloc \
    -Zbuild-std-features=compiler-builtins-mem

# Convert ELF to raw binary
rust-objcopy -O binary \
    target/aarch64-unknown-none-softfloat/release/kernel8 \
    kernel8.img
```

## SD Card Setup

### Required Files

Copy these files to a FAT32 formatted SD card:

```
/boot/
├── bootcode.bin    # RPi bootloader (from firmware repo)
├── start.elf       # GPU firmware (from firmware repo)
├── fixup.dat       # Memory fixup (from firmware repo)
├── config.txt      # Boot configuration
└── kernel8.img     # Our compiled kernel
```

### Download Firmware Files

```bash
# From Raspberry Pi firmware repository
wget https://github.com/raspberrypi/firmware/raw/master/boot/bootcode.bin
wget https://github.com/raspberrypi/firmware/raw/master/boot/start.elf
wget https://github.com/raspberrypi/firmware/raw/master/boot/fixup.dat
```

Or use `./build.sh sdcard` to automatically download and assemble all files.

## Debugging

### UART Output

The bootloader initializes the Mini UART (UART1) at 115200 baud for debug output.

**GPIO Pins:**
- GPIO14 (Pin 8): TX
- GPIO15 (Pin 10): RX
- GND (Pin 6): Ground

**Connection:**
```
RPi Zero 2W          USB-Serial Adapter
─────────────        ──────────────────
GPIO14 (TX)    ───►  RX
GPIO15 (RX)    ◄───  TX
GND            ───►  GND
```

**Viewing Output:**
```bash
# Linux/macOS
screen /dev/ttyUSB0 115200

# Or with minicom
minicom -D /dev/ttyUSB0 -b 115200
```

### QEMU Emulation

For testing without hardware:

```bash
# Install QEMU
sudo apt install qemu-system-aarch64  # Debian/Ubuntu
brew install qemu                      # macOS

# Run kernel in QEMU (RPi 3 is closest to Zero 2W)
qemu-system-aarch64 \
    -M raspi3b \
    -serial stdio \
    -display none \
    -kernel kernel8.img
```

**Note:** QEMU's Raspberry Pi emulation is incomplete. Some peripherals may not work correctly.

## Exception Handling

The bootloader sets up exception vectors at EL1:

| Vector Offset | Exception Type | Handler |
|--------------|----------------|---------|
| 0x000 | Sync (SP_EL0) | `unhandled_exception` |
| 0x080 | IRQ (SP_EL0) | `unhandled_exception` |
| 0x100 | FIQ (SP_EL0) | `unhandled_exception` |
| 0x180 | SError (SP_EL0) | `unhandled_exception` |
| 0x200 | Sync (SP_ELx) | `sync_exception_handler` |
| 0x280 | IRQ (SP_ELx) | `irq_handler` |
| 0x300 | FIQ (SP_ELx) | `unhandled_exception` |
| 0x380 | SError (SP_ELx) | `unhandled_exception` |

### Exception Output

When an exception occurs, the handler prints:
- Exception Syndrome Register (ESR_EL1)
- Fault Address Register (FAR_EL1)
- Exception Link Register (ELR_EL1)
- Exception class and type
- All general-purpose registers

## Multicore Support

Currently, cores 1-3 are parked in a low-power wait loop. Future plans:

```
Core 0: Main emulator execution
Core 1: Audio processing (if enabled)
Core 2: Display rendering
Core 3: Reserved / future use
```

To wake secondary cores, write their entry address to:
- Core 1: `0x4000_00E0`
- Core 2: `0x4000_00E8`
- Core 3: `0x4000_00F0`

Then send an event with `sev` instruction.

## Troubleshooting

### No UART Output

1. Check GPIO connections (TX/RX not swapped?)
2. Verify `enable_uart=1` in config.txt
3. Ensure `dtoverlay=disable-bt` is set
4. Check baud rate (115200)

### Kernel Doesn't Boot

1. Verify all boot files are present on SD card
2. Check SD card is FAT32 formatted
3. Ensure `kernel8.img` (not .elf) is copied
4. Try `uart_2ndstage=1` in config.txt for boot debug

### Exceptions During Boot

1. Check exception output via UART
2. Common causes:
   - Stack overflow (increase `__stack_size` in linker.ld)
   - Null pointer dereference
   - Unaligned access
   - Invalid memory access

## References

- [ARM Cortex-A53 Technical Reference Manual](https://developer.arm.com/documentation/ddi0500/latest/)
- [BCM2710 Peripherals](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf) (BCM2711 is similar)
- [Raspberry Pi Boot Process](https://www.raspberrypi.com/documentation/computers/raspberry-pi.html#raspberry-pi-boot-eeprom)
- [RPi Bare Metal Tutorial](https://github.com/bztsrc/raspi3-tutorial)
