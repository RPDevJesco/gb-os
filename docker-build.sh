#!/bin/bash
#
# gb-os Docker Build Script (HOST)
#
# This script runs on YOUR machine (not in the container).
# It builds the Docker image and runs the build inside.
#
# Usage:
#   ./docker-build.sh                    # Build GameBoy edition (default)
#   ./docker-build.sh --gameboy          # Build GameBoy edition
#   ./docker-build.sh --both             # Build both editions
#   ./docker-build.sh --rom game.gb      # Build with embedded ROM
#   ./docker-build.sh --shell            # Open shell in container
#   ./docker-build.sh --no-cache         # Force rebuild without cache
#

set -e

# Configuration
IMAGE_NAME="gb-os-builder"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="${SCRIPT_DIR}/output"

# Build options
BUILD_MODE="--gameboy"
ROM_FILE=""
NO_CACHE=""
SHELL_MODE=""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored message
info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Show help
show_help() {
    cat << EOF
gb-os Docker Build Script

Usage: $0 [options]

Build Options:
  --gameboy         Build GameBoy edition only (default)
  --normal          Build normal edition only
  --both            Build both normal and GameBoy editions
  --rom FILE        Embed ROM into GameBoy ISO
  --tools           Build mkgamedisk tool only

Docker Options:
  --no-cache        Force rebuild without Docker cache
  --shell           Open a shell in the build container

Other Options:
  --help, -h        Show this help message

Examples:
  $0                              Build GameBoy edition
  $0 --rom tetris.gb              Build with embedded ROM
  $0 --both --rom pokemon.gb      Build both, GameBoy has ROM
  $0 --shell                      Debug in container

Boot Methods:
  The built images support:
  - Floppy disk boot (gameboy-system.img)
  - CD-ROM boot with no-emulation El Torito (gameboy-system.iso)
  - USB/HDD boot (dd gameboy-system.img to drive)

EOF
    exit 0
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --no-cache)
            NO_CACHE="--no-cache"
            shift
            ;;
        --shell)
            SHELL_MODE="yes"
            shift
            ;;
        --gameboy)
            BUILD_MODE="--gameboy"
            shift
            ;;
        --normal)
            BUILD_MODE="--normal"
            shift
            ;;
        --both)
            BUILD_MODE="--both"
            shift
            ;;
        --tools)
            BUILD_MODE="--tools"
            shift
            ;;
        --rom)
            ROM_FILE="$2"
            shift 2
            ;;
        --help|-h)
            show_help
            ;;
        *)
            warn "Unknown option: $1"
            shift
            ;;
    esac
done

# Validate ROM file if specified
if [ -n "$ROM_FILE" ]; then
    if [ ! -f "$ROM_FILE" ]; then
        error "ROM file not found: $ROM_FILE"
        exit 1
    fi
    # Get absolute path
    ROM_FILE="$(cd "$(dirname "$ROM_FILE")" && pwd)/$(basename "$ROM_FILE")"
    info "ROM file: $ROM_FILE"
fi

echo "========================================"
echo "  gb-os Docker Builder"
echo "  No-Emulation Boot Support"
echo "========================================"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Check Docker is available
if ! command -v docker &> /dev/null; then
    error "Docker is not installed or not in PATH"
    exit 1
fi

# Build Docker image
info "Building Docker image '$IMAGE_NAME'..."
if docker build $NO_CACHE -t "$IMAGE_NAME" "$SCRIPT_DIR"; then
    success "Docker image built"
else
    error "Docker build failed!"
    exit 1
fi

echo ""

# Shell mode - just open bash
if [ "$SHELL_MODE" = "yes" ]; then
    info "Opening shell in container..."
    docker run --rm -it \
        -v "$OUTPUT_DIR:/output" \
        "$IMAGE_NAME" \
        /bin/bash
    exit 0
fi

# Run the build
info "Running build ($BUILD_MODE)..."

if [ -n "$ROM_FILE" ]; then
    ROM_DIR="$(dirname "$ROM_FILE")"
    ROM_NAME="$(basename "$ROM_FILE")"
    info "Embedding ROM: $ROM_NAME"

    docker run --rm \
        -v "$OUTPUT_DIR:/output" \
        -v "$ROM_DIR:/input:ro" \
        -e "ROM_FILE=/input/$ROM_NAME" \
        "$IMAGE_NAME" \
        /build.sh $BUILD_MODE
else
    docker run --rm \
        -v "$OUTPUT_DIR:/output" \
        "$IMAGE_NAME" \
        /build.sh $BUILD_MODE
fi

if [ $? -ne 0 ]; then
    error "Build failed!"
    exit 1
fi

echo ""
echo "========================================"
echo "  Output Files"
echo "========================================"
ls -la "$OUTPUT_DIR/"

echo ""
success "Build complete! Output in: $OUTPUT_DIR"
echo ""

if [[ "$BUILD_MODE" == *"gameboy"* ]] || [[ "$BUILD_MODE" == "--both" ]]; then
    echo "To run GameBoy mode:"
    echo ""
    echo "  Floppy boot:"
    echo "    qemu-system-i386 -fda $OUTPUT_DIR/gameboy-system.img -boot a -m 256M"
    echo ""
    echo "  CD-ROM boot (no-emulation):"
    echo "    qemu-system-i386 -cdrom $OUTPUT_DIR/gameboy-system.iso -boot d -m 256M"
    echo ""
    echo "  USB/HDD installation:"
    echo "    sudo dd if=$OUTPUT_DIR/gameboy-system.img of=/dev/sdX bs=512"
    echo ""
fi
