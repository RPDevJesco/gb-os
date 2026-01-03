#!/bin/bash
# rustboot Linux/macOS Build Script
# Builds all platform bootloaders using Docker

set -e

echo "========================================"
echo " rustboot - Docker Build"
echo "========================================"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

IMAGE_NAME="rustboot-builder"
CONTAINER_NAME="rustboot-build-container"

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    echo -e "${RED}ERROR: Docker is not installed${NC}"
    echo "Please install Docker from https://docker.com"
    exit 1
fi

# Check if Docker daemon is running
if ! docker info &> /dev/null; then
    echo -e "${RED}ERROR: Docker daemon is not running${NC}"
    echo "Please start Docker and try again"
    exit 1
fi

echo -e "${YELLOW}[1/4] Building Docker image...${NC}"
echo "This may take a few minutes on first run..."
echo ""

docker build -t "$IMAGE_NAME" .

echo ""
echo -e "${YELLOW}[2/4] Running build container...${NC}"
echo ""

# Remove any existing container with same name
docker rm -f "$CONTAINER_NAME" 2>/dev/null || true

# Run the build
docker run --name "$CONTAINER_NAME" "$IMAGE_NAME"

echo ""
echo -e "${YELLOW}[3/4] Copying output files...${NC}"
echo ""

# Remove old output directory
rm -rf output

# Copy output from container
docker cp "$CONTAINER_NAME":/build/output ./output
docker cp "$CONTAINER_NAME":/build/rustboot-binaries.tar.gz ./

echo ""
echo -e "${YELLOW}[4/4] Cleaning up...${NC}"
echo ""

docker rm "$CONTAINER_NAME" 2>/dev/null || true

echo ""
echo "========================================"
echo -e "${GREEN} Build Complete!${NC}"
echo "========================================"
echo ""
echo "Output files are in the 'output' directory:"
echo ""

# List output files
for dir in output/*/; do
    platform=$(basename "$dir")
    echo "  $platform/"
    for f in "$dir"*.img "$dir"*.bin; do
        if [ -f "$f" ]; then
            size=$(stat -f%z "$f" 2>/dev/null || stat -c%s "$f" 2>/dev/null)
            echo "    $(basename "$f") - $size bytes"
        fi
    done
done

echo ""
echo "Combined archive: rustboot-binaries.tar.gz"
echo ""
