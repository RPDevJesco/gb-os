#!/bin/bash
# Build script for Pi Zero 2 W kernel with GPi Case 2W support
# Run from kernel/src directory
#
# Usage:
#   ./build-pi-zero2-gpi.sh          # Build with GPi Case 2W support
#   ./build-pi-zero2-gpi.sh --hdmi   # Build for HDMI output
#   ./build-pi-zero2-gpi.sh --help   # Show help

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default settings
MODE="gpi"
OUTPUT_DIR="build-output"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --hdmi)
            MODE="hdmi"
            shift
            ;;
        --gpi)
            MODE="gpi"
            shift
            ;;
        --help|-h)
            echo "Build script for Pi Zero 2 W kernel"
            echo ""
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --gpi      Build for GPi Case 2W DPI display (default)"
            echo "  --hdmi     Build for HDMI output"
            echo "  --help     Show this help message"
            echo ""
            echo "Output files will be in: $OUTPUT_DIR/"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

echo -e "${GREEN}=== Building Pi Zero 2 W Kernel ===${NC}"
echo -e "Mode: ${YELLOW}$MODE${NC}"
echo ""

# Ensure we have the right target
echo "Checking Rust target..."
rustup target add aarch64-unknown-none 2>/dev/null || true

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Select the appropriate main.rs
if [ "$MODE" = "gpi" ]; then
    echo -e "${YELLOW}Building for GPi Case 2W (DPI display)${NC}"

    # Check if main_gpi.rs exists and swap it in
    if [ -f "platform/pi-zero2/src/main_gpi.rs" ]; then
        # Backup original main.rs if not already backed up
        if [ ! -f "platform/pi-zero2/src/main_hdmi.rs" ]; then
            cp platform/pi-zero2/src/main.rs platform/pi-zero2/src/main_hdmi.rs
        fi
        # Use GPi main
        cp platform/pi-zero2/src/main_gpi.rs platform/pi-zero2/src/main.rs
    fi

    CONFIG_FILE="platform/pi-zero2/config_gpi_case_2w.txt"
else
    echo -e "${YELLOW}Building for HDMI output${NC}"

    # Restore HDMI main if backup exists
    if [ -f "platform/pi-zero2/src/main_hdmi.rs" ]; then
        cp platform/pi-zero2/src/main_hdmi.rs platform/pi-zero2/src/main.rs
    fi

    CONFIG_FILE="platform/pi-zero2/config.txt"
fi

# Build release binary
echo ""
echo "Compiling kernel..."
RUSTFLAGS="-C link-arg=-Tplatform/pi-zero2/linker.ld" \
cargo build --release --package rustboot-pi-zero2 --target aarch64-unknown-none

# Convert ELF to binary
echo "Converting to binary..."
rust-objcopy -O binary target/aarch64-unknown-none/release/kernel8 "$OUTPUT_DIR/kernel8.img"

# Copy config file
echo "Copying config file..."
if [ -f "$CONFIG_FILE" ]; then
    cp "$CONFIG_FILE" "$OUTPUT_DIR/config.txt"
else
    echo -e "${YELLOW}Warning: Config file not found: $CONFIG_FILE${NC}"
fi

# Show results
echo ""
echo -e "${GREEN}=== Build Complete ===${NC}"
echo ""
ls -la "$OUTPUT_DIR/"
echo ""

# Calculate size
SIZE=$(stat -f%z "$OUTPUT_DIR/kernel8.img" 2>/dev/null || stat -c%s "$OUTPUT_DIR/kernel8.img" 2>/dev/null)
SIZE_KB=$((SIZE / 1024))
echo "Kernel size: ${SIZE_KB} KB"
echo ""

# Instructions
echo -e "${GREEN}=== SD Card Setup Instructions ===${NC}"
echo ""
echo "1. Format SD card as FAT32"
echo ""
echo "2. Download Raspberry Pi firmware files from:"
echo "   https://github.com/raspberrypi/firmware/tree/master/boot"
echo "   Required: start.elf, fixup.dat"
echo ""
echo "3. Copy these files to SD card root:"
echo "   - $OUTPUT_DIR/kernel8.img"
echo "   - $OUTPUT_DIR/config.txt"
echo "   - start.elf (from Pi firmware)"
echo "   - fixup.dat (from Pi firmware)"
echo ""

if [ "$MODE" = "gpi" ]; then
    echo -e "${YELLOW}Note: This build is configured for GPi Case 2W DPI display.${NC}"
    echo "The display will use GPIO pins 0-21 for video output."
    echo "HDMI will be disabled."
else
    echo -e "${YELLOW}Note: This build is configured for HDMI output.${NC}"
    echo "Default resolution: 1280x720"
fi

echo ""
echo -e "${GREEN}Done!${NC}"
