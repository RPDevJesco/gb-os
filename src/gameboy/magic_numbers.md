# Game Boy Magic Numbers Cheatsheet

A comprehensive reference for all the hardware constants and magic numbers used in Game Boy emulation.

---

## Memory Map

| Range | Size | Description |
|-------|------|-------------|
| `0x0000-0x3FFF` | 16 KB | ROM Bank 0 (fixed) |
| `0x4000-0x7FFF` | 16 KB | ROM Bank 1-N (switchable) |
| `0x8000-0x9FFF` | 8 KB | Video RAM (VRAM) |
| `0xA000-0xBFFF` | 8 KB | External/Cartridge RAM |
| `0xC000-0xCFFF` | 4 KB | Work RAM Bank 0 |
| `0xD000-0xDFFF` | 4 KB | Work RAM Bank 1-7 (CGB switchable) |
| `0xE000-0xFDFF` | 7.5 KB | Echo RAM (mirror of C000-DDFF) |
| `0xFE00-0xFE9F` | 160 B | OAM (Object Attribute Memory) |
| `0xFEA0-0xFEFF` | 96 B | Unusable |
| `0xFF00-0xFF7F` | 128 B | I/O Registers |
| `0xFF80-0xFFFE` | 127 B | High RAM (HRAM/Zero Page) |
| `0xFFFF` | 1 B | Interrupt Enable Register |

### Memory Sizes
```
VRAM_SIZE   = 0x4000  (16 KB for CGB, 8 KB for DMG)
WRAM_SIZE   = 0x8000  (32 KB for CGB, 8 KB for DMG)
OAM_SIZE    = 0xA0    (160 bytes)
HRAM_SIZE   = 0x7F    (127 bytes)
```

---

## I/O Register Addresses

### Joypad (0xFF00)
| Address | Name | Description |
|---------|------|-------------|
| `0xFF00` | P1/JOYP | Joypad register |

**P1 Register Bits:**
```
Bit 7-6: Unused (read as 1)
Bit 5:   P15 - Select action buttons    (0=Select)
Bit 4:   P14 - Select direction buttons (0=Select)
Bit 3:   P13 - Down  or Start  (0=Pressed)
Bit 2:   P12 - Up    or Select (0=Pressed)
Bit 1:   P11 - Left  or B      (0=Pressed)
Bit 0:   P10 - Right or A      (0=Pressed)
```

### Serial Transfer (0xFF01-0xFF02)
| Address | Name | Description |
|---------|------|-------------|
| `0xFF01` | SB | Serial transfer data |
| `0xFF02` | SC | Serial transfer control |

**SC Register Bits:**
```
Bit 7:   Transfer start flag (1=Start)
Bit 1:   Clock speed (CGB only, 0=Normal, 1=Fast)
Bit 0:   Shift clock (0=External, 1=Internal)
```

### Timer (0xFF04-0xFF07)
| Address | Name | Description |
|---------|------|-------------|
| `0xFF04` | DIV | Divider register (increments at 16384 Hz) |
| `0xFF05` | TIMA | Timer counter |
| `0xFF06` | TMA | Timer modulo (reload value) |
| `0xFF07` | TAC | Timer control |

**TAC Register Bits:**
```
Bit 2:   Timer enable (1=Enable)
Bit 1-0: Clock select
         00 = 4096 Hz    (CPU/1024)
         01 = 262144 Hz  (CPU/16)
         10 = 65536 Hz   (CPU/64)
         11 = 16384 Hz   (CPU/256)
```

**Timer Step Values (in CPU cycles):**
```
TAC & 0x03 = 0  →  1024 cycles
TAC & 0x03 = 1  →  16 cycles
TAC & 0x03 = 2  →  64 cycles
TAC & 0x03 = 3  →  256 cycles
```

### Interrupts (0xFF0F, 0xFFFF)
| Address | Name | Description |
|---------|------|-------------|
| `0xFF0F` | IF | Interrupt flag (requested) |
| `0xFFFF` | IE | Interrupt enable |

**Interrupt Bits:**
```
Bit 4: Joypad    (0x10) - Vector: 0x0060
Bit 3: Serial    (0x08) - Vector: 0x0058
Bit 2: Timer     (0x04) - Vector: 0x0050
Bit 1: LCD STAT  (0x02) - Vector: 0x0048
Bit 0: V-Blank   (0x01) - Vector: 0x0040
```

**Interrupt Vector Addresses:**
```
0x0040 = V-Blank
0x0048 = LCD STAT
0x0050 = Timer
0x0058 = Serial
0x0060 = Joypad
```

### Sound (0xFF10-0xFF3F)
| Address | Name | Description |
|---------|------|-------------|
| `0xFF10` | NR10 | Channel 1 sweep |
| `0xFF11` | NR11 | Channel 1 length/duty |
| `0xFF12` | NR12 | Channel 1 volume envelope |
| `0xFF13` | NR13 | Channel 1 frequency low |
| `0xFF14` | NR14 | Channel 1 frequency high + control |
| `0xFF16` | NR21 | Channel 2 length/duty |
| `0xFF17` | NR22 | Channel 2 volume envelope |
| `0xFF18` | NR23 | Channel 2 frequency low |
| `0xFF19` | NR24 | Channel 2 frequency high + control |
| `0xFF1A` | NR30 | Channel 3 DAC enable |
| `0xFF1B` | NR31 | Channel 3 length |
| `0xFF1C` | NR32 | Channel 3 volume |
| `0xFF1D` | NR33 | Channel 3 frequency low |
| `0xFF1E` | NR34 | Channel 3 frequency high + control |
| `0xFF20` | NR41 | Channel 4 length |
| `0xFF21` | NR42 | Channel 4 volume envelope |
| `0xFF22` | NR43 | Channel 4 polynomial counter |
| `0xFF23` | NR44 | Channel 4 control |
| `0xFF24` | NR50 | Master volume + VIN panning |
| `0xFF25` | NR51 | Sound panning |
| `0xFF26` | NR52 | Sound on/off |
| `0xFF30-0xFF3F` | Wave RAM | 16 bytes of wave pattern |

### LCD/GPU (0xFF40-0xFF4B)
| Address | Name | Description |
|---------|------|-------------|
| `0xFF40` | LCDC | LCD control |
| `0xFF41` | STAT | LCD status |
| `0xFF42` | SCY | Scroll Y |
| `0xFF43` | SCX | Scroll X |
| `0xFF44` | LY | LCD Y coordinate (read-only) |
| `0xFF45` | LYC | LY compare |
| `0xFF46` | DMA | OAM DMA transfer start |
| `0xFF47` | BGP | BG palette (DMG) |
| `0xFF48` | OBP0 | Object palette 0 (DMG) |
| `0xFF49` | OBP1 | Object palette 1 (DMG) |
| `0xFF4A` | WY | Window Y position |
| `0xFF4B` | WX | Window X position + 7 |

**LCDC (0xFF40) Bits:**
```
Bit 7: LCD enable           (0=Off, 1=On)
Bit 6: Window tilemap       (0=9800-9BFF, 1=9C00-9FFF)
Bit 5: Window enable        (0=Off, 1=On)
Bit 4: BG/Window tile data  (0=8800-97FF, 1=8000-8FFF)
Bit 3: BG tilemap           (0=9800-9BFF, 1=9C00-9FFF)
Bit 2: Sprite size          (0=8x8, 1=8x16)
Bit 1: Sprite enable        (0=Off, 1=On)
Bit 0: BG/Window enable     (0=Off, 1=On) [DMG: priority, CGB: master enable]
```

**STAT (0xFF41) Bits:**
```
Bit 6: LYC=LY interrupt     (1=Enable)
Bit 5: Mode 2 interrupt     (1=Enable)
Bit 4: Mode 1 interrupt     (1=Enable)
Bit 3: Mode 0 interrupt     (1=Enable)
Bit 2: LYC=LY flag          (read-only)
Bit 1-0: Mode flag          (read-only)
         00 = HBlank
         01 = VBlank
         10 = OAM Search
         11 = Pixel Transfer
```

### CGB-Only Registers
| Address | Name | Description |
|---------|------|-------------|
| `0xFF4D` | KEY1 | CPU speed switch |
| `0xFF4F` | VBK | VRAM bank select |
| `0xFF51-0xFF55` | HDMA1-5 | HDMA source/dest/length |
| `0xFF68` | BCPS/BGPI | BG palette index |
| `0xFF69` | BCPD/BGPD | BG palette data |
| `0xFF6A` | OCPS/OBPI | Sprite palette index |
| `0xFF6B` | OCPD/OBPD | Sprite palette data |
| `0xFF70` | SVBK | WRAM bank select |

**KEY1 (0xFF4D) Bits:**
```
Bit 7: Current speed (0=Normal, 1=Double)
Bit 0: Speed switch request (write 1 to request)
```

---

## Timing Constants

### CPU Clock
```
DMG Clock:    4,194,304 Hz (4.194304 MHz)
CGB Normal:   4,194,304 Hz
CGB Double:   8,388,608 Hz

Machine Cycle: 4 T-states (1.048576 MHz effective)
```

### Frame Timing
```
Scanlines per frame:     154 (144 visible + 10 VBlank)
Dots per scanline:       456
Dots per frame:          70,224
Frames per second:       ~59.73 Hz

VBlank duration:         4,560 dots (10 lines × 456)
```

### Scanline Timing (456 dots total)
```
Mode 2 (OAM Search):      80 dots
Mode 3 (Pixel Transfer):  ~172 dots (variable: 172-289)
Mode 0 (HBlank):          ~204 dots (variable: 87-204)
```

### Frame Sequencer (Sound)
```
Frame sequencer rate:    512 Hz (8192 CPU cycles per step)
Steps per frame:         8

Step 0: Length counter
Step 1: -
Step 2: Length counter, Sweep
Step 3: -
Step 4: Length counter
Step 5: -
Step 6: Length counter, Sweep
Step 7: Volume envelope
```

---

## Screen Dimensions

```
SCREEN_W = 160 pixels
SCREEN_H = 144 pixels

Framebuffer size (RGB888): 160 × 144 × 3 = 69,120 bytes

Tilemap size:    32 × 32 tiles = 256 × 256 pixels
Tile size:       8 × 8 pixels
Tiles per row:   20 visible (32 total in tilemap)
Tiles per col:   18 visible (32 total in tilemap)

Sprite limit:    40 total, 10 per scanline
Sprite sizes:    8×8 or 8×16 pixels
```

---

## Cartridge Header

| Offset | Size | Description |
|--------|------|-------------|
| `0x0100-0x0103` | 4 | Entry point |
| `0x0104-0x0133` | 48 | Nintendo logo |
| `0x0134-0x0143` | 16 | Game title |
| `0x013F-0x0142` | 4 | Manufacturer code |
| `0x0143` | 1 | CGB flag |
| `0x0144-0x0145` | 2 | New licensee code |
| `0x0146` | 1 | SGB flag |
| `0x0147` | 1 | Cartridge type (MBC) |
| `0x0148` | 1 | ROM size |
| `0x0149` | 1 | RAM size |
| `0x014A` | 1 | Destination code |
| `0x014B` | 1 | Old licensee code |
| `0x014C` | 1 | ROM version |
| `0x014D` | 1 | Header checksum |
| `0x014E-0x014F` | 2 | Global checksum |

### CGB Flag (0x0143)
```
0x80 = CGB compatible (works on DMG too)
0xC0 = CGB only
Other = DMG only
```

### Cartridge Type (0x0147)
```
0x00 = ROM only (MBC0)
0x01 = MBC1
0x02 = MBC1 + RAM
0x03 = MBC1 + RAM + Battery
0x05 = MBC2
0x06 = MBC2 + Battery
0x0F = MBC3 + Timer + Battery
0x10 = MBC3 + Timer + RAM + Battery
0x11 = MBC3
0x12 = MBC3 + RAM
0x13 = MBC3 + RAM + Battery
0x19 = MBC5
0x1A = MBC5 + RAM
0x1B = MBC5 + RAM + Battery
0x1C = MBC5 + Rumble
0x1D = MBC5 + Rumble + RAM
0x1E = MBC5 + Rumble + RAM + Battery
```

### ROM Size (0x0148)
```
0x00 =  32 KB (2 banks)
0x01 =  64 KB (4 banks)
0x02 = 128 KB (8 banks)
0x03 = 256 KB (16 banks)
0x04 = 512 KB (32 banks)
0x05 =   1 MB (64 banks)
0x06 =   2 MB (128 banks)
0x07 =   4 MB (256 banks)
0x08 =   8 MB (512 banks)

Formula: 32 KB << value = 2 << value banks
```

### RAM Size (0x0149)
```
0x00 = None
0x01 = 2 KB (unofficial, treated as 8 KB)
0x02 = 8 KB (1 bank)
0x03 = 32 KB (4 banks)
0x04 = 128 KB (16 banks)
0x05 = 64 KB (8 banks)
```

---

## MBC Register Ranges

### MBC1
```
0x0000-0x1FFF: RAM enable (write 0x0A to enable)
0x2000-0x3FFF: ROM bank (lower 5 bits)
0x4000-0x5FFF: RAM bank / ROM bank upper bits
0x6000-0x7FFF: Banking mode (0=ROM, 1=RAM)
```

### MBC2
```
0x0000-0x3FFF: RAM enable (bit 8 of address = 0) / ROM bank (bit 8 = 1)
               RAM enable: write 0x0A, ROM bank: lower 4 bits
```

### MBC3
```
0x0000-0x1FFF: RAM/RTC enable (write 0x0A to enable)
0x2000-0x3FFF: ROM bank (7 bits, 0 maps to 1)
0x4000-0x5FFF: RAM bank (0-3) or RTC register (0x08-0x0C)
0x6000-0x7FFF: Latch clock data (write 0x00 then 0x01)
```

### MBC5
```
0x0000-0x1FFF: RAM enable (write 0x0A to enable)
0x2000-0x2FFF: ROM bank low 8 bits
0x3000-0x3FFF: ROM bank bit 8
0x4000-0x5FFF: RAM bank (4 bits)
```

---

## CPU Flags

```
Bit 7: Z (Zero)       - Set if result is zero
Bit 6: N (Subtract)   - Set if last op was subtraction
Bit 5: H (Half-carry) - Set if carry from bit 3 to 4
Bit 4: C (Carry)      - Set if carry from bit 7

Flag masks:
Z = 0b10000000 = 0x80
N = 0b01000000 = 0x40
H = 0b00100000 = 0x20
C = 0b00010000 = 0x10
```

---

## Initial Register Values

### DMG (Classic Game Boy)
```
A = 0x01    F = 0xB0 (Z=1, N=0, H=1, C=1)
B = 0x00    C = 0x13
D = 0x00    E = 0xD8
H = 0x01    L = 0x4D
SP = 0xFFFE
PC = 0x0100
```

### CGB (Color Game Boy)
```
A = 0x11    F = 0x80 (Z=1, N=0, H=0, C=0)
B = 0x00    C = 0x00
D = 0xFF    E = 0x56 (or 0x08 for ColorAsClassic)
H = 0x00    L = 0x0D (or 0x7C for ColorAsClassic)
SP = 0xFFFE
PC = 0x0100
```

---

## Audio Constants

### Duty Cycle Patterns
```
Duty 0 (12.5%): 0 0 0 0 0 0 0 1  = _______-
Duty 1 (25%):   1 0 0 0 0 0 0 1  = -______-
Duty 2 (50%):   1 0 0 0 0 1 1 1  = -____---
Duty 3 (75%):   0 1 1 1 1 1 1 0  = _------_
```

### Frequency Calculation
```
Square channels (1 & 2):
  Frequency (Hz) = 131072 / (2048 - freq_reg)
  Period (cycles) = (2048 - freq_reg) × 4

Wave channel (3):
  Frequency (Hz) = 65536 / (2048 - freq_reg)
  Period (cycles) = (2048 - freq_reg) × 2

Noise channel (4):
  Frequency = 524288 / divisor / 2^(shift+1)
  Divisor = [8, 16, 32, 48, 64, 80, 96, 112] (based on lower 3 bits)
```

### Noise LFSR
```
15-bit LFSR (normal):  Feedback bit = XOR of bits 0 and 1
7-bit LFSR (short):    Also writes to bit 6

Width select (NR43 bit 3):
  0 = 15-bit (sounds like white noise)
  1 = 7-bit  (sounds more tonal/buzzy)
```

---

## DMG Palette Values

```
DMG monochrome shades (0xFF47, 0xFF48, 0xFF49):
  Bits 1-0: Color for index 0
  Bits 3-2: Color for index 1
  Bits 5-4: Color for index 2
  Bits 7-6: Color for index 3

Color values:
  0 = White      (255, 255, 255) or 0xFF
  1 = Light gray (192, 192, 192) or 0xC0
  2 = Dark gray  (96, 96, 96)    or 0x60
  3 = Black      (0, 0, 0)       or 0x00
```

---

## CGB Color Conversion

```
CGB uses 15-bit RGB (5 bits per channel):
  Bit 0-4:   Red   (0-31)
  Bit 5-9:   Green (0-31)
  Bit 10-14: Blue  (0-31)

Gamma-corrected conversion (from Gambatte):
  R_out = (r × 13 + g × 2 + b) / 2
  G_out = (g × 3 + b) × 2
  B_out = (r × 3 + g × 2 + b × 11) / 2
```

---

## OAM Sprite Attributes

Each sprite uses 4 bytes in OAM (0xFE00-0xFE9F):
```
Byte 0: Y position (screen_y = value - 16)
Byte 1: X position (screen_x = value - 8)
Byte 2: Tile index
Byte 3: Attributes

Attribute bits (DMG):
  Bit 7: Priority (0=Above BG, 1=Behind BG colors 1-3)
  Bit 6: Y flip
  Bit 5: X flip
  Bit 4: Palette (0=OBP0, 1=OBP1)

Attribute bits (CGB):
  Bit 7: Priority
  Bit 6: Y flip
  Bit 5: X flip
  Bit 4: Unused
  Bit 3: VRAM bank
  Bit 2-0: Palette number (0-7)
```

---

## HDMA Transfer

### HDMA Registers (CGB only)
```
0xFF51: HDMA1 - Source high byte
0xFF52: HDMA2 - Source low byte (lower 4 bits ignored)
0xFF53: HDMA3 - Destination high byte (only bits 4-0 used, OR'd with 0x80)
0xFF54: HDMA4 - Destination low byte (lower 4 bits ignored)
0xFF55: HDMA5 - Length/Mode/Start

HDMA5 write:
  Bit 7: Mode (0=GDMA, 1=HDMA)
  Bit 6-0: Length (blocks - 1, each block is 16 bytes)

HDMA5 read:
  Bit 7: 0=Active, 1=Inactive
  Bit 6-0: Remaining blocks - 1

Transfer sizes:
  Minimum: 16 bytes (length = 0)
  Maximum: 2048 bytes (length = 0x7F)

GDMA: Transfers all data immediately (~8 cycles per 16 bytes)
HDMA: Transfers 16 bytes per HBlank
```

---

## Game Boy Printer

```
Magic bytes: 0x88, 0x33 (packet header)

Commands:
  0x01 = Initialize
  0x02 = Print
  0x04 = Data (compressed or raw)
  0x0F = Status

Packet format:
  Bytes 0-1: Magic (0x88, 0x33)
  Byte 2:    Command
  Byte 3:    Compression (0=None, 1=RLE)
  Bytes 4-5: Data length (little-endian)
  Bytes 6-N: Data
  Bytes N+1, N+2: Checksum (little-endian)
```

---

## Quick Reference Values

```c
// Commonly used masks and values
#define RAM_ENABLE_VALUE    0x0A
#define STAT_MODE_MASK      0x03
#define INTERRUPT_VBLANK    0x01
#define INTERRUPT_STAT      0x02
#define INTERRUPT_TIMER     0x04
#define INTERRUPT_SERIAL    0x08
#define INTERRUPT_JOYPAD    0x10

// Frame timing
#define CYCLES_PER_FRAME    70224
#define CYCLES_PER_LINE     456
#define VISIBLE_LINES       144
#define TOTAL_LINES         154
#define VBLANK_LINES        10

// CPU frequencies
#define CPU_FREQ_DMG        4194304
#define CPU_FREQ_CGB_DOUBLE 8388608

// Screen
#define SCREEN_WIDTH        160
#define SCREEN_HEIGHT       144
#define TILES_PER_ROW       20
#define TILES_PER_COL       18
```
