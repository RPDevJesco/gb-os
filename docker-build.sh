#!/bin/bash
#
# Rustacean OS Docker Build Script (with GameBoy Mode)
#
# Usage:
#   /build.sh                 # Build normal Rustacean OS
#   /build.sh --gameboy       # Build GameBoy edition
#   /build.sh --both          # Build both editions
#   /build.sh --tools         # Build mkgamedisk tool only
#

set -e

BUILD_NORMAL="yes"
BUILD_GAMEBOY="no"
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
        --tools)
            BUILD_NORMAL="no"
            BUILD_TOOLS="yes"
            ;;
        --help|-h)
            echo "Rustacean OS Build Script"
            echo ""
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --gameboy     Build GameBoy edition only"
            echo "  --both        Build both normal and GameBoy editions"
            echo "  --tools       Build mkgamedisk tool only"
            echo "  --help, -h    Show this help message"
            exit 0
            ;;
    esac
done

echo "========================================"
echo "  Rustacean OS Build System"
echo "========================================"
echo ""

mkdir -p build

# ============================================================================
# Build Bootloader
# ============================================================================

echo "[1/5] Assembling bootloader..."
nasm -f bin -o build/boot.bin boot/boot.asm
echo "      boot.bin: $(stat -c%s build/boot.bin) bytes"

if [ "$BUILD_NORMAL" = "yes" ]; then
    echo "      Building stage2.bin (normal mode)..."
    nasm -f bin -o build/stage2.bin boot/stage2.asm
    echo "      stage2.bin: $(stat -c%s build/stage2.bin) bytes"
fi

if [ "$BUILD_GAMEBOY" = "yes" ]; then
    echo "      Building stage2-gameboy.bin (GameBoy mode, NO VESA)..."
    nasm -f bin -DGAMEBOY_MODE -DSKIP_VESA -o build/stage2-gameboy.bin boot/stage2.asm
    echo "      stage2-gameboy.bin: $(stat -c%s build/stage2-gameboy.bin) bytes"
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
if [ -f "kernel/target/i686-rustacean/release/rustacean-kernel" ]; then
    KERNEL_BIN="kernel/target/i686-rustacean/release/rustacean-kernel"
elif [ -f "kernel/target/i686-rustacean/release/rustacean_kernel" ]; then
    KERNEL_BIN="kernel/target/i686-rustacean/release/rustacean_kernel"
fi

if [ -n "$KERNEL_BIN" ]; then
    echo "      Converting ELF to flat binary..."
    objcopy -O binary "$KERNEL_BIN" build/kernel.bin
    echo "      kernel.bin: $(stat -c%s build/kernel.bin) bytes"
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
        cargo build --release
        cp target/release/mkgamedisk ../../build/
        cd ../..
        echo "      mkgamedisk built"
    else
        echo "      WARNING: tools/mkgamedisk not found, skipping"
    fi
    echo ""
else
    echo "[3/5] Skipping tools (not needed for normal build)"
    echo ""
fi

# ============================================================================
# Create Disk Images
# ============================================================================

echo "[4/5] Creating disk images..."

if [ "$BUILD_NORMAL" = "yes" ]; then
    echo "      Creating rustacean.img (floppy)..."
    dd if=/dev/zero of=build/rustacean.img bs=512 count=2880 2>/dev/null
    dd if=build/boot.bin of=build/rustacean.img bs=512 count=1 conv=notrunc 2>/dev/null
    dd if=build/stage2.bin of=build/rustacean.img bs=512 seek=1 conv=notrunc 2>/dev/null
    dd if=build/kernel.bin of=build/rustacean.img bs=512 seek=33 conv=notrunc 2>/dev/null
    echo "      rustacean.img: $(stat -c%s build/rustacean.img) bytes"

    echo "      Creating rustacean.iso (floppy emulation)..."
    mkdir -p build/iso
    cp build/rustacean.img build/iso/
    genisoimage -o build/rustacean.iso \
        -b rustacean.img \
        -V "RUSTACEAN_OS" \
        build/iso/ 2>/dev/null || \
    xorriso -as mkisofs -o build/rustacean.iso \
        -b rustacean.img \
        -V "RUSTACEAN_OS" \
        build/iso/ 2>/dev/null
    echo "      rustacean.iso: $(stat -c%s build/rustacean.iso) bytes"
    rm -rf build/iso
fi

if [ "$BUILD_GAMEBOY" = "yes" ]; then
    echo "      Creating gameboy-system.img (floppy)..."
    dd if=/dev/zero of=build/gameboy-system.img bs=512 count=2880 2>/dev/null
    dd if=build/boot.bin of=build/gameboy-system.img bs=512 count=1 conv=notrunc 2>/dev/null
    dd if=build/stage2-gameboy.bin of=build/gameboy-system.img bs=512 seek=1 conv=notrunc 2>/dev/null
    dd if=build/kernel.bin of=build/gameboy-system.img bs=512 seek=33 conv=notrunc 2>/dev/null

    # Embed ROM if provided via ROM_FILE environment variable or /input/game.gb
    ROM_FILE="${ROM_FILE:-}"
    if [ -z "$ROM_FILE" ] && [ -f "/input/game.gb" ]; then
        ROM_FILE="/input/game.gb"
    fi

    if [ -n "$ROM_FILE" ] && [ -f "$ROM_FILE" ]; then
        echo "      Embedding ROM: $ROM_FILE"
        ROM_SIZE=$(stat -c%s "$ROM_FILE")

        # Extract title from ROM header (bytes 0x134-0x143)
        ROM_TITLE=$(dd if="$ROM_FILE" bs=1 skip=308 count=16 2>/dev/null | tr -d '\0' | tr -cd '[:alnum:] ')
        [ -z "$ROM_TITLE" ] && ROM_TITLE="UNKNOWN"
        echo "      ROM Title: $ROM_TITLE"
        echo "      ROM Size: $ROM_SIZE bytes"

        # Create ROM header (512 bytes)
        # Format: 'GBOY' (4) + size (4) + title (32) + padding
        printf 'GBOY' > build/rom_header.bin
        printf "$(printf '\\x%02x\\x%02x\\x%02x\\x%02x' \
            $((ROM_SIZE & 0xFF)) \
            $(((ROM_SIZE >> 8) & 0xFF)) \
            $(((ROM_SIZE >> 16) & 0xFF)) \
            $(((ROM_SIZE >> 24) & 0xFF)))" >> build/rom_header.bin
        printf "%-32s" "$ROM_TITLE" | head -c 32 >> build/rom_header.bin
        # Pad to 512 bytes
        dd if=/dev/zero bs=1 count=$((512 - 40)) >> build/rom_header.bin 2>/dev/null

        # Write ROM header at sector 289
        dd if=build/rom_header.bin of=build/gameboy-system.img bs=512 seek=289 conv=notrunc 2>/dev/null

        # Write ROM data starting at sector 290
        dd if="$ROM_FILE" of=build/gameboy-system.img bs=512 seek=290 conv=notrunc 2>/dev/null

        echo "      ROM embedded at sectors 289+"
    else
        echo "      No ROM file specified (use ROM_FILE env var or mount to /input/game.gb)"
        echo "      Creating base image without embedded ROM"
    fi

    echo "      gameboy-system.img: $(stat -c%s build/gameboy-system.img) bytes"

    echo "      Creating gameboy-system.iso (floppy emulation)..."
    mkdir -p build/iso
    cp build/gameboy-system.img build/iso/
    genisoimage -o build/gameboy-system.iso \
        -b gameboy-system.img \
        -V "GAMEBOY_OS" \
        build/iso/ 2>/dev/null || \
    xorriso -as mkisofs -o build/gameboy-system.iso \
        -b gameboy-system.img \
        -V "GAMEBOY_OS" \
        build/iso/ 2>/dev/null
    echo "      gameboy-system.iso: $(stat -c%s build/gameboy-system.iso) bytes"
    rm -rf build/iso
fi
echo ""

# ============================================================================
# Copy to Output
# ============================================================================

echo "[5/5] Copying to output directory..."

if [ "$BUILD_NORMAL" = "yes" ]; then
    cp build/rustacean.img /output/
    cp build/rustacean.iso /output/
    cp build/stage2.bin /output/
    echo "      /output/rustacean.img"
    echo "      /output/rustacean.iso"
    echo "      /output/stage2.bin"
fi

if [ "$BUILD_GAMEBOY" = "yes" ]; then
    cp build/gameboy-system.img /output/
    cp build/gameboy-system.iso /output/
    cp build/stage2-gameboy.bin /output/
    echo "      /output/gameboy-system.img"
    echo "      /output/gameboy-system.iso"
    echo "      /output/stage2-gameboy.bin"

    if [ -f "build/mkgamedisk" ]; then
        cp build/mkgamedisk /output/
        echo "      /output/mkgamedisk"
    fi
fi

# Always copy bootloader and kernel components for debugging/reuse
cp build/boot.bin /output/
cp build/kernel.bin /output/
echo "      /output/boot.bin"
echo "      /output/kernel.bin"

echo ""
echo "========================================"
echo "  Build Complete!"
echo "========================================"

if [ "$BUILD_NORMAL" = "yes" ]; then
    echo ""
    echo "  Normal Mode:"
    echo "    qemu-system-i386 -fda output/rustacean.img -boot a -m 256M"
    echo "    qemu-system-i386 -cdrom output/rustacean.iso -boot d -m 256M"
fi

if [ "$BUILD_GAMEBOY" = "yes" ]; then
    echo ""
    echo "  GameBoy Mode:"
    echo "    1. Create game floppy: ./output/mkgamedisk game.gb output/game.img"
    echo "    2. Run: qemu-system-i386 -fda output/gameboy-system.img -boot a -m 256M"
    echo "       Or:  qemu-system-i386 -cdrom output/gameboy-system.iso -boot d -m 256M"
    echo "    3. When prompted, swap to game floppy in QEMU monitor"
fi

echo ""
echo "  Components (for custom builds):"
echo "    boot.bin        - Stage 1 bootloader (512 bytes)"
if [ "$BUILD_NORMAL" = "yes" ]; then
    echo "    stage2.bin      - Stage 2 bootloader (normal mode)"
fi
if [ "$BUILD_GAMEBOY" = "yes" ]; then
    echo "    stage2-gameboy.bin - Stage 2 bootloader (GameBoy mode)"
fi
echo "    kernel.bin      - Kernel binary"
echo ""
