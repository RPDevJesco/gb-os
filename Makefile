# gb-os Makefile
#
# Builds bare-metal GameBoy emulator with support for:
#   - Floppy disk boot (legacy 1.44MB)
#   - CD-ROM boot via El Torito no-emulation mode
#   - USB/HDD boot (raw image)
#
# Quick Start (Docker - Recommended):
#   ./docker-build.sh                    # Build GameBoy edition
#   ./docker-build.sh --rom game.gb      # Build with embedded ROM
#
# Native Build Targets:
#   make                - Show help
#   make gameboy        - Build GameBoy edition
#   make normal         - Build normal edition
#   make tools          - Build mkgamedisk ROM converter
#   make game ROM=x     - Create game floppy from ROM file
#   make run-gb         - Run GameBoy mode in QEMU (floppy)
#   make run-gb-cd      - Run GameBoy mode in QEMU (CD)
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

# ISO images
NORMAL_ISO := $(BUILD_DIR)/rustacean.iso
GAMEBOY_ISO := $(BUILD_DIR)/gameboy-system.iso

# Docker
DOCKER_IMAGE := gb-os-builder

.PHONY: all normal gameboy clean tools game run run-gb run-cd run-gb-cd docker docker-shell help iso

# Default: show help
help:
	@echo "gb-os Build System (No-Emulation Boot)"
	@echo ""
	@echo "Docker Build (Recommended):"
	@echo "  make docker              Build GameBoy edition via Docker"
	@echo "  make docker-shell        Open shell in build container"
	@echo ""
	@echo "Native Build:"
	@echo "  make normal              Build normal gb-os"
	@echo "  make gameboy             Build GameBoy edition"
	@echo "  make tools               Build mkgamedisk tool"
	@echo "  make game ROM=path.gb    Create game floppy from ROM"
	@echo ""
	@echo "Run in QEMU:"
	@echo "  make run                 Run normal mode (floppy)"
	@echo "  make run-gb              Run GameBoy mode (floppy)"
	@echo "  make run-cd              Run normal mode (CD)"
	@echo "  make run-gb-cd           Run GameBoy mode (CD)"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean               Remove build artifacts"
	@echo "  make distclean           Remove build + Docker image"

# ============================================================================
# Docker Build
# ============================================================================

docker:
	./docker-build.sh --gameboy

docker-shell:
	./docker-build.sh --shell

# ============================================================================
# Normal Edition Build
# ============================================================================

normal: $(NORMAL_IMG) $(NORMAL_ISO)
	@mkdir -p $(OUTPUT_DIR)
	cp $(NORMAL_IMG) $(OUTPUT_DIR)/
	cp $(NORMAL_ISO) $(OUTPUT_DIR)/
	@echo ""
	@echo "Normal edition built:"
	@echo "  $(OUTPUT_DIR)/rustacean.img (floppy)"
	@echo "  $(OUTPUT_DIR)/rustacean.iso (CD, no-emulation boot)"

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

gameboy: $(GAMEBOY_IMG) $(GAMEBOY_ISO)
	@mkdir -p $(OUTPUT_DIR)
	cp $(GAMEBOY_IMG) $(OUTPUT_DIR)/
	cp $(GAMEBOY_ISO) $(OUTPUT_DIR)/
	cp $(KERNEL_BIN) $(OUTPUT_DIR)/
	@echo ""
	@echo "GameBoy edition built:"
	@echo "  $(OUTPUT_DIR)/gameboy-system.img (floppy)"
	@echo "  $(OUTPUT_DIR)/gameboy-system.iso (CD, no-emulation boot)"

$(GAMEBOY_IMG): $(BOOT_BIN) $(STAGE2_GB_BIN) $(KERNEL_BIN)
	$(DD) if=/dev/zero of=$@ bs=512 count=2880 2>/dev/null
	$(DD) if=$(BOOT_BIN) of=$@ bs=512 count=1 conv=notrunc 2>/dev/null
	$(DD) if=$(STAGE2_GB_BIN) of=$@ bs=512 seek=1 conv=notrunc 2>/dev/null
	$(DD) if=$(KERNEL_BIN) of=$@ bs=512 seek=33 conv=notrunc 2>/dev/null

$(STAGE2_GB_BIN): $(BOOT_DIR)/stage2.asm | $(BUILD_DIR)
	$(NASM) -f bin -DGAMEBOY_MODE -o $@ $<

# ============================================================================
# ISO Creation (No-Emulation El Torito Boot)
# ============================================================================

$(NORMAL_ISO): $(NORMAL_IMG) $(BOOT_BIN) $(STAGE2_BIN) $(KERNEL_BIN)
	@mkdir -p $(BUILD_DIR)/iso/boot
	# Create boot image for El Torito
	$(DD) if=$(BOOT_BIN) of=$(BUILD_DIR)/iso/boot/boot.img bs=2048 count=1 conv=sync 2>/dev/null
	$(DD) if=$(STAGE2_BIN) of=$(BUILD_DIR)/iso/boot/boot.img bs=2048 seek=1 conv=notrunc 2>/dev/null
	$(DD) if=$(KERNEL_BIN) of=$(BUILD_DIR)/iso/boot/boot.img bs=2048 seek=9 conv=notrunc 2>/dev/null
	cp $(NORMAL_IMG) $(BUILD_DIR)/iso/
	# Create ISO with no-emulation boot
	xorriso -as mkisofs -o $@ -V "RUSTACEAN_OS" \
		-b boot/boot.img -no-emul-boot -boot-load-size 4 -boot-info-table \
		-R -J $(BUILD_DIR)/iso/ 2>/dev/null || \
	genisoimage -o $@ -V "RUSTACEAN_OS" \
		-b boot/boot.img -no-emul-boot -boot-load-size 4 -boot-info-table \
		-R -J $(BUILD_DIR)/iso/ 2>/dev/null
	rm -rf $(BUILD_DIR)/iso

$(GAMEBOY_ISO): $(GAMEBOY_IMG) $(BOOT_BIN) $(STAGE2_GB_BIN) $(KERNEL_BIN)
	@mkdir -p $(BUILD_DIR)/iso/boot
	# Create boot image for El Torito
	$(DD) if=$(BOOT_BIN) of=$(BUILD_DIR)/iso/boot/boot.img bs=2048 count=1 conv=sync 2>/dev/null
	$(DD) if=$(STAGE2_GB_BIN) of=$(BUILD_DIR)/iso/boot/boot.img bs=2048 seek=1 conv=notrunc 2>/dev/null
	$(DD) if=$(KERNEL_BIN) of=$(BUILD_DIR)/iso/boot/boot.img bs=2048 seek=9 conv=notrunc 2>/dev/null
	cp $(GAMEBOY_IMG) $(BUILD_DIR)/iso/
	# Create ISO with no-emulation boot
	xorriso -as mkisofs -o $@ -V "GAMEBOY_OS" \
		-b boot/boot.img -no-emul-boot -boot-load-size 4 -boot-info-table \
		-R -J $(BUILD_DIR)/iso/ 2>/dev/null || \
	genisoimage -o $@ -V "GAMEBOY_OS" \
		-b boot/boot.img -no-emul-boot -boot-load-size 4 -boot-info-table \
		-R -J $(BUILD_DIR)/iso/ 2>/dev/null
	rm -rf $(BUILD_DIR)/iso

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

# Run with CD-ROM image (no-emulation boot)
run-cd: $(NORMAL_ISO)
	$(QEMU) -cdrom $< -boot d -m 256M

run-gb-cd: $(GAMEBOY_ISO)
	$(QEMU) -cdrom $< -boot d -m 256M

# Run with USB emulation (raw disk image)
run-usb: $(GAMEBOY_IMG)
	$(QEMU) -drive file=$<,format=raw,if=none,id=usbdisk \
		-device usb-ehci -device usb-storage,drive=usbdisk \
		-boot menu=on -m 256M

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
