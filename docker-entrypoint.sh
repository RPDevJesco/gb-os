#!/bin/bash
# =============================================================================
# docker-entrypoint.sh - Docker Container Build Entry Point
# =============================================================================
# This script runs inside the Docker container to build the kernel.
# =============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Configuration
TARGET="aarch64-unknown-none-softfloat"
KERNEL_NAME="kernel8"
KERNEL_IMG="${KERNEL_NAME}.img"
BUILD_MODE="${BUILD_MODE:-release}"

info "==================================="
info "  GB-OS Bare-Metal Docker Builder"
info "==================================="
echo ""

# Show toolchain info
info "Rust toolchain:"
rustc --version
cargo --version
echo ""

# Determine build profile
if [ "$BUILD_MODE" = "debug" ]; then
    CARGO_PROFILE="dev"
    TARGET_DIR="target/$TARGET/debug"
    info "Building in DEBUG mode..."
else
    CARGO_PROFILE="release"
    TARGET_DIR="target/$TARGET/release"
    info "Building in RELEASE mode..."
fi

# Build the kernel
info "Compiling kernel..."
cargo build \
    --profile "$CARGO_PROFILE" \
    --target "$TARGET" \
    -Zbuild-std=core,alloc \
    -Zbuild-std-features=compiler-builtins-mem

if [ ! -f "$TARGET_DIR/$KERNEL_NAME" ]; then
    error "Build failed: $TARGET_DIR/$KERNEL_NAME not found"
fi

success "Compilation complete: $TARGET_DIR/$KERNEL_NAME"

# Convert ELF to raw binary
info "Creating binary image..."
rust-objcopy -O binary "$TARGET_DIR/$KERNEL_NAME" "$KERNEL_IMG"

# Get file size
IMG_SIZE=$(stat -c%s "$KERNEL_IMG" 2>/dev/null || stat -f%z "$KERNEL_IMG" 2>/dev/null)
IMG_SIZE_KB=$((IMG_SIZE / 1024))

success "Created $KERNEL_IMG (${IMG_SIZE_KB} KB)"

# Create output directory structure if requested
if [ "$CREATE_SDCARD" = "1" ]; then
    info "Creating SD card output directory..."
    
    SDCARD_DIR="sdcard_output"
    mkdir -p "$SDCARD_DIR"
    
    # Copy kernel image
    cp "$KERNEL_IMG" "$SDCARD_DIR/"
    
    # Copy config.txt
    if [ -f "sdcard/config.txt" ]; then
        cp "sdcard/config.txt" "$SDCARD_DIR/"
    fi
    
    # Download boot files if not present
    FIRMWARE_URL="https://github.com/raspberrypi/firmware/raw/master/boot"
    
    for file in bootcode.bin start.elf fixup.dat; do
        if [ ! -f "$SDCARD_DIR/$file" ]; then
            info "Downloading $file..."
            curl -sL "$FIRMWARE_URL/$file" -o "$SDCARD_DIR/$file" || warn "Failed to download $file"
        fi
    done
    
    success "SD card directory ready: $SDCARD_DIR/"
    ls -la "$SDCARD_DIR/"
fi

# =============================================================================
# Copy outputs to dedicated output folder
# =============================================================================
OUTPUT_DIR="output"
info "Copying build artifacts to $OUTPUT_DIR/..."

mkdir -p "$OUTPUT_DIR"

# Copy kernel image
cp "$KERNEL_IMG" "$OUTPUT_DIR/"

# Copy ELF file (useful for debugging with symbols)
cp "$TARGET_DIR/$KERNEL_NAME" "$OUTPUT_DIR/${KERNEL_NAME}.elf"

# Copy config.txt
if [ -f "sdcard/config.txt" ]; then
    cp "sdcard/config.txt" "$OUTPUT_DIR/"
fi

# If sdcard output was created, copy those files too
if [ -d "sdcard_output" ]; then
    cp -r sdcard_output/* "$OUTPUT_DIR/" 2>/dev/null || true
fi

success "Build artifacts copied to $OUTPUT_DIR/"

# Print summary
echo ""
info "==================================="
info "  Build Complete!"
info "==================================="
echo ""
info "Output files in ./$OUTPUT_DIR/:"
ls -la "$OUTPUT_DIR/"
echo ""
info "To deploy to SD card:"
echo "  1. Format SD card as FAT32"
echo "  2. Copy all files from ./$OUTPUT_DIR/ to SD card"
echo "  3. If boot files missing, download from RPi firmware repo:"
echo "     - bootcode.bin"
echo "     - start.elf" 
echo "     - fixup.dat"
echo ""
