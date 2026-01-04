#!/bin/bash
# Build script for GPi Case 2W kernel
# This builds a standalone kernel8.img for bare-metal Pi Zero 2 W

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== GPi Case 2W Kernel Build ===${NC}"
echo ""

# Check for required tools
command -v rustc >/dev/null 2>&1 || { echo -e "${RED}Error: rustc not found${NC}"; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo -e "${RED}Error: cargo not found${NC}"; exit 1; }

# Ensure we have the right target
echo "Checking Rust target..."
rustup target add aarch64-unknown-none 2>/dev/null || true

# Create a minimal Cargo project if needed
if [ ! -f "Cargo.toml" ]; then
    echo "Creating Cargo.toml..."
    cat > Cargo.toml << 'EOF'
[package]
name = "gpi-kernel"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "kernel8"
path = "main.rs"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = "symbols"

[profile.dev]
panic = "abort"
EOF
fi

# Create output directory
mkdir -p output

# Build
echo ""
echo "Compiling kernel..."
RUSTFLAGS="-C link-arg=-Tlinker.ld" \
cargo build --release --target aarch64-unknown-none

# Convert ELF to binary
echo "Converting to binary..."
rust-objcopy -O binary target/aarch64-unknown-none/release/kernel8 output/kernel8.img

# Copy config
echo "Copying config.txt..."
cp config.txt output/

# Show results
echo ""
echo -e "${GREEN}=== Build Complete ===${NC}"
echo ""
ls -la output/
echo ""

SIZE=$(stat -c%s output/kernel8.img 2>/dev/null || stat -f%z output/kernel8.img 2>/dev/null)
SIZE_KB=$((SIZE / 1024))
echo "Kernel size: ${SIZE_KB} KB ($SIZE bytes)"
echo ""

echo -e "${GREEN}=== SD Card Setup ===${NC}"
echo ""
echo "1. Format SD card as FAT32"
echo ""
echo "2. Download Raspberry Pi firmware from:"
echo "   https://github.com/raspberrypi/firmware/tree/master/boot"
echo "   Required files: start.elf, fixup.dat"
echo ""
echo "3. Copy to SD card root:"
echo "   - output/kernel8.img"
echo "   - output/config.txt"
echo "   - start.elf (from Pi firmware)"
echo "   - fixup.dat (from Pi firmware)"
echo ""
echo -e "${YELLOW}LED Blink Pattern:${NC}"
echo "  1 blink  = Kernel started"
echo "  2 blinks = GPIO configured for DPI"
echo "  3 blinks = Framebuffer initialized"
echo "  4 blinks = Test pattern drawn"
echo "  LED ON   = Running normally"
echo ""
echo "  Rapid blinks then N = Error code N"
echo "    Error 1 = GPIO configuration failed"
echo "    Error 2 = Framebuffer init failed"
echo ""
echo -e "${GREEN}Done!${NC}"
