#!/bin/bash
# =============================================================================
# docker-build.sh - Docker Build Script for Linux/macOS
# =============================================================================
#
# Usage:
#   ./docker-build.sh              Build release kernel
#   ./docker-build.sh debug        Build debug kernel
#   ./docker-build.sh sdcard       Build and create SD card directory
#   ./docker-build.sh shell        Open interactive shell in container
#   ./docker-build.sh clean        Remove Docker image and build artifacts
#
# =============================================================================

set -e

# Configuration
IMAGE_NAME="gb-os-builder"
CONTAINER_NAME="gb-os-build-container"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Check for Docker
check_docker() {
    if ! command -v docker &> /dev/null; then
        error "Docker is not installed. Please install Docker first."
    fi
    
    if ! docker info &> /dev/null; then
        error "Docker daemon is not running. Please start Docker."
    fi
}

# Build Docker image if it doesn't exist or is outdated
build_image() {
    local dockerfile_hash=$(md5sum Dockerfile 2>/dev/null | cut -d' ' -f1 || md5 -q Dockerfile 2>/dev/null)
    local cached_hash=""
    
    if docker image inspect "$IMAGE_NAME" &> /dev/null; then
        cached_hash=$(docker image inspect "$IMAGE_NAME" --format '{{index .Config.Labels "dockerfile.hash"}}' 2>/dev/null || echo "")
    fi
    
    if [ "$cached_hash" != "$dockerfile_hash" ] || [ -z "$cached_hash" ]; then
        info "Building Docker image: $IMAGE_NAME"
        docker build \
            --label "dockerfile.hash=$dockerfile_hash" \
            -t "$IMAGE_NAME" \
            .
        success "Docker image built successfully"
    else
        info "Using cached Docker image: $IMAGE_NAME"
    fi
}

# Run build in container
run_build() {
    local build_mode="${1:-release}"
    local create_sdcard="${2:-0}"
    
    info "Starting build (mode: $build_mode)..."
    
    docker run --rm \
        -v "$(pwd):/project" \
        -e "BUILD_MODE=$build_mode" \
        -e "CREATE_SDCARD=$create_sdcard" \
        --name "$CONTAINER_NAME" \
        "$IMAGE_NAME"
    
    # Verify output exists
    if [ -d "output" ] && [ -f "output/kernel8.img" ]; then
        success "Build complete! Output files in ./output/"
        echo ""
        ls -la output/
    else
        error "Build may have failed - output/kernel8.img not found"
    fi
}

# Open interactive shell
run_shell() {
    info "Opening interactive shell in container..."
    
    docker run --rm -it \
        -v "$(pwd):/project" \
        --name "$CONTAINER_NAME" \
        "$IMAGE_NAME" \
        /bin/bash
}

# Clean up
clean() {
    info "Cleaning up..."
    
    # Remove container if running
    docker rm -f "$CONTAINER_NAME" 2>/dev/null || true
    
    # Remove image
    if docker image inspect "$IMAGE_NAME" &> /dev/null; then
        docker rmi "$IMAGE_NAME"
        success "Removed Docker image: $IMAGE_NAME"
    fi
    
    # Remove build artifacts
    rm -rf target/
    rm -f kernel8.img
    rm -rf output/
    rm -rf sdcard_output/
    
    success "Clean complete"
}

# Show usage
usage() {
    echo "GB-OS Bare-Metal Docker Build Script"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  (none)     Build release kernel"
    echo "  debug      Build debug kernel with symbols"
    echo "  sdcard     Build and create SD card directory with boot files"
    echo "  shell      Open interactive shell in build container"
    echo "  rebuild    Force rebuild of Docker image"
    echo "  clean      Remove Docker image and build artifacts"
    echo "  help       Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                  # Build release kernel"
    echo "  $0 debug            # Build debug kernel"
    echo "  $0 sdcard           # Build and prepare SD card files"
    echo "  $0 shell            # Interactive shell for debugging"
}

# Main
main() {
    check_docker
    
    case "${1:-}" in
        ""|"release")
            build_image
            run_build "release" "0"
            ;;
        "debug")
            build_image
            run_build "debug" "0"
            ;;
        "sdcard")
            build_image
            run_build "release" "1"
            ;;
        "shell")
            build_image
            run_shell
            ;;
        "rebuild")
            info "Forcing Docker image rebuild..."
            docker rmi "$IMAGE_NAME" 2>/dev/null || true
            build_image
            ;;
        "clean")
            clean
            ;;
        "help"|"--help"|"-h")
            usage
            ;;
        *)
            error "Unknown command: $1"
            echo ""
            usage
            exit 1
            ;;
    esac
}

main "$@"
