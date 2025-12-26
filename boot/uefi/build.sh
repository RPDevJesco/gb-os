#!/bin/bash
# Build and run the UEFI bootloader
# Usage: ./build.sh [build|run|clean]

set -e

TARGET="x86_64-unknown-uefi"
BINARY="target/${TARGET}/release/uefi-bootloader.efi"
ESP_DIR="esp/EFI/BOOT"

build() {
    echo "Building UEFI bootloader..."
    
    # Add UEFI target if not present
    rustup target add $TARGET 2>/dev/null || true
    
    # Build in release mode
    cargo build --release --target $TARGET
    
    # Create ESP directory structure
    mkdir -p $ESP_DIR
    
    # Copy bootloader to ESP
    cp $BINARY $ESP_DIR/BOOTX64.EFI
    
    # Create startup.nsh for auto-boot in UEFI shell
    cat > esp/startup.nsh << 'EOF'
@echo -off
fs0:\EFI\BOOT\BOOTX64.EFI
EOF

    echo "Built: $ESP_DIR/BOOTX64.EFI"
    echo "Size: $(wc -c < $ESP_DIR/BOOTX64.EFI | tr -d ' ') bytes"
}

find_ovmf() {
    # macOS Homebrew paths (Apple Silicon and Intel)
    local MACOS_PATHS=(
        "/opt/homebrew/share/qemu/edk2-x86_64-code.fd"
        "/opt/homebrew/Cellar/qemu/*/share/qemu/edk2-x86_64-code.fd"
        "/usr/local/share/qemu/edk2-x86_64-code.fd"
        "/usr/local/Cellar/qemu/*/share/qemu/edk2-x86_64-code.fd"
    )

    # Linux paths
    local LINUX_PATHS=(
        "/usr/share/OVMF/OVMF_CODE.fd"
        "/usr/share/edk2-ovmf/x64/OVMF_CODE.fd"
        "/usr/share/qemu/OVMF_CODE.fd"
        "/usr/share/edk2/ovmf/OVMF_CODE.fd"
        "/usr/share/OVMF/x64/OVMF_CODE.fd"
    )

    # Check macOS paths first (with glob expansion)
    for pattern in "${MACOS_PATHS[@]}"; do
        for path in $pattern; do
            if [[ -f "$path" ]]; then
                echo "$path"
                return 0
            fi
        done
    done

    # Check Linux paths
    for path in "${LINUX_PATHS[@]}"; do
        if [[ -f "$path" ]]; then
            echo "$path"
            return 0
        fi
    done

    return 1
}

run() {
    build

    echo "Running in QEMU..."

    # Find OVMF firmware
    OVMF=$(find_ovmf) || true

    if [[ -z "$OVMF" ]]; then
        echo ""
        echo "ERROR: OVMF UEFI firmware not found!"
        echo ""
        echo "Install OVMF/EDK2 for your platform:"
        echo "  macOS:   brew install qemu  (includes EDK2)"
        echo "  Ubuntu:  sudo apt install ovmf"
        echo "  Fedora:  sudo dnf install edk2-ovmf"
        echo "  Arch:    sudo pacman -S edk2-ovmf"
        echo ""
        echo "Or download manually from:"
        echo "  https://github.com/tianocore/edk2/releases"
        echo ""
        echo "Then set OVMF_PATH environment variable:"
        echo "  export OVMF_PATH=/path/to/OVMF_CODE.fd"
        exit 1
    fi

    # Allow override via environment variable
    OVMF="${OVMF_PATH:-$OVMF}"

    echo "Using OVMF: $OVMF"

    # Detect if we're on macOS (no KVM) or Linux
    if [[ "$(uname)" == "Darwin" ]]; then
        # macOS - no KVM, use HVF if available
        ACCEL=""
        if sysctl -n kern.hv_support 2>/dev/null | grep -q 1; then
            # Apple Silicon or Intel with HVF
            ACCEL="-accel hvf"
            echo "Using Hypervisor.framework acceleration"
        else
            echo "Running without hardware acceleration (slower)"
        fi

        qemu-system-x86_64 \
            $ACCEL \
            -m 256M \
            -drive if=pflash,format=raw,readonly=on,file="$OVMF" \
            -drive format=raw,file=fat:rw:esp \
            -net none \
            -serial stdio \
            -display default,show-cursor=on
    else
        # Linux - try KVM
        KVM_OPT=""
        if [[ -r /dev/kvm ]]; then
            KVM_OPT="-enable-kvm"
            echo "Using KVM acceleration"
        fi

        qemu-system-x86_64 \
            $KVM_OPT \
            -m 256M \
            -drive if=pflash,format=raw,readonly=on,file="$OVMF" \
            -drive format=raw,file=fat:rw:esp \
            -net none \
            -serial stdio
    fi
}

clean() {
    echo "Cleaning..."
    cargo clean
    rm -rf esp esp.img
}

case "${1:-build}" in
    build)
        build
        ;;
    run)
        run
        ;;
    clean)
        clean
        ;;
    *)
        echo "Usage: $0 [build|run|clean]"
        exit 1
        ;;
esac