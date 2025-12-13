# Rustacean OS + GameBoy Integration Makefile
#
# Builds either normal Rustacean OS or GameBoy edition from same codebase.
#
# Targets:
#   make            - Build normal Rustacean OS
#   make gameboy    - Build GameBoy edition
#   make both       - Build both editions
#   make tools      - Build mkgamedisk ROM converter
#   make game ROM=x - Create game floppy from ROM file
#   make run        - Run normal mode in QEMU
#   make run-gb     - Run GameBoy mode in QEMU
#   make components - Build just boot.bin, stage2.bin, kernel.bin

# Tools
NASM := nasm
CARGO := cargo
DD := dd
OBJCOPY := objcopy

# Directories
BOOT_DIR := boot
KERNEL_DIR := kernel
TOOLS_DIR := tools
BUILD_DIR := build

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

.PHONY: all normal gameboy both clean tools game run run-gb components

# Default: build normal Rustacean OS
all: normal

# ============================================================================
# Normal Rustacean OS Build
# ============================================================================

normal: $(NORMAL_IMG)
	@echo ""
	@echo "=== Normal Rustacean OS built ==="
	@echo "Run with: make run"
	@echo ""
	@echo "Components in $(BUILD_DIR)/:"
	@echo "  boot.bin    - Stage 1 bootloader"
	@echo "  stage2.bin  - Stage 2 bootloader"
	@echo "  kernel.bin  - Kernel binary"

$(NORMAL_IMG): $(BOOT_BIN) $(STAGE2_BIN) $(KERNEL_BIN)
	$(DD) if=/dev/zero of=$@ bs=512 count=2880 2>/dev/null
	$(DD) if=$(BOOT_BIN) of=$@ bs=512 count=1 conv=notrunc 2>/dev/null
	$(DD) if=$(STAGE2_BIN) of=$@ bs=512 seek=1 conv=notrunc 2>/dev/null
	$(DD) if=$(KERNEL_BIN) of=$@ bs=512 seek=33 conv=notrunc 2>/dev/null

# Stage 2 for normal mode (no GAMEBOY_MODE flag)
$(STAGE2_BIN): $(BOOT_DIR)/stage2.asm | $(BUILD_DIR)
	$(NASM) -f bin -o $@ $<

# ============================================================================
# GameBoy Edition Build
# ============================================================================

gameboy: $(GAMEBOY_IMG)
	@echo ""
	@echo "=== GameBoy Edition built ==="
	@echo "Run with: make run-gb"
	@echo "Create game floppy with: make game ROM=path/to/game.gb"
	@echo ""
	@echo "Components in $(BUILD_DIR)/:"
	@echo "  boot.bin           - Stage 1 bootloader"
	@echo "  stage2-gameboy.bin - Stage 2 bootloader (GameBoy mode)"
	@echo "  kernel.bin         - Kernel binary"

$(GAMEBOY_IMG): $(BOOT_BIN) $(STAGE2_GB_BIN) $(KERNEL_BIN)
	$(DD) if=/dev/zero of=$@ bs=512 count=2880 2>/dev/null
	$(DD) if=$(BOOT_BIN) of=$@ bs=512 count=1 conv=notrunc 2>/dev/null
	$(DD) if=$(STAGE2_GB_BIN) of=$@ bs=512 seek=1 conv=notrunc 2>/dev/null
	$(DD) if=$(KERNEL_BIN) of=$@ bs=512 seek=33 conv=notrunc 2>/dev/null

# Stage 2 for GameBoy mode (with GAMEBOY_MODE flag)
$(STAGE2_GB_BIN): $(BOOT_DIR)/stage2.asm | $(BUILD_DIR)
	$(NASM) -f bin -DGAMEBOY_MODE -o $@ $<

# ============================================================================
# Build Both Editions
# ============================================================================

both: $(NORMAL_IMG) $(GAMEBOY_IMG)
	@echo ""
	@echo "=== Both editions built ==="
	@echo ""
	@echo "Components in $(BUILD_DIR)/:"
	@echo "  boot.bin           - Stage 1 bootloader"
	@echo "  stage2.bin         - Stage 2 bootloader (normal mode)"
	@echo "  stage2-gameboy.bin - Stage 2 bootloader (GameBoy mode)"
	@echo "  kernel.bin         - Kernel binary"

# ============================================================================
# Components Only (no disk images)
# ============================================================================

components: $(BOOT_BIN) $(STAGE2_BIN) $(STAGE2_GB_BIN) $(KERNEL_BIN)
	@echo ""
	@echo "=== Components built ==="
	@echo ""
	@echo "$(BUILD_DIR)/boot.bin           - Stage 1 bootloader ($(shell stat -c%s $(BOOT_BIN) 2>/dev/null || echo '?') bytes)"
	@echo "$(BUILD_DIR)/stage2.bin         - Stage 2 normal ($(shell stat -c%s $(STAGE2_BIN) 2>/dev/null || echo '?') bytes)"
	@echo "$(BUILD_DIR)/stage2-gameboy.bin - Stage 2 GameBoy ($(shell stat -c%s $(STAGE2_GB_BIN) 2>/dev/null || echo '?') bytes)"
	@echo "$(BUILD_DIR)/kernel.bin         - Kernel ($(shell stat -c%s $(KERNEL_BIN) 2>/dev/null || echo '?') bytes)"

# ============================================================================
# Common Components
# ============================================================================

$(BOOT_BIN): $(BOOT_DIR)/boot.asm | $(BUILD_DIR)
	$(NASM) -f bin -o $@ $<

$(KERNEL_BIN): $(KERNEL_ELF)
	$(OBJCOPY) -O binary $< $@

$(KERNEL_ELF): FORCE | $(BUILD_DIR)
	cd $(KERNEL_DIR) && $(CARGO) +nightly build --release \
		--target $(TARGET_JSON) \
		-Zbuild-std=core,alloc \
		-Zbuild-std-features=compiler-builtins-mem

FORCE:

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# ============================================================================
# Tools
# ============================================================================

tools: $(BUILD_DIR)/mkgamedisk

$(BUILD_DIR)/mkgamedisk: | $(BUILD_DIR)
	cd $(TOOLS_DIR)/mkgamedisk && $(CARGO) build --release
	cp $(TOOLS_DIR)/mkgamedisk/target/release/mkgamedisk $@

# ============================================================================
# Create Game Floppy
# ============================================================================

game: $(BUILD_DIR)/mkgamedisk
ifndef ROM
	$(error ROM not specified. Usage: make game ROM=path/to/game.gb)
endif
	$(BUILD_DIR)/mkgamedisk $(ROM) $(BUILD_DIR)/game.img
	@echo "Game floppy created: $(BUILD_DIR)/game.img"

# ============================================================================
# Run in QEMU
# ============================================================================

run: $(NORMAL_IMG)
	qemu-system-i386 -fda $(NORMAL_IMG) -boot a -m 256M

run-gb: $(GAMEBOY_IMG)
	qemu-system-i386 -fda $(GAMEBOY_IMG) -boot a -m 256M

# Run with game floppy
run-game: $(GAMEBOY_IMG)
ifndef ROM
	$(error ROM not specified. Usage: make run-game ROM=path/to/game.gb)
endif
	$(MAKE) game ROM=$(ROM)
	@echo "Starting QEMU with game..."
	@echo "Use QEMU monitor (Ctrl+Alt+2) to change floppies if needed"
	qemu-system-i386 -fda $(GAMEBOY_IMG) -fdb $(BUILD_DIR)/game.img -boot a -m 256M

# ============================================================================
# Clean
# ============================================================================

clean:
	rm -rf $(BUILD_DIR)
	cd $(KERNEL_DIR) && $(CARGO) clean
	@if [ -d "$(TOOLS_DIR)/mkgamedisk" ]; then \
		cd $(TOOLS_DIR)/mkgamedisk && $(CARGO) clean; \
	fi

# ============================================================================
# Help
# ============================================================================

help:
	@echo "Rustacean OS Build System"
	@echo ""
	@echo "Build Targets:"
	@echo "  make              Build normal Rustacean OS"
	@echo "  make gameboy      Build GameBoy edition"
	@echo "  make both         Build both editions"
	@echo "  make components   Build individual components only (no disk images)"
	@echo "  make tools        Build mkgamedisk tool"
	@echo ""
	@echo "Run Targets:"
	@echo "  make run          Run normal mode in QEMU"
	@echo "  make run-gb       Run GameBoy mode in QEMU"
	@echo "  make run-game ROM=file.gb   Run with game"
	@echo ""
	@echo "Other:"
	@echo "  make game ROM=file.gb   Create game floppy"
	@echo "  make clean        Clean build artifacts"
	@echo ""
	@echo "Output Components (in build/):"
	@echo "  boot.bin           - Stage 1 bootloader (512 bytes)"
	@echo "  stage2.bin         - Stage 2 bootloader (normal mode)"
	@echo "  stage2-gameboy.bin - Stage 2 bootloader (GameBoy mode)"
	@echo "  kernel.bin         - Kernel binary"
