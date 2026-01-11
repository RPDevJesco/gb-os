# Game Boy Emulator Memory Allocation

A detailed breakdown of all memory allocations required by the gb-os-core emulator.

---

## Summary

| Category | DMG (Classic) | CGB (Color) | Notes |
|----------|---------------|-------------|-------|
| **Fixed Hardware** | 41,080 bytes | 57,464 bytes | VRAM, WRAM, OAM, ZRAM |
| **Framebuffer** | 69,120 bytes | 69,120 bytes | RGB888, 160×144 |
| **Audio (if enabled)** | ~64,300 bytes | ~64,300 bytes | 4 BlipBuf channels |
| **Cartridge ROM** | 32 KB - 8 MB | 32 KB - 8 MB | Varies by game |
| **Cartridge RAM** | 0 - 128 KB | 0 - 128 KB | Battery-backed |
| **Overhead** | ~2 KB | ~2 KB | Structs, state |
| | | | |
| **Minimum Total** | ~140 KB | ~157 KB | No cart RAM, no audio |
| **Typical Total** | ~210 KB | ~230 KB | 8KB cart RAM + audio |
| **Maximum Total** | ~8.5 MB | ~8.5 MB | 8MB ROM + 128KB RAM |

---

## Detailed Breakdown

### 1. Video RAM (VRAM)

```
DMG:  8,192 bytes  (0x2000) - Single bank
CGB: 16,384 bytes  (0x4000) - Two banks

Location: 0x8000-0x9FFF
Contains: Tile data, tile maps
```

### 2. Work RAM (WRAM)

```
DMG:  8,192 bytes  (0x2000) - Single bank
CGB: 32,768 bytes  (0x8000) - Eight banks (4KB × 8)

Location: 0xC000-0xDFFF (+ echo at 0xE000-0xFDFF)
Contains: Game variables, stack
```

### 3. Object Attribute Memory (OAM)

```
Size: 160 bytes (0xA0)

Location: 0xFE00-0xFE9F
Contains: Sprite attributes (40 sprites × 4 bytes)
```

### 4. High RAM / Zero Page (ZRAM)

```
Size: 127 bytes (0x7F)

Location: 0xFF80-0xFFFE
Contains: Fast-access variables
```

### 5. Framebuffer (GPU Output)

```
Format: RGB888 (3 bytes per pixel)
Resolution: 160 × 144 pixels
Size: 160 × 144 × 3 = 69,120 bytes

Allocated: Heap (Vec<u8>)
Usage: Display output buffer
```

### 6. GPU Internal State

```
Background Priority Array:
  Size: 160 bytes (1 byte per pixel in scanline)
  Purpose: Sprite/BG priority tracking

CGB Palettes:
  BG Palettes:     8 × 4 × 3 = 96 bytes
  Sprite Palettes: 8 × 4 × 3 = 96 bytes
  Total: 192 bytes

DMG Palettes:
  3 palettes × 4 entries = 12 bytes
```

### 7. Audio System (Optional)

Each audio channel uses a BlipBuf for band-limited synthesis:

```
BlipBuf Buffer Size:
  max_samples = 2001 (OUTPUT_SAMPLE_COUNT + 1)
  buffer_size = (2001 + 8) × 2 = 4,018 i32 values
  Per channel: 4,018 × 4 = 16,072 bytes

4 Channels Total:
  Channel 1 (Square + Sweep): 16,072 bytes
  Channel 2 (Square):         16,072 bytes
  Channel 3 (Wave):           16,072 bytes + 16 bytes wave RAM
  Channel 4 (Noise):          16,072 bytes
  ─────────────────────────────────────────
  Total BlipBuf:              64,304 bytes

Sound Struct Overhead: ~200 bytes (registers, state)

Audio Total: ~64,500 bytes
```

### 8. Cartridge ROM (Read-Only)

```
Typical Sizes:
  32 KB   (0x8000)    - 2 banks    - Simple games
  64 KB   (0x10000)   - 4 banks    - Early games
  128 KB  (0x20000)   - 8 banks    - Common
  256 KB  (0x40000)   - 16 banks   - Common
  512 KB  (0x80000)   - 32 banks   - Common
  1 MB    (0x100000)  - 64 banks   - Large games
  2 MB    (0x200000)  - 128 banks  - Very large
  4 MB    (0x400000)  - 256 banks  - Rare
  8 MB    (0x800000)  - 512 banks  - Maximum

Allocation: Heap (Vec<u8>)
Note: ROM is loaded entirely into RAM
```

### 9. Cartridge RAM (Battery-Backed)

```
Sizes by RAM Code (0x0149):
  0x00: None       (0 bytes)
  0x01: 2 KB       (treated as 8 KB)
  0x02: 8 KB       (0x2000)  - 1 bank
  0x03: 32 KB      (0x8000)  - 4 banks
  0x04: 128 KB     (0x20000) - 16 banks
  0x05: 64 KB      (0x10000) - 8 banks

Special Cases:
  MBC2: 512 bytes (4-bit values, 256 addresses)
  MBC3: May include RTC state (~18 bytes)

Allocation: Heap (Vec<u8>)
Note: Save data, persisted to file
```

### 10. MBC Controller State

```
MBC0: ~0 bytes (ROM only, no state)
MBC1: ~10 bytes (bank registers, mode)
MBC2: ~10 bytes (bank register, RAM enable)
MBC3: ~30 bytes (banks, RTC registers, latch)
MBC5: ~12 bytes (banks, RAM enable)

Plus:
  ROM name string: ~16 bytes
```

### 11. CPU State

```
Registers:
  A, F, B, C, D, E, H, L: 8 bytes
  SP, PC: 4 bytes
  Total: 12 bytes

Flags/State:
  IME, halt, halt_bug: ~3 bytes
  DI/EI delay counters: ~8 bytes
  Total: ~11 bytes

CPU Total: ~25 bytes
```

### 12. Timer State

```
DIV, TIMA, TMA, TAC: 4 bytes
Internal counters: 8 bytes
Interrupt flag: 1 byte
Step size: 4 bytes

Timer Total: ~17 bytes
```

### 13. Keypad State

```
Row registers: 2 bytes
Data register: 1 byte
Interrupt flag: 1 byte

Keypad Total: 4 bytes
```

### 14. Serial State

```
Data register: 1 byte
Control register: 1 byte
Interrupt flag: 1 byte
Callback pointer: 8 bytes (on 64-bit)

Serial Total: ~12 bytes
```

### 15. DMA State

```
HDMA registers: 4 bytes
HDMA source/dest: 4 bytes
HDMA length/status: 3 bytes

DMA Total: ~11 bytes
```

---

## Memory Layout by Component

```
┌─────────────────────────────────────────────────────────────┐
│                     HEAP ALLOCATIONS                         │
├─────────────────────────────────────────────────────────────┤
│ Cartridge ROM           │ 32 KB - 8 MB (game dependent)     │
│ Cartridge RAM           │ 0 - 128 KB (game dependent)       │
│ Framebuffer             │ 69,120 bytes (fixed)              │
│ BlipBuf × 4 (audio)     │ 64,288 bytes (if audio enabled)   │
├─────────────────────────────────────────────────────────────┤
│                    EMBEDDED IN STRUCTS                       │
├─────────────────────────────────────────────────────────────┤
│ VRAM                    │ 8-16 KB (DMG/CGB)                 │
│ WRAM                    │ 8-32 KB (DMG/CGB)                 │
│ OAM                     │ 160 bytes                         │
│ ZRAM                    │ 127 bytes                         │
│ Registers/State         │ ~500 bytes                        │
└─────────────────────────────────────────────────────────────┘
```

---

## Practical Memory Requirements

### Bare-Metal / Embedded Target (ARM Cortex-A53)

For a minimal working emulator on Raspberry Pi Zero 2W:

```
Minimum Required RAM:
  ├── Core emulator (no audio):     ~90 KB
  ├── Typical ROM (256 KB):        256 KB
  ├── Cart RAM (8 KB typical):       8 KB
  └── System overhead:              ~10 KB
      ─────────────────────────────────────
      Total:                       ~365 KB

With Audio:
  └── Add BlipBuf channels:        +65 KB
      ─────────────────────────────────────
      Total:                       ~430 KB

Large Game (2 MB ROM, 32 KB RAM):
      Total:                       ~2.2 MB
```

### Stack Requirements

```
Estimated Stack Usage:
  ├── Main loop:                    ~256 bytes
  ├── CPU instruction decode:       ~128 bytes
  ├── GPU scanline render:          ~512 bytes
  ├── Audio frame mixing:           ~32 KB (local buffers)
  └── Interrupt handling:           ~64 bytes
      ─────────────────────────────────────
      Recommended Stack:            64 KB (safe)
      Minimum Stack:                48 KB
```

### Total RAM Budget

```
For a 512 MB Raspberry Pi Zero 2W:

Conservative allocation:
  ├── Emulator:                    ~2-3 MB (large game)
  ├── Display buffer (double):     ~140 KB
  ├── DMA descriptors:             ~4 KB
  ├── Audio buffer:                ~32 KB
  └── Stack + misc:                ~128 KB
      ─────────────────────────────────────
      Total:                       ~3.5 MB

This leaves 508+ MB for:
  - Framebuffer (GPU memory)
  - Operating system (if any)
  - Additional features
```

---

## Optimization Notes

### Memory-Constrained Systems

1. **Stream ROM from storage** instead of loading entirely
   - Only keep 2-4 ROM banks in RAM
   - Requires ~64 KB instead of up to 8 MB

2. **Reduce audio buffer size**
   - OUTPUT_SAMPLE_COUNT can be reduced from 2000
   - Trade-off: More frequent audio callbacks needed

3. **Single-buffered framebuffer**
   - Use 69 KB instead of 138 KB
   - May cause tearing

4. **Skip audio entirely**
   - Saves ~65 KB
   - Many games playable without sound

### Performance-Critical Allocations

| Allocation | Access Pattern | Cache Notes |
|------------|----------------|-------------|
| VRAM | Random read/write | Keep in L1 if possible |
| WRAM | Random read/write | Bank 0 most accessed |
| OAM | Sequential (10×/frame) | Small, fits in cache |
| Framebuffer | Sequential write | Write-through OK |
| BlipBuf | Sequential | Not latency-critical |

---

## Quick Reference

```c
// Minimum allocations for DMG emulation
#define VRAM_SIZE      0x2000   //  8,192 bytes
#define WRAM_SIZE      0x2000   //  8,192 bytes
#define OAM_SIZE       0x00A0   //    160 bytes
#define ZRAM_SIZE      0x007F   //    127 bytes
#define FRAMEBUFFER    69120    // 69,120 bytes

// Additional for CGB
#define VRAM_SIZE_CGB  0x4000   // 16,384 bytes
#define WRAM_SIZE_CGB  0x8000   // 32,768 bytes

// Audio (per channel)
#define BLIPBUF_SIZE   16072    // ~16 KB per channel

// Typical ROM sizes
#define ROM_MIN        0x8000   //  32 KB
#define ROM_TYPICAL    0x40000  // 256 KB
#define ROM_MAX        0x800000 //   8 MB

// Cart RAM sizes
#define CART_RAM_NONE  0
#define CART_RAM_8K    0x2000   //  8 KB
#define CART_RAM_32K   0x8000   // 32 KB
#define CART_RAM_128K  0x20000  // 128 KB
```
