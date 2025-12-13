# Rustacean OS + GameBoy Integration Makefile
#
# Builds either normal Rustacean OS or GameBoy edition from same codebase.
#
# Targets:
#   make            - Build normal Rustacean OS
#   make gameboy    - Build GameBoy edition
#   make tools      - Build mkgamedisk ROM converter
#   make game ROM=x - Create game floppy from ROM file
#   make run        - Run normal mode in QEMU
#   make run-gb     - Run GameBoy mode in QEMU

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

.PHONY: all normal gameboy clean tools game run run-gb

# Default: build normal Rustacean OS
all: normal

# ============================================================================
# Normal Rustacean OS Build
# ============================================================================

normal: $(NORMAL_IMG)
	@echo ""
	@echo "=== Normal Rustacean OS built ==="
	@echo "Run with: make run"

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

$(GAMEBOY_IMG): $(BOOT_BIN) $(STAGE2_GB_BIN) $(KERNEL_BIN)
	$(DD) if=/dev/zero of=$@ bs=512 count=2880 2>/dev/null
	$(DD) if=$(BOOT_BIN) of=$@ bs=512 count=1 conv=notrunc 2>/dev/null
	$(DD) if=$(STAGE2_GB_BIN) of=$@ bs=512 seek=1 conv=notrunc 2>/dev/null
	$(DD) if=$(KERNEL_BIN) of=$@ bs=512 seek=33 conv=notrunc 2>/dev/null

# Stage 2 for GameBoy mode (with GAMEBOY_MODE flag)
$(STAGE2_GB_BIN): $(BOOT_DIR)/stage2.asm | $(BUILD_DIR)
	$(NASM) -f bin -DGAMEBOY_MODE -o $@ $<

# ============================================================================
# Common Components
# ============================================================================

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Stage 1 bootloader (same for both modes)
$(BOOT_BIN): $(BOOT_DIR)/boot.asm | $(BUILD_DIR)
	$(NASM) -f bin -o $@ $<

# Kernel (same binary, mode detected at runtime via boot info magic)
$(KERNEL_BIN): FORCE | $(BUILD_DIR)
	cd $(KERNEL_DIR) && $(CARGO) build --release \
		--target $(TARGET_JSON) \
		-Zbuild-std=core,alloc \
		-Zbuild-std-features=compiler-builtins-mem
	$(OBJCOPY) -O binary $(KERNEL_ELF) $@

# ============================================================================
# Tools
# ============================================================================

tools: $(BUILD_DIR)/mkgamedisk

$(BUILD_DIR)/mkgamedisk: FORCE | $(BUILD_DIR)
	cd $(TOOLS_DIR)/mkgamedisk && $(CARGO) build --release
	cp $(TOOLS_DIR)/mkgamedisk/target/release/mkgamedisk $@
	@echo "Built: $@"

# Create game floppy from ROM file
# Usage: make game ROM=path/to/game.gb
game: $(BUILD_DIR)/mkgamedisk
ifndef ROM
	$(error ROM not specified. Usage: make game ROM=path/to/game.gb)
endif
	$(BUILD_DIR)/mkgamedisk $(ROM) $(BUILD_DIR)/game.img
	@echo ""
	@echo "Game floppy created: $(BUILD_DIR)/game.img"

# ============================================================================
# Run in QEMU
# ============================================================================

# Run normal Rustacean OS
run: $(NORMAL_IMG)
	qemu-system-i386 \
		-fda $< \
		-boot a \
		-m 256M \
		-vga std

# Run GameBoy edition (prompts for floppy swap)
run-gb: $(GAMEBOY_IMG)
	@echo "GameBoy Mode: You'll be prompted to 'insert' game floppy"
	@echo "In QEMU, use Ctrl+Alt+2 for monitor, then: change floppy0 build/game.img"
	@echo ""
	qemu-system-i386 \
		-fda $< \
		-boot a \
		-m 256M \
		-vga std

# Run GameBoy with game pre-loaded (for testing)
# Usage: make run-game ROM=path/to/game.gb
run-game: $(GAMEBOY_IMG) game
	@echo "Running GameBoy OS with game floppy..."
	qemu-system-i386 \
		-fda $(GAMEBOY_IMG) \
		-fdb $(BUILD_DIR)/game.img \
		-boot a \
		-m 256M \
		-vga std

# Debug mode (text output to serial)
debug: $(NORMAL_IMG)
	qemu-system-i386 \
		-fda $< \
		-boot a \
		-m 256M \
		-nographic \
		-serial mon:stdio

# ============================================================================
# Clean
# ============================================================================

clean:
	rm -rf $(BUILD_DIR)
	cd $(KERNEL_DIR) && $(CARGO) clean 2>/dev/null || true
	cd $(TOOLS_DIR)/mkgamedisk && $(CARGO) clean 2>/dev/null || true

FORCE:
