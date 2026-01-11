#!/bin/bash
# =============================================================================
# build.sh - Build Script for GB-OS Bare-Metal Kernel
# =============================================================================
# Usage:
#   ./build.sh          - Build release kernel
#   ./build.sh debug    - Build debug kernel with symbols
#   ./build.sh clean    - Clean build artifacts
#   ./build.sh sdcard   - Create SD card image directory
# =============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Target architecture
TARGET="aarch64-unknown-none-softfloat"
KERNEL_NAME="kernel8"
KERNEL_IMG="${KERNEL_NAME}.img"

# Print colored message
info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# Check for required tools
check_tools() {
    info "Checking for required tools..."
    
    if ! command -v rustup &> /dev/null; then
        error "rustup not found. Please install Rust: https://rustup.rs"
    fi
    
    if ! command -v cargo &> /dev/null; then
        error "cargo not found. Please install Rust: https://rustup.rs"
    fi
    
    # Check for the target
    if ! rustup target list --installed | grep -q "$TARGET"; then
        info "Installing target: $TARGET"
        rustup target add "$TARGET"
    fi
    
    # Check for rust-src (needed for build-std)
    if ! rustup component list --installed | grep -q "rust-src"; then
        info "Installing rust-src component..."
        rustup component add rust-src
    fi
    
    # Check for llvm-tools (for objcopy)
    if ! rustup component list --installed | grep -q "llvm-tools"; then
        info "Installing llvm-tools component..."
        rustup component add llvm-tools
    fi
    
    success "All required tools present"
}

# Build the kernel
build_kernel() {
    local profile="$1"
    
    info "Building kernel (profile: $profile)..."
    
    if [ "$profile" = "debug" ]; then
        CARGO_PROFILE="dev"
        TARGET_DIR="target/$TARGET/debug"
    else
        CARGO_PROFILE="release"
        TARGET_DIR="target/$TARGET/release"
    fi
    
    # Build with cargo
    cargo build --profile "$CARGO_PROFILE" --target "$TARGET" -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem
    
    if [ ! -f "$TARGET_DIR/$KERNEL_NAME" ]; then
        error "Build failed: $TARGET_DIR/$KERNEL_NAME not found"
    fi
    
    success "Build complete: $TARGET_DIR/$KERNEL_NAME"
    
    # Convert ELF to raw binary
    info "Creating binary image..."
    
    # Find the llvm-objcopy in the toolchain
    OBJCOPY=$(find "$(rustc --print sysroot)" -name "llvm-objcopy" 2>/dev/null | head -n1)
    
    if [ -z "$OBJCOPY" ]; then
        # Try cargo-binutils if available
        if command -v rust-objcopy &> /dev/null; then
            OBJCOPY="rust-objcopy"
        else
            warn "llvm-objcopy not found, trying system objcopy"
            if command -v aarch64-linux-gnu-objcopy &> /dev/null; then
                OBJCOPY="aarch64-linux-gnu-objcopy"
            elif command -v aarch64-none-elf-objcopy &> /dev/null; then
                OBJCOPY="aarch64-none-elf-objcopy"
            else
                error "No suitable objcopy found. Install llvm-tools: rustup component add llvm-tools"
            fi
        fi
    fi
    
    "$OBJCOPY" -O binary "$TARGET_DIR/$KERNEL_NAME" "$KERNEL_IMG"
    
    # Create output directory and copy artifacts
    OUTPUT_DIR="output"
    mkdir -p "$OUTPUT_DIR"
    
    cp "$KERNEL_IMG" "$OUTPUT_DIR/"
    cp "$TARGET_DIR/$KERNEL_NAME" "$OUTPUT_DIR/${KERNEL_NAME}.elf"
    
    if [ -f "sdcard/config.txt" ]; then
        cp "sdcard/config.txt" "$OUTPUT_DIR/"
    fi

    # Print image info
    local img_size=$(stat -f%z "$OUTPUT_DIR/$KERNEL_IMG" 2>/dev/null || stat -c%s "$OUTPUT_DIR/$KERNEL_IMG" 2>/dev/null)
    success "Created $OUTPUT_DIR/$KERNEL_IMG ($(numfmt --to=iec-i --suffix=B $img_size 2>/dev/null || echo "$img_size bytes"))"
    
    echo ""
    info "Output files in ./$OUTPUT_DIR/:"
    ls -la "$OUTPUT_DIR/"
}

# Clean build artifacts
clean() {
    info "Cleaning build artifacts..."
    cargo clean
    rm -f "$KERNEL_IMG"
    rm -rf output/
    rm -rf sdcard_output/
    success "Clean complete"
}

# Create SD card directory with all needed files
create_sdcard() {
    local sdcard_dir="output"
    
    info "Creating SD card output directory..."
    
    mkdir -p "$sdcard_dir"
    
    # Copy kernel image
    if [ ! -f "$sdcard_dir/$KERNEL_IMG" ]; then
        if [ ! -f "$KERNEL_IMG" ]; then
            warn "Kernel image not found, building first..."
            build_kernel "release"
        else
            cp "$KERNEL_IMG" "$sdcard_dir/"
        fi
    fi
    
    # Copy config.txt
    if [ -f "sdcard/config.txt" ]; then
        cp "sdcard/config.txt" "$sdcard_dir/"
    else
        warn "sdcard/config.txt not found"
    fi
    
    # Download RPi boot files if not present
    local boot_files=("bootcode.bin" "start.elf" "fixup.dat")
    local firmware_url="https://github.com/raspberrypi/firmware/raw/master/boot"
    
    for file in "${boot_files[@]}"; do
        if [ ! -f "$sdcard_dir/$file" ]; then
            info "Downloading $file..."
            if command -v curl &> /dev/null; then
                curl -sL "$firmware_url/$file" -o "$sdcard_dir/$file" || warn "Failed to download $file"
            elif command -v wget &> /dev/null; then
                wget -q "$firmware_url/$file" -O "$sdcard_dir/$file" || warn "Failed to download $file"
            else
                warn "Neither curl nor wget found, cannot download boot files"
                warn "Please manually download from: $firmware_url"
            fi
        fi
    done
    
    success "SD card files ready in: $sdcard_dir/"
    info "Copy all files from ./$sdcard_dir/ to a FAT32 formatted SD card"
    echo ""
    ls -la "$sdcard_dir/"
}

# Show usage
usage() {
    echo "GB-OS Bare-Metal Build Script"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  (none)    Build release kernel"
    echo "  debug     Build debug kernel with symbols"
    echo "  clean     Clean build artifacts"
    echo "  sdcard    Create SD card directory with all boot files"
    echo "  help      Show this help message"
}

# Main entry point
main() {
    case "${1:-}" in
        "")
            check_tools
            build_kernel "release"
            ;;
        "debug")
            check_tools
            build_kernel "debug"
            ;;
        "clean")
            clean
            ;;
        "sdcard")
            check_tools
            create_sdcard
            ;;
        "help"|"--help"|"-h")
            usage
            ;;
        *)
            error "Unknown command: $1"
            usage
            exit 1
            ;;
    esac
}

main "$@"
