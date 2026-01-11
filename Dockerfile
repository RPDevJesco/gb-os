# =============================================================================
# Dockerfile - Rust Cross-Compilation Environment for RPi Zero 2W Bare-Metal
# =============================================================================

FROM rust:latest

LABEL maintainer="GB-OS Bare-Metal"
LABEL description="Build environment for RPi Zero 2W bare-metal Game Boy emulator"

# Install additional tools
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    wget \
    zip \
    && rm -rf /var/lib/apt/lists/*

# Pin to a specific nightly to avoid sync issues on every run
RUN rustup default nightly-2025-01-07

# Add the bare-metal AArch64 targets
RUN rustup target add aarch64-unknown-none-softfloat
RUN rustup target add aarch64-unknown-none

# Add required components
RUN rustup component add rust-src
RUN rustup component add llvm-tools

# Install cargo-binutils for objcopy
RUN cargo install cargo-binutils

WORKDIR /project

ENTRYPOINT ["/bin/bash", "-c"]
CMD ["chmod +x ./docker-entrypoint.sh 2>/dev/null; ./docker-entrypoint.sh"]
