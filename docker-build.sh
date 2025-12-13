#!/bin/bash
# ============================================================================
# RetroFutureGB Build Script (Universal Boot - No Floppy Emulation)
# ============================================================================
#
# Builds the kernel and bootloader, creates bootable disk images.
# Supports LBA-based booting without floppy size restrictions.
#
# Usage:
#   ./build.sh                    # Build GameBoy mode (default)
#   ./build.sh --gameboy          # Build GameBoy mode
#   ./build.sh --normal           # Build normal mode
#   ./build.sh --both             # Build both modes
#
# Output:
#   /output/gameboy-system.img    # Raw disk image (2.88MB base, expandable)
#   /output/gameboy-system.iso    # El Torito bootable CD (no floppy emulation)
#   /output/boot.bin              # Stage 1 bootloader
#   /output/stage2-gameboy.bin    # Stage 2 bootloader (GameBoy mode)
#   /output/kernel.bin            # Kernel binary
#   /output/vbr.bin               # Volume Boot Record (for installer)
# ============================================================================

set -e

# Parse arguments
BUILD_NORMAL="no"
BUILD_GAMEBOY="yes"
BUILD_TOOLS="yes"

for arg in "$@"; do
    case $arg in
        --normal)
            BUILD_NORMAL="yes"
            BUILD_GAMEBOY="no"
            ;;
        --gameboy)
            BUILD_GAMEBOY="yes"
            BUILD_NORMAL="no"
            ;;
        --both)
            BUILD_NORMAL="yes"
            BUILD_GAMEBOY="yes"
            ;;
        --no-tools)
            BUILD_TOOLS="no"
            ;;
    esac
done

echo "========================================"
echo "  RetroFutureGB Build System"
echo "  (Universal Boot - No Floppy Emulation)"
echo "========================================"
echo ""
echo "Build configuration:"
echo "  Normal mode:  $BUILD_NORMAL"
echo "  GameBoy mode: $BUILD_GAMEBOY"
echo "  Tools:        $BUILD_TOOLS"
echo ""

# Create build directory
mkdir -p build
mkdir -p /output

# ============================================================================
# Build Bootloaders
# ============================================================================

echo "[1/5] Building bootloaders..."

# Stage 1 bootloader (universal)
nasm -f bin -o build/boot.bin boot/boot.asm
echo "      boot.bin: $(stat -c%s build/boot.bin) bytes"

# VBR (for installer to use)
nasm -f bin -o build/vbr.bin boot/vbr.asm
echo "      vbr.bin: $(stat -c%s build/vbr.bin) bytes"

if [ "$BUILD_NORMAL" = "yes" ]; then
    nasm -f bin -o build/stage2.bin boot/stage2.asm
    echo "      stage2.bin: $(stat -c%s build/stage2.bin) bytes"
fi

if [ "$BUILD_GAMEBOY" = "yes" ]; then
    # GameBoy mode uses the same stage2 now (universal)
    nasm -f bin -DGAMEBOY_MODE -o build/stage2-gameboy.bin boot/stage2.asm
    echo "      stage2-gameboy.bin: $(stat -c%s build/stage2-gameboy.bin) bytes"
fi
echo ""

# ============================================================================
# Build Kernel
# ============================================================================

echo "[2/5] Building kernel..."
cd kernel

if cargo +nightly build --release \
    --target ../i686-rustacean.json \
    -Zbuild-std=core,alloc \
    -Zbuild-std-features=compiler-builtins-mem 2>&1; then
    echo "      Kernel build successful"
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
        cargo build --release 2>&1 | tail -1
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

# Calculate sizes
BOOT_SIZE=512
STAGE2_SIZE=$(stat -c%s build/stage2-gameboy.bin 2>/dev/null || echo 16384)
KERNEL_SIZE=$(stat -c%s build/kernel.bin)

# Sector layout:
#   0:        Boot sector
#   1-32:     Stage 2 (16KB = 32 sectors)
#   33-64:    Reserved
#   65-320:   Kernel (128KB max = 256 sectors)
#   321:      ROM header
#   322+:     ROM data

BOOT_SECTOR=0
STAGE2_SECTOR=1
STAGE2_SECTORS=32
KERNEL_SECTOR=65
KERNEL_SECTORS=256
ROM_HEADER_SECTOR=321
ROM_DATA_SECTOR=322

# Base image size: 2.88MB (enough for system without ROM)
# This can be extended for ROMs
BASE_SECTORS=5760  # 2.88MB

if [ "$BUILD_NORMAL" = "yes" ]; then
    echo "      Creating rustacean.img..."
    dd if=/dev/zero of=build/rustacean.img bs=512 count=$BASE_SECTORS 2>/dev/null
    dd if=build/boot.bin of=build/rustacean.img bs=512 seek=$BOOT_SECTOR conv=notrunc 2>/dev/null
    dd if=build/stage2.bin of=build/rustacean.img bs=512 seek=$STAGE2_SECTOR conv=notrunc 2>/dev/null
    dd if=build/kernel.bin of=build/rustacean.img bs=512 seek=$KERNEL_SECTOR conv=notrunc 2>/dev/null
    echo "      rustacean.img: $(stat -c%s build/rustacean.img) bytes"

    echo "      Creating rustacean.iso (El Torito no-emulation)..."
    mkdir -p build/iso
    cp build/rustacean.img build/iso/

    # Create El Torito bootable ISO without floppy emulation
    genisoimage -o build/rustacean.iso \
        -b rustacean.img \
        -no-emul-boot \
        -boot-load-size 4 \
        -boot-info-table \
        -V "RUSTACEAN_OS" \
        build/iso/ 2>/dev/null || \
    xorriso -as mkisofs -o build/rustacean.iso \
        -b rustacean.img \
        -no-emul-boot \
        -boot-load-size 4 \
        -boot-info-table \
        -V "RUSTACEAN_OS" \
        build/iso/ 2>/dev/null

    echo "      rustacean.iso: $(stat -c%s build/rustacean.iso) bytes"
    rm -rf build/iso
fi

if [ "$BUILD_GAMEBOY" = "yes" ]; then
    echo "      Creating gameboy-system.img..."

    # Start with base image
    dd if=/dev/zero of=build/gameboy-system.img bs=512 count=$BASE_SECTORS 2>/dev/null
    dd if=build/boot.bin of=build/gameboy-system.img bs=512 seek=$BOOT_SECTOR conv=notrunc 2>/dev/null
    dd if=build/stage2-gameboy.bin of=build/gameboy-system.img bs=512 seek=$STAGE2_SECTOR conv=notrunc 2>/dev/null
    dd if=build/kernel.bin of=build/gameboy-system.img bs=512 seek=$KERNEL_SECTOR conv=notrunc 2>/dev/null

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

        # Calculate how many sectors the ROM needs
        ROM_SECTORS=$(( (ROM_SIZE + 511) / 512 ))
        TOTAL_SECTORS=$(( ROM_DATA_SECTOR + ROM_SECTORS ))

        # Expand image if needed
        if [ $TOTAL_SECTORS -gt $BASE_SECTORS ]; then
            echo "      Expanding image to $TOTAL_SECTORS sectors for ROM..."
            dd if=/dev/zero of=build/gameboy-system.img bs=512 count=$TOTAL_SECTORS 2>/dev/null
            # Re-write system components
            dd if=build/boot.bin of=build/gameboy-system.img bs=512 seek=$BOOT_SECTOR conv=notrunc 2>/dev/null
            dd if=build/stage2-gameboy.bin of=build/gameboy-system.img bs=512 seek=$STAGE2_SECTOR conv=notrunc 2>/dev/null
            dd if=build/kernel.bin of=build/gameboy-system.img bs=512 seek=$KERNEL_SECTOR conv=notrunc 2>/dev/null
        fi

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

        # Write ROM header at ROM_HEADER_SECTOR
        dd if=build/rom_header.bin of=build/gameboy-system.img bs=512 seek=$ROM_HEADER_SECTOR conv=notrunc 2>/dev/null

        # Write ROM data starting at ROM_DATA_SECTOR
        dd if="$ROM_FILE" of=build/gameboy-system.img bs=512 seek=$ROM_DATA_SECTOR conv=notrunc 2>/dev/null

        echo "      ROM embedded at sectors $ROM_HEADER_SECTOR+"
    else
        echo "      No ROM file specified (use ROM_FILE env var or mount to /input/game.gb)"
        echo "      Creating base image without embedded ROM"
    fi

    echo "      gameboy-system.img: $(stat -c%s build/gameboy-system.img) bytes"

    # Create El Torito bootable ISO with NO floppy emulation
    # This allows larger images for big ROMs
    echo "      Creating gameboy-system.iso (El Torito no-emulation)..."
    mkdir -p build/iso
    cp build/gameboy-system.img build/iso/

    genisoimage -o build/gameboy-system.iso \
        -b gameboy-system.img \
        -no-emul-boot \
        -boot-load-size 4 \
        -boot-info-table \
        -V "RETROGB" \
        build/iso/ 2>/dev/null || \
    xorriso -as mkisofs -o build/gameboy-system.iso \
        -b gameboy-system.img \
        -no-emul-boot \
        -boot-load-size 4 \
        -boot-info-table \
        -V "RETROGB" \
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

# Always copy bootloader and kernel components for debugging/installer payload
cp build/boot.bin /output/
cp build/vbr.bin /output/
cp build/kernel.bin /output/
echo "      /output/boot.bin"
echo "      /output/vbr.bin"
echo "      /output/kernel.bin"

echo ""
echo "========================================"
echo "  Build Complete!"
echo "========================================"

if [ "$BUILD_NORMAL" = "yes" ]; then
    echo ""
    echo "  Normal Mode:"
    echo "    qemu-system-i386 -drive file=output/rustacean.img,format=raw -boot c -m 256M"
    echo "    qemu-system-i386 -cdrom output/rustacean.iso -boot d -m 256M"
fi

if [ "$BUILD_GAMEBOY" = "yes" ]; then
    echo ""
    echo "  GameBoy Mode (without ROM):"
    echo "    qemu-system-i386 -drive file=output/gameboy-system.img,format=raw -boot c -m 256M"
    echo "    qemu-system-i386 -cdrom output/gameboy-system.iso -boot d -m 256M"
    echo ""
    echo "  To build with embedded ROM:"
    echo "    ROM_FILE=/path/to/game.gb ./build.sh --gameboy"
    echo ""
    echo "  Disk Layout:"
    echo "    Sector 0:       Boot sector"
    echo "    Sector 1-32:    Stage 2 bootloader (16KB)"
    echo "    Sector 65-320:  Kernel (128KB)"
    echo "    Sector 321:     ROM header"
    echo "    Sector 322+:    ROM data"
fi

echo ""
echo "  Components (for installer payload):"
echo "    boot.bin              - Stage 1 bootloader"
echo "    vbr.bin               - Volume Boot Record"
if [ "$BUILD_NORMAL" = "yes" ]; then
    echo "    stage2.bin            - Stage 2 bootloader (normal mode)"
fi
if [ "$BUILD_GAMEBOY" = "yes" ]; then
    echo "    stage2-gameboy.bin    - Stage 2 bootloader (GameBoy mode)"
fi
echo "    kernel.bin            - Kernel binary"
echo ""
