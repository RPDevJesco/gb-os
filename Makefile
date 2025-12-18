# gb-os Makefile
#
# Builds bare-metal GameBoy emulator or normal gb-os.
#
# Quick Start (Docker - Recommended):
#   ./docker-build.sh                    # Build GameBoy edition
#   ./docker-build.sh --rom game.gb      # Build with embedded ROM
#
# Native Build Targets:
#   make                - Build normal gb-os
#   make gameboy        # Build GameBoy edition
#   make tools          - Build mkgamedisk ROM converter
#   make game ROM=x     - Create game floppy from ROM file
#   make run            - Run normal mode in QEMU
#   make run-gb         - Run GameBoy mode in QEMU
#   make docker         - Build via Docker
#   make clean          - Remove build artifacts

# Tools
NASM := nasm
CARGO := cargo
DD := dd
OBJCOPY := objcopy
QEMU := qemu-system-i386

# Directories
BOOT_DIR := boot
KERNEL_DIR := kernel
TOOLS_DIR := tools
BUILD_DIR := build
OUTPUT_DIR := output

# Target specification
TARGET_JSON := ../i686-rustacean.json
TARGET_DIR := $(KERNEL_DIR)/target/i686-rustacean/release

# Output files
BOOT_BIN := $(BUILD_DIR)/boot.bin
STAGE2_BIN := $(BUILD_DIR)/stage2.bin
STAGE2_GB_BIN := $(BUILD_DIR)/stage2-gameboy.bin
KERNEL_ELF := $(TARGET_DIR)/rustacean-kernel
KERNEL_BIN := $(BUILD_DIR)/kernel.bin

# Floppy images
NORMAL_IMG := $(BUILD_DIR)/rustacean.img
GAMEBOY_IMG := $(BUILD_DIR)/gameboy-system.img

# Docker
DOCKER_IMAGE := gb-os-builder

.PHONY: all normal gameboy clean tools game run run-gb docker docker-shell help

# Default: show help
help:
	@echo "gb-os Build System"
	@echo ""
	@echo "Docker Build (Recommended):"
	@echo "  make docker              Build GameBoy edition via Docker"
	@echo "  make docker-shell        Open shell in build container"
	@echo ""
	@echo "Native Build:"
	@echo "  make normal              Build normal gb-os"
	@echo "  make gameboy             Build GameBoy edition"
	@echo "  make tools               Build mkgamedisk tool"
	@echo "  make game ROM=file.gb    Create game floppy from ROM"
	@echo ""
	@echo "Run:"
	@echo "  make run                 Run normal mode in QEMU"
	@echo "  make run-gb              Run GameBoy mode in QEMU"
	@echo ""
	@echo "Other:"
	@echo "  make clean               Remove build artifacts"
	@echo ""

# Convenience: just 'make' builds gameboy
all: gameboy

# ============================================================================
# Docker Build (Recommended)
# ============================================================================

docker:
	@./docker-build.sh --gameboy

docker-shell:
	@./docker-build.sh --shell

# ============================================================================
# Normal gb-os Build
# ============================================================================

normal: $(NORMAL_IMG)
	@echo ""
	@echo "=== Normal gb-os built ==="
	@echo "Run with: make run"

$(NORMAL_IMG): $(BOOT_BIN) $(STAGE2_BIN) $(KERNEL_BIN)
	$(DD) if=/dev/zero of=$@ bs=512 count=2880 2>/dev/null
	$(DD) if=$(BOOT_BIN) of=$@ bs=512 count=1 conv=notrunc 2>/dev/null
	$(DD) if=$(STAGE2_BIN) of=$@ bs=512 seek=1 conv=notrunc 2>/dev/null
	$(DD) if=$(KERNEL_BIN) of=$@ bs=512 seek=33 conv=notrunc 2>/dev/null

$(STAGE2_BIN): $(BOOT_DIR)/stage2.asm | $(BUILD_DIR)
	$(NASM) -f bin -o $@ $<

# ============================================================================
# GameBoy Edition Build
# ============================================================================

gameboy: $(GAMEBOY_IMG)
	@echo ""
	@echo "=== GameBoy Edition built ==="
	@echo "Run with: make run-gb"
	@echo "Create game floppy: make game ROM=path/to/game.gb"

$(GAMEBOY_IMG): $(BOOT_BIN) $(STAGE2_GB_BIN) $(KERNEL_BIN)
	$(DD) if=/dev/zero of=$@ bs=512 count=2880 2>/dev/null
	$(DD) if=$(BOOT_BIN) of=$@ bs=512 count=1 conv=notrunc 2>/dev/null
	$(DD) if=$(STAGE2_GB_BIN) of=$@ bs=512 seek=1 conv=notrunc 2>/dev/null
	$(DD) if=$(KERNEL_BIN) of=$@ bs=512 seek=33 conv=notrunc 2>/dev/null

$(STAGE2_GB_BIN): $(BOOT_DIR)/stage2.asm | $(BUILD_DIR)
	$(NASM) -f bin -DGAMEBOY_MODE -o $@ $<

# ============================================================================
# Common Build Steps
# ============================================================================

$(BOOT_BIN): $(BOOT_DIR)/boot.asm | $(BUILD_DIR)
	$(NASM) -f bin -o $@ $<

$(KERNEL_BIN): $(KERNEL_ELF) | $(BUILD_DIR)
	$(OBJCOPY) -O binary $< $@

$(KERNEL_ELF): FORCE
	cd $(KERNEL_DIR) && $(CARGO) +nightly build --release \
		--target $(TARGET_JSON) \
		-Zbuild-std=core,alloc \
		-Zbuild-std-features=compiler-builtins-mem

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(OUTPUT_DIR):
	mkdir -p $(OUTPUT_DIR)

FORCE:

# ============================================================================
# Tools
# ============================================================================

tools:
	@if [ -d "$(TOOLS_DIR)/mkgamedisk" ]; then \
		cd $(TOOLS_DIR)/mkgamedisk && $(CARGO) build --release; \
		mkdir -p $(BUILD_DIR); \
		cp target/release/mkgamedisk ../../$(BUILD_DIR)/; \
		echo "mkgamedisk built: $(BUILD_DIR)/mkgamedisk"; \
	else \
		echo "tools/mkgamedisk not found"; \
	fi

# ============================================================================
# Game Floppy Creation
# ============================================================================

game:
ifndef ROM
	@echo "Usage: make game ROM=path/to/game.gb"
	@echo ""
	@echo "Creates a game floppy image from a ROM file."
	@exit 1
endif
	@if [ ! -f "$(BUILD_DIR)/mkgamedisk" ]; then \
		echo "Building mkgamedisk first..."; \
		$(MAKE) tools; \
	fi
	@mkdir -p $(OUTPUT_DIR)
	$(BUILD_DIR)/mkgamedisk "$(ROM)" $(OUTPUT_DIR)/game.img
	@echo ""
	@echo "Game floppy created: $(OUTPUT_DIR)/game.img"

# ============================================================================
# Run in QEMU
# ============================================================================

run: $(NORMAL_IMG)
	$(QEMU) -fda $< -boot a -m 256M

run-gb: $(GAMEBOY_IMG)
	$(QEMU) -fda $< -boot a -m 256M

# Run with CD-ROM image
run-cd: $(BUILD_DIR)/rustacean.iso
	$(QEMU) -cdrom $< -boot d -m 256M

run-gb-cd: $(BUILD_DIR)/gameboy-system.iso
	$(QEMU) -cdrom $< -boot d -m 256M

# ============================================================================
# ISO Creation (optional)
# ============================================================================

$(BUILD_DIR)/rustacean.iso: $(NORMAL_IMG)
	mkdir -p $(BUILD_DIR)/iso
	cp $< $(BUILD_DIR)/iso/
	genisoimage -o $@ -b rustacean.img -no-emul-boot -boot-load-size 4 \
		-boot-info-table -V "RUSTACEAN_OS" $(BUILD_DIR)/iso/ 2>/dev/null || \
	xorriso -as mkisofs -o $@ -b rustacean.img -no-emul-boot -boot-load-size 4 \
		-boot-info-table -V "RUSTACEAN_OS" $(BUILD_DIR)/iso/ 2>/dev/null
	rm -rf $(BUILD_DIR)/iso

$(BUILD_DIR)/gameboy-system.iso: $(GAMEBOY_IMG)
	mkdir -p $(BUILD_DIR)/iso
	cp $< $(BUILD_DIR)/iso/
	genisoimage -o $@ -b gameboy-system.img -no-emul-boot -boot-load-size 4 \
		-boot-info-table -V "GAMEBOY_OS" $(BUILD_DIR)/iso/ 2>/dev/null || \
	xorriso -as mkisofs -o $@ -b gameboy-system.img -no-emul-boot -boot-load-size 4 \
		-boot-info-table -V "GAMEBOY_OS" $(BUILD_DIR)/iso/ 2>/dev/null
	rm -rf $(BUILD_DIR)/iso

# ============================================================================
# Clean
# ============================================================================

clean:
	rm -rf $(BUILD_DIR)
	rm -rf $(OUTPUT_DIR)
	cd $(KERNEL_DIR) && $(CARGO) clean 2>/dev/null || true
	@if [ -d "$(TOOLS_DIR)/mkgamedisk" ]; then \
		cd $(TOOLS_DIR)/mkgamedisk && $(CARGO) clean 2>/dev/null || true; \
	fi
	@echo "Clean complete"

# Deep clean including Docker
distclean: clean
	docker rmi $(DOCKER_IMAGE) 2>/dev/null || true
	@echo "Distclean complete"
