# Rustacean OS + GameBoy Integration Makefile
#
# Builds either normal Rustacean OS or GameBoy edition from same codebase.
# Uses LBA disk access (no floppy emulation) to support larger ROMs.
#
# Targets:
#   make            - Build normal Rustacean OS
#   make gameboy    - Build GameBoy edition
#   make tools      - Build mkgamedisk ROM converter
#   make game ROM=x - Create game disk image from ROM file
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

# Disk images (larger than floppy to support bigger ROMs)
# 8MB disk image = 16384 sectors (enough for 4MB+ ROMs like Pokemon)
DISK_SECTORS := 16384
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
	$(DD) if=/dev/zero of=$@ bs=512 count=$(DISK_SECTORS) 2>/dev/null
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
	@echo "Create game disk with: make game ROM=path/to/game.gb"

$(GAMEBOY_IMG): $(BOOT_BIN) $(STAGE2_GB_BIN) $(KERNEL_BIN)
	$(DD) if=/dev/zero of=$@ bs=512 count=$(DISK_SECTORS) 2>/dev/null
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
# Run in QEMU (using hard disk emulation for LBA support)
# ============================================================================

# Run normal Rustacean OS
run: $(NORMAL_IMG)
	qemu-system-i386 \
		-drive file=$<,format=raw,if=ide \
		-boot c \
		-m 256M \
		-vga std

# Run GameBoy edition
run-gb: $(GAMEBOY_IMG)
	@echo "GameBoy Mode: ROM can be embedded in disk image or loaded from partition"
	@echo ""
	qemu-system-i386 \
		-drive file=$<,format=raw,if=ide \
		-boot c \
		-m 256M \
		-vga std

# Run GameBoy with ROM embedded in disk image
# Usage: make run-game ROM=path/to/game.gb
run-game: $(GAMEBOY_IMG)
ifndef ROM
	$(error ROM not specified. Usage: make run-game ROM=path/to/game.gb)
endif
	@echo "Embedding ROM into disk image..."
	@# Create ROM header (512 bytes): 'GBOY' + size + title + padding
	@ROM_SIZE=$$(stat -c%s "$(ROM)"); \
	ROM_TITLE=$$(dd if="$(ROM)" bs=1 skip=308 count=16 2>/dev/null | tr -d '\0' | tr -cd '[:alnum:] '); \
	[ -z "$$ROM_TITLE" ] && ROM_TITLE="UNKNOWN"; \
	printf 'GBOY' > $(BUILD_DIR)/rom_header.bin; \
	printf "$$(printf '\\x%02x\\x%02x\\x%02x\\x%02x' \
		$$((ROM_SIZE & 0xFF)) \
		$$(((ROM_SIZE >> 8) & 0xFF)) \
		$$(((ROM_SIZE >> 16) & 0xFF)) \
		$$(((ROM_SIZE >> 24) & 0xFF)))" >> $(BUILD_DIR)/rom_header.bin; \
	printf "%-32s" "$$ROM_TITLE" | head -c 32 >> $(BUILD_DIR)/rom_header.bin; \
	$(DD) if=/dev/zero bs=1 count=$$((512 - 40)) >> $(BUILD_DIR)/rom_header.bin 2>/dev/null; \
	$(DD) if=$(BUILD_DIR)/rom_header.bin of=$< bs=512 seek=289 conv=notrunc 2>/dev/null; \
	$(DD) if="$(ROM)" of=$< bs=512 seek=290 conv=notrunc 2>/dev/null; \
	echo "ROM embedded: $$ROM_TITLE ($$ROM_SIZE bytes)"
	@echo "Running GameBoy OS with embedded ROM..."
	qemu-system-i386 \
		-drive file=$<,format=raw,if=ide \
		-boot c \
		-m 256M \
		-vga std

# Debug mode (text output to serial)
debug: $(NORMAL_IMG)
	qemu-system-i386 \
		-drive file=$<,format=raw,if=ide \
		-boot c \
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
