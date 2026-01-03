#!/bin/bash
# rustboot Docker build script
# Builds all platform targets and organizes outputs

# Don't use set -e, we handle errors manually
# set -e

echo "========================================"
echo " rustboot - Multi-Platform Bootloader"
echo " Build Script"
echo "========================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Build directory
BUILD_DIR="/build"
OUTPUT_DIR="/build/output"

# Target definitions: name|rust_target|binary_name|output_name
TARGETS=(
    "pi-zero2|aarch64-unknown-none|kernel8|kernel8.img"
    "pi5|aarch64-unknown-none|kernel8|kernel8.img"
    "kickpi-k2b|aarch64-unknown-none|boot0|boot0.bin"
    "rp2040|thumbv6m-none-eabi|bootloader|bootloader.bin"
)

# Create output directory structure
create_output_dirs() {
    echo -e "${YELLOW}Creating output directories...${NC}"
    rm -rf "$OUTPUT_DIR"
    mkdir -p "$OUTPUT_DIR"
    
    for target_info in "${TARGETS[@]}"; do
        IFS='|' read -r name rust_target bin_name out_name <<< "$target_info"
        mkdir -p "$OUTPUT_DIR/$name"
    done
    
    echo -e "${GREEN}Output directories created.${NC}"
    echo ""
}

# Build a single target
build_target() {
    local name="$1"
    local rust_target="$2"
    local bin_name="$3"
    local out_name="$4"
    
    echo -e "${YELLOW}Building $name for $rust_target...${NC}"
    
    local platform_dir="$BUILD_DIR/platform/$name"
    local linker_script="$platform_dir/linker.ld"
    
    # Check linker script exists
    if [ ! -f "$linker_script" ]; then
        echo -e "${RED}  ERROR: Linker script not found: $linker_script${NC}"
        return 1
    fi
    
    # Build with cargo
    if ! RUSTFLAGS="-C link-arg=-T$linker_script" \
        cargo build \
        --release \
        --package "rustboot-$name" \
        --target "$rust_target" \
        2>&1; then
        echo -e "${RED}  ERROR: Cargo build failed for $name${NC}"
        return 1
    fi
    
    local elf_path="$BUILD_DIR/target/$rust_target/release/$bin_name"
    local bin_path="$BUILD_DIR/target/$rust_target/release/${bin_name}.bin"
    local target_output_dir="$OUTPUT_DIR/$name"
    
    # Check if build succeeded
    if [ ! -f "$elf_path" ]; then
        echo -e "${RED}  ERROR: Build failed for $name - ELF not found at $elf_path${NC}"
        return 1
    fi
    
    # Convert ELF to binary
    echo "  Converting ELF to binary..."
    if ! rust-objcopy -O binary "$elf_path" "$bin_path"; then
        echo -e "${RED}  ERROR: objcopy failed for $name${NC}"
        return 1
    fi
    
    # Copy to output directory
    cp "$bin_path" "$target_output_dir/$out_name"
    cp "$elf_path" "$target_output_dir/${bin_name}.elf"
    
    # Copy platform-specific files
    if [ -f "$platform_dir/config.txt" ]; then
        cp "$platform_dir/config.txt" "$target_output_dir/"
    fi
    
    # Generate size info
    local size=$(stat -c%s "$target_output_dir/$out_name")
    local size_kb=$(awk "BEGIN {printf \"%.2f\", $size / 1024}")
    
    echo -e "${GREEN}  Built: $out_name ($size bytes / ${size_kb} KB)${NC}"
    
    # Create info file
    cat > "$target_output_dir/BUILD_INFO.txt" << BUILDEOF
rustboot - $name
================

Binary: $out_name
Size: $size bytes (${size_kb} KB)
Target: $rust_target
Built: $(date -u '+%Y-%m-%d %H:%M:%S UTC')

Files included:
- $out_name       : Raw binary for flashing
- ${bin_name}.elf : ELF with debug symbols
BUILDEOF

    if [ -f "$target_output_dir/config.txt" ]; then
        echo "- config.txt      : Boot configuration" >> "$target_output_dir/BUILD_INFO.txt"
    fi
    
    return 0
}

# Build all targets
build_all() {
    local success=0
    local failed=0
    
    for target_info in "${TARGETS[@]}"; do
        IFS='|' read -r name rust_target bin_name out_name <<< "$target_info"
        
        echo ""
        if build_target "$name" "$rust_target" "$bin_name" "$out_name"; then
            ((success++))
        else
            ((failed++))
        fi
    done
    
    echo ""
    echo "========================================"
    echo " Build Summary"
    echo "========================================"
    echo -e "${GREEN}Successful: $success${NC}"
    if [ $failed -gt 0 ]; then
        echo -e "${RED}Failed: $failed${NC}"
    fi
    echo ""
}

# Generate platform-specific README files
generate_readmes() {
    echo -e "${YELLOW}Generating platform documentation...${NC}"
    
    # Pi Zero 2 W
    cat > "$OUTPUT_DIR/pi-zero2/README.txt" << 'EOF'
Raspberry Pi Zero 2 W Bootloader
================================

Installation:
1. Format SD card with FAT32 partition
2. Download Pi firmware files from:
   https://github.com/raspberrypi/firmware/tree/master/boot
   Required: start.elf, fixup.dat
3. Copy to SD card:
   - start.elf
   - fixup.dat  
   - config.txt (included)
   - kernel8.img (included)

UART Connection:
- TX: GPIO14 (pin 8)
- RX: GPIO15 (pin 10)
- GND: pin 6
- Baud: 115200, 8N1

The bootloader will print system info and enter echo mode.
EOF

    # Pi 5
    cat > "$OUTPUT_DIR/pi5/README.txt" << 'EOF'
Raspberry Pi 5 Bootloader
=========================

NOTE: This is a placeholder build. Pi 5 support is incomplete.

The Pi 5 uses different:
- Peripheral base addresses
- RP1 southbridge for I/O
- Boot firmware chain

Full implementation requires hardware testing.

Installation (when complete):
1. Format SD card with FAT32 partition
2. Get Pi 5 firmware files (start4.elf, fixup4.dat)
3. Copy kernel8.img and config.txt to SD card
EOF

    # KickPi K2B
    cat > "$OUTPUT_DIR/kickpi-k2b/README.txt" << 'EOF'
KickPi K2B (Allwinner H618) Bootloader
======================================

NOTE: This is a placeholder build. DRAM initialization required.

The H618 boot chain:
1. BROM loads boot0 from SD offset 8KB
2. boot0 initializes DRAM
3. boot0 loads U-Boot or payload to DRAM

Flashing to SD card:
  sudo dd if=boot0.bin of=/dev/sdX bs=1024 seek=8

FEL Mode (USB boot for development):
  sunxi-fel spl boot0.bin

Full implementation requires:
- Clock (CCU) initialization
- DRAM controller setup
- UART0 initialization
EOF

    # RP2040
    cat > "$OUTPUT_DIR/rp2040/README.txt" << 'EOF'
RP2040 Bootloader
=================

NOTE: This is a placeholder build. Stage2 flash init required.

The RP2040 boot chain:
1. Boot ROM loads 256-byte stage2 from flash
2. Stage2 configures XIP (execute-in-place)
3. Main application runs from flash

To flash:
1. Hold BOOTSEL button while connecting USB
2. Convert to UF2: elf2uf2-rs bootloader.elf bootloader.uf2
3. Copy bootloader.uf2 to mounted drive

Full implementation requires:
- Stage2 flash timing for W25Q chips
- Clock configuration (XOSC, PLL)
- Proper vector table setup
EOF

    echo -e "${GREEN}Documentation generated.${NC}"
}

# Create combined archive
create_archive() {
    echo ""
    echo -e "${YELLOW}Creating combined archive...${NC}"
    
    cd "$OUTPUT_DIR"
    tar -czvf ../rustboot-binaries.tar.gz ./*
    
    echo -e "${GREEN}Archive created: rustboot-binaries.tar.gz${NC}"
}

# Main
main() {
    cd "$BUILD_DIR"
    
    create_output_dirs
    build_all
    generate_readmes
    create_archive
    
    echo ""
    echo "========================================"
    echo " Build Complete!"
    echo "========================================"
    echo ""
    echo "Output directory: $OUTPUT_DIR"
    echo ""
    echo "Contents:"
    find "$OUTPUT_DIR" -type f -name "*.img" -o -name "*.bin" | sort | while read f; do
        size=$(stat -c%s "$f")
        echo "  $(basename $(dirname $f))/$(basename $f) - $size bytes"
    done
    echo ""
}

main "$@"
