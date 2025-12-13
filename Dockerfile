# Rustacean OS Build Environment (with GameBoy Mode)
#
# Builds the kernel and bootloader in an isolated container
# with all necessary toolchain components.
#
# Usage:
#   docker build -t rustacean-builder .
#   docker run --rm -v $(pwd)/output:/output rustacean-builder
#   docker run --rm -v $(pwd)/output:/output rustacean-builder /build.sh --gameboy

FROM ubuntu:24.04

LABEL maintainer="Rustacean OS Contributors"
LABEL description="Build environment for Rustacean OS (with GameBoy Mode)"

# Avoid interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Install build dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    binutils \
    nasm \
    xorriso \
    grub-pc-bin \
    grub-common \
    grub2-common \
    mtools \
    dosfstools \
    qemu-system-x86 \
    genisoimage \
    && rm -rf /var/lib/apt/lists/*

# Install Rust nightly
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain nightly

# Add Rust to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Install rust-src component for building core/alloc
RUN rustup component add rust-src --toolchain nightly

# Create working directory
WORKDIR /rustacean-os

# Copy project files
COPY boot/ ./boot/
COPY kernel/ ./kernel/
COPY tools/ ./tools/
COPY i686-rustacean.json ./
COPY Makefile ./
COPY docker-build.sh /build.sh

# Fix Windows CRLF line endings and make build script executable
# This ensures the script works even if checked out with CRLF on Windows
RUN sed -i 's/\r$//' /build.sh && chmod +x /build.sh && mkdir -p /output

# Default command runs the build
CMD ["/build.sh"]
