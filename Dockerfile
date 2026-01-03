# rustboot build environment
# Multi-architecture bare-metal bootloader build container

FROM ubuntu:24.04

LABEL maintainer="Jesse"
LABEL description="Build environment for rustboot multi-platform bootloader"

# Avoid interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Install system dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    git \
    bc \
    && rm -rf /var/lib/apt/lists/*

# Install Rust via rustup
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain nightly --profile minimal

# Add embedded targets
RUN rustup target add aarch64-unknown-none thumbv6m-none-eabi

# Add required components
RUN rustup component add rust-src llvm-tools

# Install cargo-binutils for objcopy
RUN cargo install cargo-binutils

# Create build directory
WORKDIR /build

# Copy source files
COPY . /build/

# Make build script executable (script is in project root)
RUN chmod +x /build/docker-build.sh

# Default command runs the build
CMD ["/build/docker-build.sh"]
