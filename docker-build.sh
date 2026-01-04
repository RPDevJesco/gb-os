#!/bin/bash
# rustboot Docker build script
# Builds all platform targets and organizes outputs

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

# Build directory (root of project in container)
BUILD_DIR="/build"

# Source directory (where Cargo.toml and platform folders are)
SRC_DIR="$BUILD_DIR/kernel/src"

# Output directory
OUTPUT_DIR="$BUILD_DIR/output"

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

    # Platform directory is under kernel/src/platform/
    local platform_dir="$SRC_DIR/platform/$name"
    local linker_script="$platform_dir/linker.ld"

    # Check linker script exists
    if [ ! -f "$linker_script" ]; then
        echo -e "${RED}  ERROR: Linker script not found: $linker_script${NC}"
        return 1
    fi

    # Build with cargo from the kernel/src directory (where Cargo.toml is)
    if ! RUSTFLAGS="-C link-arg=-T$linker_script" \
        cargo build \
        --manifest-path "$SRC_DIR/Cargo.toml" \
        --release \
        --package "rustboot-$name" \
        --target "$rust_target" \
        2>&1; then
        echo -e "${RED}  ERROR: Cargo build failed for $name${NC}"
        return 1
    fi

    # Output paths - cargo puts output relative to manifest directory
    local elf_path="$SRC_DIR/target/$rust_target/release/$bin_name"
    local bin_path="$SRC_DIR/target/$rust_target/release/${bin_name}.bin"
    local target_output_dir="$OUTPUT_DIR/$name"

    # Check if build succeeded
    if [ ! -f "$elf_path" ]; then
        echo -e "${RED}  ERROR: Build failed for $name - ELF not found at $elf_path${NC}"
        return 1
    fi

    # Convert ELF to binary
    echo "  Converting ELF to binary..."
    if ! rust-objcopy -O binary "$elf_path" "$bin_path" 2>&1; then
        echo -e "${RED}  ERROR: objcopy failed for $name${NC}"
        return 1
    fi

    # Copy outputs
    cp "$bin_path" "$target_output_dir/$out_name"
    cp "$elf_path" "$target_output_dir/"

    # Copy config.txt if it exists
    if [ -f "$platform_dir/config.txt" ]; then
        cp "$platform_dir/config.txt" "$target_output_dir/"
    fi

    # Get size info
    local size=$(stat -c%s "$target_output_dir/$out_name")
    echo -e "${GREEN}  SUCCESS: $out_name ($size bytes)${NC}"

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
    echo -e "Successful: ${GREEN}$success${NC}"
    echo -e "Failed: ${RED}$failed${NC}"
}

# Generate README files for each platform
generate_readmes() {
    echo ""
    echo -e "${YELLOW}Generating platform documentation...${NC}"

    # Pi Zero 2 W
    cat > "$OUTPUT_DIR/pi-zero2/README.txt" << 'EOF'
Raspberry Pi Zero 2 W Bootloader
================================

Files:
  kernel8.img  - Kernel binary (copy to SD card)
  config.txt   - GPU configuration (copy to SD card)
  kernel8      - ELF file (for debugging)

Installation:
1. Format SD card with FAT32 partition
2. Download Pi firmware files from:
   https://github.com/raspberrypi/firmware/tree/master/boot
   Required: start.elf, fixup.dat
3. Copy to SD card:
   - start.elf
   - fixup.dat
   - config.txt
   - kernel8.img

The kernel will display a green border and title box on HDMI.
The ACT LED will blink patterns during boot.
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
EOF

    # KickPi K2B
    cat > "$OUTPUT_DIR/kickpi-k2b/README.txt" << 'EOF'
KickPi K2B (Allwinner H618) Bootloader
======================================

NOTE: This is a placeholder build. DRAM initialization required.

The H618 boot chain requires complex initialization.
EOF

    # RP2040
    cat > "$OUTPUT_DIR/rp2040/README.txt" << 'EOF'
RP2040 Bootloader
=================

NOTE: This is a placeholder build.

The RP2040 requires stage2 flash configuration.
EOF

    echo -e "${GREEN}Documentation generated.${NC}"
}

# Create combined archive
create_archive() {
    echo ""
    echo -e "${YELLOW}Creating combined archive...${NC}"
    
    cd "$BUILD_DIR"
    tar -czvf rustboot-binaries.tar.gz -C "$OUTPUT_DIR" .
    
    echo -e "${GREEN}Archive created: rustboot-binaries.tar.gz${NC}"
}

# Main execution
main() {
    create_output_dirs
    build_all
    generate_readmes
    create_archive
    
    echo ""
    echo "========================================"
    echo -e "${GREEN} All Done!${NC}"
    echo "========================================"
    echo ""
    echo "Output files in: $OUTPUT_DIR"
    echo "Combined archive: $BUILD_DIR/rustboot-binaries.tar.gz"
}

main
