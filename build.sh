#!/bin/bash
#
# gb-os Build Script
#
# This script builds the bare-metal GameBoy emulator with support for:
#   - Floppy disk boot (legacy 1.44MB)
#   - CD-ROM boot via El Torito no-emulation mode
#   - USB/HDD boot via raw image
#   - UEFI boot preparation (future)
#
# Usage (inside container):
#   /build.sh                 # Build GameBoy edition
#   /build.sh --gameboy       # Build GameBoy edition
#   /build.sh --both          # Build both editions
#   /build.sh --tools         # Build mkgamedisk tool only
#

set -e

BUILD_NORMAL="no"
BUILD_GAMEBOY="yes"
BUILD_TOOLS="no"

# Parse arguments
for arg in "$@"; do
    case $arg in
        --gameboy)
            BUILD_NORMAL="no"
            BUILD_GAMEBOY="yes"
            ;;
        --both)
            BUILD_NORMAL="yes"
            BUILD_GAMEBOY="yes"
            ;;
        --normal)
            BUILD_NORMAL="yes"
            BUILD_GAMEBOY="no"
            ;;
        --tools)
            BUILD_NORMAL="no"
            BUILD_GAMEBOY="no"
            BUILD_TOOLS="yes"
            ;;
        --help|-h)
            echo "gb-os Build Script"
            echo ""
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --gameboy     Build GameBoy edition only (default)"
            echo "  --normal      Build normal edition only"
            echo "  --both        Build both normal and GameBoy editions"
            echo "  --tools       Build mkgamedisk tool only"
            echo "  --help, -h    Show this help message"
            exit 0
            ;;
    esac
done

echo "========================================"
echo "  gb-os Build System"
echo "  No-Emulation Boot + UEFI Prep"
echo "========================================"
echo ""

mkdir -p build

# ============================================================================
# Build Bootloader
# ============================================================================

echo "[1/5] Assembling bootloader..."
nasm -f bin -o build/boot.bin boot/boot.asm
BOOT_SIZE=$(stat -c%s build/boot.bin 2>/dev/null || stat -f%z build/boot.bin)
echo "      boot.bin: $BOOT_SIZE bytes"

if [ "$BUILD_NORMAL" = "yes" ]; then
    echo "      Building stage2.bin (normal mode)..."
    nasm -f bin -o build/stage2.bin boot/stage2.asm
    STAGE2_SIZE=$(stat -c%s build/stage2.bin 2>/dev/null || stat -f%z build/stage2.bin)
    echo "      stage2.bin: $STAGE2_SIZE bytes"
fi

if [ "$BUILD_GAMEBOY" = "yes" ]; then
    echo "      Building stage2-gameboy.bin (GameBoy mode)..."
    nasm -f bin -DGAMEBOY_MODE -o build/stage2-gameboy.bin boot/stage2.asm
    STAGE2_GB_SIZE=$(stat -c%s build/stage2-gameboy.bin 2>/dev/null || stat -f%z build/stage2-gameboy.bin)
    echo "      stage2-gameboy.bin: $STAGE2_GB_SIZE bytes"
fi
echo ""

# ============================================================================
# Build Kernel
# ============================================================================

echo "[2/5] Building kernel..."
cd kernel

if cargo +nightly build --release --target ../i686-rustacean.json \
    -Zbuild-std=core,alloc \
    -Zbuild-std-features=compiler-builtins-mem 2>&1; then
    echo "      Kernel build successful!"
else
    echo ""
    echo "      ERROR: Kernel build failed!"
    exit 1
fi

cd ..

# Find and convert kernel binary
KERNEL_BIN=""
for name in gb-os gb_os rustacean-kernel rustacean_kernel; do
    if [ -f "kernel/target/i686-rustacean/release/$name" ]; then
        KERNEL_BIN="kernel/target/i686-rustacean/release/$name"
        break
    fi
done

# If not found, try to find any ELF binary
if [ -z "$KERNEL_BIN" ]; then
    KERNEL_BIN=$(find kernel/target/i686-rustacean/release -maxdepth 1 -type f -executable ! -name "*.d" ! -name "*.rlib" 2>/dev/null | head -1)
fi

if [ -n "$KERNEL_BIN" ]; then
    echo "      Found kernel binary: $KERNEL_BIN"
    echo "      Converting ELF to flat binary..."
    objcopy -O binary "$KERNEL_BIN" build/kernel.bin
    KERNEL_SIZE=$(stat -c%s build/kernel.bin 2>/dev/null || stat -f%z build/kernel.bin)
    echo "      kernel.bin: $KERNEL_SIZE bytes"
else
    echo "      ERROR: Kernel binary not found!"
    exit 1
fi
echo ""

# ============================================================================
# Build Tools (mkgamedisk)
# ============================================================================

if [ "$BUILD_GAMEBOY" = "yes" ] || [ "$BUILD_TOOLS" = "yes" ]; then
    echo "[3/5] Building tools..."
    if [ -d "tools/mkgamedisk" ]; then
        cd tools/mkgamedisk
        cargo build --release 2>&1
        cp target/release/mkgamedisk ../../build/ 2>/dev/null || true
        cd ../..
        echo "      mkgamedisk built"
    else
        echo "      WARNING: tools/mkgamedisk not found, skipping"
    fi
    echo ""
else
    echo "[3/5] Skipping tools"
    echo ""
fi

# ============================================================================
# Create Disk Images
# ============================================================================

echo "[4/5] Creating disk images..."

# Function to create floppy image
create_floppy_image() {
    local IMG_NAME="$1"
    local STAGE2_BIN="$2"

    echo "      Creating $IMG_NAME (floppy 1.44MB)..."

    # Create 1.44MB floppy image
    dd if=/dev/zero of="build/$IMG_NAME" bs=512 count=2880 2>/dev/null

    # Write boot sector (sector 0)
    dd if=build/boot.bin of="build/$IMG_NAME" bs=512 count=1 conv=notrunc 2>/dev/null

    # Write stage2 (sectors 1-32, 16KB)
    dd if="$STAGE2_BIN" of="build/$IMG_NAME" bs=512 seek=1 conv=notrunc 2>/dev/null

    # Write kernel (sectors 33+, up to 256KB)
    dd if=build/kernel.bin of="build/$IMG_NAME" bs=512 seek=33 conv=notrunc 2>/dev/null

    local SIZE=$(stat -c%s "build/$IMG_NAME" 2>/dev/null || stat -f%z "build/$IMG_NAME")
    echo "      $IMG_NAME: $SIZE bytes"
}

# Function to create ISO with no-emulation boot
create_noemu_iso() {
    local IMG_NAME="$1"
    local ISO_NAME="$2"
    local VOLUME_ID="$3"

    echo "      Creating $ISO_NAME (no-emulation El Torito)..."

    # Create ISO directory structure
    mkdir -p build/iso/boot

    # Create the boot image for El Torito
    # This is a flat binary containing: boot.bin + stage2 + kernel
    # Padded to align to 2048-byte CD sector boundaries

    # Boot sector (512 bytes, padded to 2048)
    dd if=build/boot.bin of=build/iso/boot/boot.img bs=2048 count=1 conv=sync 2>/dev/null

    # Stage2 (16KB = 8 CD sectors)
    if [ -f "build/stage2-gameboy.bin" ] && [[ "$IMG_NAME" == *"gameboy"* ]]; then
        dd if=build/stage2-gameboy.bin of=build/iso/boot/boot.img bs=2048 seek=1 conv=notrunc 2>/dev/null
    else
        dd if=build/stage2.bin of=build/iso/boot/boot.img bs=2048 seek=1 conv=notrunc 2>/dev/null
    fi

    # Kernel (aligned to 2048-byte boundary, starting at CD sector 9)
    dd if=build/kernel.bin of=build/iso/boot/boot.img bs=2048 seek=9 conv=notrunc 2>/dev/null

    # Copy floppy image for fallback/installation
    cp "build/$IMG_NAME" build/iso/

    # Determine boot image size in CD sectors (2048 bytes each)
    # boot.img needs to cover: boot(1) + stage2(8) + kernel(128) = ~137 sectors minimum
    # We'll use the actual file size
    local BOOT_IMG_SIZE=$(stat -c%s build/iso/boot/boot.img 2>/dev/null || stat -f%z build/iso/boot/boot.img)
    local BOOT_SECTORS=$(( (BOOT_IMG_SIZE + 2047) / 2048 ))

    # Create ISO with xorriso (preferred) or genisoimage
    # Using no-emulation boot mode (-no-emul-boot)
    # boot-load-size specifies sectors to load (we load enough for stage1+stage2)
    if command -v xorriso &> /dev/null; then
        xorriso -as mkisofs \
            -o "build/$ISO_NAME" \
            -V "$VOLUME_ID" \
            -b boot/boot.img \
            -no-emul-boot \
            -boot-load-size 4 \
            -boot-info-table \
            -R -J \
            build/iso/ 2>/dev/null
    elif command -v genisoimage &> /dev/null; then
        genisoimage \
            -o "build/$ISO_NAME" \
            -V "$VOLUME_ID" \
            -b boot/boot.img \
            -no-emul-boot \
            -boot-load-size 4 \
            -boot-info-table \
            -R -J \
            build/iso/ 2>/dev/null
    else
        echo "      ERROR: Neither xorriso nor genisoimage found!"
        return 1
    fi

    local ISO_SIZE=$(stat -c%s "build/$ISO_NAME" 2>/dev/null || stat -f%z "build/$ISO_NAME")
    echo "      $ISO_NAME: $ISO_SIZE bytes (no-emul boot)"

    # Clean up ISO directory
    rm -rf build/iso
}

# Function to embed ROM into images
embed_rom() {
    local IMG_NAME="$1"
    local ROM_FILE="$2"

    if [ -z "$ROM_FILE" ] || [ ! -f "$ROM_FILE" ]; then
        echo "      No ROM file to embed"
        return 0
    fi

    echo "      Embedding ROM: $ROM_FILE"

    local ROM_SIZE=$(stat -c%s "$ROM_FILE" 2>/dev/null || stat -f%z "$ROM_FILE")
    local ROM_TITLE=$(dd if="$ROM_FILE" bs=1 skip=308 count=16 2>/dev/null | tr -d '\0' | tr -cd '[:alnum:] ')
    [ -z "$ROM_TITLE" ] && ROM_TITLE="UNKNOWN"

    echo "      ROM Title: $ROM_TITLE"
    echo "      ROM Size: $ROM_SIZE bytes"

    # Create ROM header (512 bytes)
    printf 'GBOY' > build/rom_header.bin
    printf "$(printf '\\x%02x\\x%02x\\x%02x\\x%02x' \
        $((ROM_SIZE & 0xFF)) \
        $(((ROM_SIZE >> 8) & 0xFF)) \
        $(((ROM_SIZE >> 16) & 0xFF)) \
        $(((ROM_SIZE >> 24) & 0xFF)))" >> build/rom_header.bin
    printf "%-32s" "$ROM_TITLE" | head -c 32 >> build/rom_header.bin
    dd if=/dev/zero bs=1 count=$((512 - 40)) >> build/rom_header.bin 2>/dev/null

    # Write ROM header at sector 289 (floppy image)
    dd if=build/rom_header.bin of="build/$IMG_NAME" bs=512 seek=289 conv=notrunc 2>/dev/null

    # Write ROM data starting at sector 290
    dd if="$ROM_FILE" of="build/$IMG_NAME" bs=512 seek=290 conv=notrunc 2>/dev/null

    echo "      ROM embedded at sectors 289+"
}

# Build Normal Edition
if [ "$BUILD_NORMAL" = "yes" ]; then
    create_floppy_image "rustacean.img" "build/stage2.bin"
    create_noemu_iso "rustacean.img" "rustacean.iso" "RUSTACEAN_OS"
fi

# Build GameBoy Edition
if [ "$BUILD_GAMEBOY" = "yes" ]; then
    create_floppy_image "gameboy-system.img" "build/stage2-gameboy.bin"

    # Embed ROM if provided
    ROM_FILE="${ROM_FILE:-}"
    if [ -z "$ROM_FILE" ] && [ -f "/input/game.gb" ]; then
        ROM_FILE="/input/game.gb"
    fi

    if [ -n "$ROM_FILE" ] && [ -f "$ROM_FILE" ]; then
        embed_rom "gameboy-system.img" "$ROM_FILE"
    else
        echo "      No ROM file specified (use ROM_FILE env var or mount to /input/game.gb)"
    fi

    create_noemu_iso "gameboy-system.img" "gameboy-system.iso" "GAMEBOY_OS"
fi

echo ""

# ============================================================================
# Copy to Output
# ============================================================================

echo "[5/5] Copying to output directory..."

# Determine output directory
if [ -d "/output" ]; then
    OUTPUT_DIR="/output"
else
    OUTPUT_DIR="./output"
    mkdir -p "$OUTPUT_DIR"
fi

if [ "$BUILD_NORMAL" = "yes" ]; then
    cp build/rustacean.img "$OUTPUT_DIR/"
    cp build/rustacean.iso "$OUTPUT_DIR/"
    echo "      $OUTPUT_DIR/rustacean.img"
    echo "      $OUTPUT_DIR/rustacean.iso"
fi

if [ "$BUILD_GAMEBOY" = "yes" ]; then
    cp build/gameboy-system.img "$OUTPUT_DIR/"
    cp build/gameboy-system.iso "$OUTPUT_DIR/"
    echo "      $OUTPUT_DIR/gameboy-system.img"
    echo "      $OUTPUT_DIR/gameboy-system.iso"

    if [ -f "build/mkgamedisk" ]; then
        cp build/mkgamedisk "$OUTPUT_DIR/"
        echo "      $OUTPUT_DIR/mkgamedisk"
    fi
fi

# Always copy kernel for debugging
cp build/kernel.bin "$OUTPUT_DIR/"
echo "      $OUTPUT_DIR/kernel.bin"

echo ""
echo "========================================"
echo "  Build Complete!"
echo "========================================"

if [ "$BUILD_NORMAL" = "yes" ]; then
    echo ""
    echo "  Normal Mode:"
    echo "    qemu-system-i386 -fda $OUTPUT_DIR/rustacean.img -boot a -m 256M"
    echo "    qemu-system-i386 -cdrom $OUTPUT_DIR/rustacean.iso -boot d -m 256M"
fi

if [ "$BUILD_GAMEBOY" = "yes" ]; then
    echo ""
    echo "  GameBoy Mode:"
    echo "    qemu-system-i386 -fda $OUTPUT_DIR/gameboy-system.img -boot a -m 256M"
    echo "    qemu-system-i386 -cdrom $OUTPUT_DIR/gameboy-system.iso -boot d -m 256M"
    echo ""
    echo "  USB/HDD Installation:"
    echo "    sudo dd if=$OUTPUT_DIR/gameboy-system.img of=/dev/sdX bs=512"
fi

echo ""
