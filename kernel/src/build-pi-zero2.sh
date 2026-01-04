#!/bin/bash
# Build script for Pi Zero 2 W kernel
# Run from kernel/src directory

set -e

echo "=== Building Pi Zero 2 W Kernel ==="

# Ensure we have the right target
rustup target add aarch64-unknown-none 2>/dev/null || true

# Build release binary
RUSTFLAGS="-C link-arg=-Tplatform/pi-zero2/linker.ld" \
cargo build --release --package rustboot-pi-zero2 --target aarch64-unknown-none

# Convert ELF to binary
rust-objcopy -O binary target/aarch64-unknown-none/release/kernel8 kernel8.img

# Show size
ls -la kernel8.img

echo ""
echo "=== Build Complete ==="
echo "Output: kernel8.img"
echo ""
echo "To use:"
echo "1. Format SD card as FAT32"
echo "2. Copy Raspberry Pi firmware files (start.elf, fixup.dat) from:"
echo "   https://github.com/raspberrypi/firmware/tree/master/boot"
echo "3. Copy kernel8.img to SD card"
echo "4. Copy platform/pi-zero2/config.txt to SD card"
