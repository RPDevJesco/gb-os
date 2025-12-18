# gb-os Build Environment
#
# Builds the kernel and bootloader in an isolated container
# with all necessary toolchain components.
#
# Usage:
#   docker build -t gb-os-builder .
#   docker run --rm -v "$(pwd)/output:/output" gb-os-builder
#   docker run --rm -v "$(pwd)/output:/output" gb-os-builder /build.sh --gameboy
#
# Windows (PowerShell):
#   docker build -t gb-os-builder .
#   docker run --rm -v "${PWD}/output:/output" gb-os-builder
#
# Windows (CMD):
#   docker build -t gb-os-builder .
#   docker run --rm -v "%cd%/output:/output" gb-os-builder

FROM ubuntu:24.04

LABEL maintainer="gb-os Contributors"
LABEL description="Build environment for gb-os (GameBoy bare-metal emulator)"

# Avoid interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Install build dependencies (including dos2unix for line ending conversion)
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
    dos2unix \
    && rm -rf /var/lib/apt/lists/*

# Install Rust nightly
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain nightly

# Add Rust to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Install rust-src component for building core/alloc
RUN rustup component add rust-src --toolchain nightly

# Create working directory
WORKDIR /gb-os

# Copy project files
COPY boot/ ./boot/
COPY kernel/ ./kernel/
COPY tools/ ./tools/
COPY i686-rustacean.json ./
COPY Makefile ./
COPY build.sh /build.sh

# Fix line endings (Windows CRLF -> Unix LF) and make executable
RUN dos2unix /build.sh && chmod +x /build.sh && mkdir -p /output

# Default command runs the GameBoy build
CMD ["/build.sh", "--gameboy"]
