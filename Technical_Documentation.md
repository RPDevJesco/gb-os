# GB-OS: Complete Technical Documentation

> **Version:** 0.0.6 
> **Last Updated:** December 2025 
> **Project:** Bare-Metal Game Boy Color Emulator

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Architecture Overview](#2-architecture-overview)
3. [Boot System](#3-boot-system)
4. [Kernel Core](#4-kernel-core)
5. [Memory Management](#5-memory-management)
6. [Hardware Abstraction Layer](#6-hardware-abstraction-layer)
7. [Device Drivers](#7-device-drivers)
8. [Storage Subsystem](#8-storage-subsystem)
9. [Graphics Pipeline](#9-graphics-pipeline)
10. [Game Boy Emulator Core](#10-game-boy-emulator-core)
11. [Overlay System](#11-overlay-system)
12. [Event Chains Framework](#12-event-chains-framework)
13. [Build System](#13-build-system)
14. [Memory Map](#14-memory-map)
15. [Performance Analysis](#15-performance-analysis)
16. [Known Limitations](#16-known-limitations)
17. [Future Directions](#17-future-directions)
18. [Complete File Reference](#18-complete-file-reference)

---

## 1. Executive Summary

**GB-OS** (also known as RetroFutureGB) is a **bare-metal Game Boy Color emulator** written entirely in Rust. It boots directly on x86 hardware without an operating system, running in VGA Mode 13h (320×200×256 colors).

### Key Characteristics

| Feature | Implementation |
|---------|----------------|
| **Execution Model** | Pure bare-metal (no OS, no BIOS runtime) |
| **Target Architecture** | x86 (32-bit protected mode) |
| **Graphics** | VGA Mode 13h with double buffering and VSync |
| **Emulation** | Full Game Boy Color with accurate color rendering |
| **Boot Media** | Floppy, CD-ROM (El Torito), USB/HDD |
| **Filesystem** | FAT32 read-only for ROM loading |
| **Save System** | Persistent SRAM via ATA/IDE storage |
| **Special Features** | Real-time Pokémon game overlay system |

### Design Philosophy

- **Complete OS from scratch** — bootloader, memory management, device drivers
- **Accurate emulation** — Full Game Boy / Game Boy Color compatibility
- **Hardware integration** — VGA, keyboard, ATA storage, FAT32 filesystem
- **Production features** — Save games, overlay system, flicker-free rendering
- **Clean architecture** — Modular Rust code with `no_std` constraints
- **No external dependencies** — Everything implemented from scratch

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            GB-OS Architecture                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────┐   ┌───────────────────┐   ┌───────────────────────┐  │
│  │    Bootloader    │ → │   Kernel Entry    │ → │  Emulator Main Loop   │  │
│  │    (ASM)         │   │   (Rust)          │   │                       │  │
│  └──────────────────┘   └───────────────────┘   └───────────────────────┘  │
│          │                      │                         │                │
│          v                      v                         v                │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                    Hardware Abstraction Layer                         │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────────┐  │  │
│  │  │   arch/    │  │  drivers/  │  │  storage/  │  │   graphics/    │  │  │
│  │  │   x86      │  │            │  │            │  │                │  │  │
│  │  │  GDT/IDT   │  │  Keyboard  │  │  PCI/ATA   │  │ VGA Mode 13h   │  │  │
│  │  │  PIC/PIT   │  │  VGA Text  │  │  FAT32     │  │ Double Buffer  │  │  │
│  │  │  I/O       │  │  Mouse     │  │  Savefile  │  │ Palette Mgmt   │  │  │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│          │                                                                  │
│          v                                                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                       Game Boy Emulator Core                          │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────────┐ │  │
│  │  │   CPU   │  │   GPU   │  │   MMU   │  │   MBC   │  │   Timer     │ │  │
│  │  │ LR35902 │  │   PPU   │  │ Memory  │  │ 0,1,2,  │  │   Serial    │ │  │
│  │  │         │  │ 160x144 │  │   Map   │  │ 3,5     │  │   Keypad    │ │  │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────────┘ │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│          │                                                                  │
│          v                                                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                          Overlay System                               │  │
│  │  • Pokémon Gen 1/2 RAM reading    • Three-panel layout               │  │
│  │  • HP bars, stats, moves          • Map/badge display                │  │
│  │  • Dirty region tracking          • Battle mode detection            │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Layered Architecture

| Layer | Purpose | Components |
|-------|---------|------------|
| **Hardware Abstraction** | Architecture-specific code | x86 GDT, IDT, PIT, I/O operations |
| **Device Drivers** | Hardware communication | ATA/IDE, keyboard, PCI enumeration |
| **Filesystem Layer** | Storage abstraction | FAT32 read-only implementation |
| **Emulation Core** | Game Boy hardware emulation | CPU, GPU, MMU, MBC implementations |
| **Graphics Pipeline** | Display management | VGA palette management, double buffering |
| **Overlay System** | Game-specific features | Pokémon RAM reading, real-time stats |

---

## 3. Boot System

GB-OS uses a **two-stage bootloader** that supports multiple boot media and handles the transition from 16-bit real mode to 32-bit protected mode.

### 3.1 Stage 1 Bootloader

**File:** `boot/boot.asm`  
**Size:** 512 bytes (single sector)  
**Load Address:** 0x7C00

#### Purpose

Minimal boot sector that detects boot media and loads Stage 2.

#### Boot Sequence

1. **Initialization**
   ```asm
   start:
       cli                     ; Disable interrupts
       xor ax, ax
       mov ds, ax
       mov es, ax
       mov ss, ax
       mov sp, 0x7C00          ; Stack below bootloader
       sti                     ; Re-enable interrupts
   ```

2. **Media Detection**
   - `DL < 0x80` → Floppy disk (CHS addressing via INT 13h)
   - `DL ≥ 0x80` → HDD/CD/USB (LBA addressing via INT 13h Extensions)
   
   ```asm
   cmp dl, 0x80
   jb .floppy
   mov ah, 0x41              ; Check LBA extensions
   mov bx, 0x55AA
   int 0x13
   ```

3. **Stage 2 Loading**
   - Loads 32 sectors (16KB) starting at sector 1
   - Destination: 0x7E00
   - Validates magic bytes "GR" (0x5247) at start of stage 2

4. **DAP Structure** (for LBA addressing)
   ```asm
   dap:        db 0x10, 0        ; Size, reserved
               dw 1              ; Sector count
   dap_buf:    dw STAGE2, 0      ; Buffer segment:offset
   dap_lba:    dd 0, 0           ; 64-bit LBA
   ```

#### Boot Info Structure (at 0x0500)

Populated by Stage 1 for Stage 2:
- Boot drive number
- Boot media type (0=Floppy, 1=CD, 2=HDD)

### 3.2 Stage 2 Bootloader

**File:** `boot/stage2.asm`  
**Size:** ~16KB  
**Load Address:** 0x7E00

#### Purpose

Extended bootloader that prepares the system for kernel execution.

#### Boot Sequence

1. **Hardware Initialization**
   ```asm
   call query_e820           ; Query BIOS memory map
   call enable_a20           ; Enable A20 line for >1MB access
   mov ax, 0x0013            ; Set VGA Mode 13h
   int 0x10
   ```

2. **Kernel Loading**
   - Loads 256KB kernel from disk
   - Initial destination: Low memory (temporary)
   - Final destination: 1MB (0x100000)

3. **ROM Loading** (optional)
   - If ROM embedded in image, loads to 3MB (0x300000)
   - Supports up to 2MB ROM size

4. **Protected Mode Transition**
   ```asm
   lgdt [gdt_descriptor]     ; Load GDT
   mov eax, cr0
   or al, 1                  ; Set PE bit
   mov cr0, eax
   jmp 0x08:protected_entry  ; Far jump to 32-bit code
   ```

5. **Memory Relocation**
   - Copies kernel to final address (1MB)
   - Copies ROM to final address (3MB) if present

#### Boot Info Structure (passed to kernel at 0x0500)

| Offset | Size | Field |
|--------|------|-------|
| 0x00 | 4 | Magic ('GBOY' = 0x594F4247) |
| 0x04 | 4 | E820 map address |
| 0x08 | 4 | VGA mode (0x13) |
| 0x0C | 4 | Framebuffer address (0xA0000) |
| 0x10 | 4 | Screen width (320) |
| 0x14 | 4 | Screen height (200) |
| 0x18 | 4 | Bits per pixel (8) |
| 0x1C | 4 | Pitch (320) |
| 0x20 | 4 | ROM address (0x300000 if loaded, else 0) |
| 0x24 | 4 | ROM size in bytes |
| 0x28 | 32 | ROM title (null-terminated) |
| 0x48 | 4 | Boot media type |
| 0x4C | 4 | Boot drive number |

#### GDT Configuration

| Selector | Description |
|----------|-------------|
| 0x00 | Null descriptor |
| 0x08 | Code segment: base=0, limit=4GB, 32-bit, execute/read |
| 0x10 | Data segment: base=0, limit=4GB, 32-bit, read/write |

---

## 4. Kernel Core

### 4.1 Entry Point

**File:** `kernel/src/main.rs`

#### Attributes

```rust
#![no_std]                    // No standard library
#![no_main]                   // Custom entry point
extern crate alloc;           // Heap allocation support
```

#### Assembly Entry

```rust
global_asm!(
    ".section .text.boot",
    ".global _start",
    "_start:",
    "    mov edi, 0xA0640",   // Draw progress pixels
    "    mov al, 0x0F",
    "    mov ecx, 10",
    "1:  stosb",
    "    loop 1b",
    "    mov esp, 0x90000",   // Set up stack at ~576KB
    "    push eax",           // Save boot_info pointer
    "    call kernel_main",
    "    cli",
    "    hlt",
);
```

#### Initialization Sequence

```rust
fn kernel_main(boot_info_ptr: *const BootInfo) {
    // 1. Parse boot info from 0x500
    let boot_info = unsafe { BootInfo::from_ptr(0x500 as *const u8) };
    
    // 2. Initialize GDT
    gdt::init();
    
    // 3. Initialize IDT
    idt::init();
    
    // 4. Initialize memory manager (heap, PMM)
    mm::init(boot_info.e820_map_addr);
    
    // 5. Initialize stack guard
    defensive::init_stack_guard();
    
    // 6. Initialize storage (PCI → ATA → FAT32)
    storage::init();
    
    // 7. Enable interrupts
    unsafe { asm!("sti"); }
    
    // 8. Enter ROM browser or emulation loop
    // ...
}
```

### 4.2 Boot Info Parsing

**File:** `kernel/src/boot_info.rs`

Parses the boot info structure passed by the bootloader at address 0x500.

### 4.3 Defensive Programming

**File:** `kernel/src/defensive.rs`

Comprehensive safety primitives for bare-metal development:

#### Operation Tracking

```rust
#[repr(u32)]
pub enum OperationId {
    None = 0,
    BootStart = 1,
    GdtInit = 2,
    IdtInit = 3,
    PicInit = 4,
    HeapInit = 5,
    AtaInit = 6,
    Fat32Mount = 7,
    RomLoad = 8,
    EmulatorInit = 9,
    FrameStart = 10,
    CpuCycle = 11,
    GpuRender = 12,
    VgaBlit = 13,
    KeyboardPoll = 14,
    FrameEnd = 15,
}

pub fn set_last_operation(op: OperationId) {
    LAST_OPERATION.store(op as u32, Ordering::Relaxed);
}
```

#### Memory Validation

```rust
pub fn is_safe_memory_range(addr: usize, len: usize) -> bool {
    // Validates against known memory map:
    // - VGA buffer: 0xA0000-0xAFFFF
    // - ROM data: 0x300000-0x500000
    // - Heap: 0x1000000-0x1400000
}
```

#### Stack Overflow Detection

```rust
pub fn check_stack_overflow() -> bool {
    let current_sp: u32;
    unsafe { asm!("mov {}, esp", out(reg) current_sp); }
    current_sp < STACK_GUARD_VALUE
}
```

#### Diagnostic Panic Handler

```rust
pub fn diagnostic_panic(info: &core::panic::PanicInfo) -> ! {
    unsafe { core::arch::asm!("cli"); }
    let diag = take_diagnostic_snapshot();
    draw_panic_screen(&diag, info);
    loop { unsafe { core::arch::asm!("hlt"); } }
}
```

---

## 5. Memory Management

**Directory:** `kernel/src/mm/`

### 5.1 Heap Allocator

**File:** `kernel/src/mm/heap.rs`

#### Configuration

| Parameter | Value |
|-----------|-------|
| Start Address | 0x01000000 (16MB) |
| Size | 0x00400000 (4MB) |
| Type | Simple bump allocator |

#### Implementation

```rust
unsafe impl GlobalAlloc for SimpleBumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let alloc_start = Self::align_up(current, layout.align());
        let alloc_end = alloc_start + layout.size();
        if alloc_end > HEAP_END { return ptr::null_mut(); }
        *next_ptr = alloc_end;
        alloc_start as *mut u8
    }
    
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't free - adequate for static allocation patterns
    }
}
```

### 5.2 Physical Memory Manager

**File:** `kernel/src/mm/pmm.rs`

#### Architecture

Pooled intrusive free list with statically allocated page frames.

#### Page Frame Structure

```rust
pub struct PageFrame {
    free_node: IntrusiveNode,  // Embedded list node
    flags: PageFlags,          // FREE, RESERVED, KERNEL, DMA
    ref_count: u16,
}
```

#### Memory Regions

| Pages | Address Range | Usage |
|-------|---------------|-------|
| < 256 | 0 - 1MB | Reserved for BIOS/bootloader |
| 256-512 | 1MB - 2MB | Kernel code/data |
| > 512 | > 2MB | Available for allocation |

#### Free List Implementation

```rust
static FREE_LIST: IntrusiveStack<PageFrame, fn(&PageFrame) -> &IntrusiveNode> = 
    IntrusiveStack::new(|pf| &pf.free_node);
```

### 5.3 Intrusive Data Structures

**File:** `kernel/src/mm/intrusive.rs`

Zero-allocation linked lists where nodes are embedded directly in data structures.

#### Core Components

1. **IntrusiveNode** — Embeddable node with next/prev pointers
   ```rust
   pub struct IntrusiveNode {
       next: Option<NonNull<IntrusiveNode>>,
       prev: Option<NonNull<IntrusiveNode>>,
   }
   ```

2. **IntrusiveList<T, N>** — Doubly-linked list with accessor function
   ```rust
   pub struct IntrusiveList<T, N: Fn(&T) -> &IntrusiveNode> {
       head: Option<NonNull<T>>,
       tail: Option<NonNull<T>>,
       len: usize,
       node_accessor: N,
   }
   ```

3. **IntrusiveStack<T, N>** — LIFO stack (used by PMM free list)

4. **IntrusiveQueue<T, N>** — FIFO queue (used by scheduler)

#### Accessor Pattern

Uses Rust generics instead of C macros:

```rust
// Linux kernel (C):
container_of(ptr, type, member)  // Pointer arithmetic + offsetof()

// GB-OS (Rust):
Fn(&T) -> &IntrusiveNode         // Type-safe closure-based access
```

#### Helper Macro

```rust
macro_rules! node_accessor {
    ($type:ty, $field:ident) => {
        |item: &$type| &item.$field
    };
}
```

#### Current Limitations

| Aspect | Current State | Production-Ready |
|--------|---------------|------------------|
| `node_to_container` | Assumes node at offset 0 | Would need `offset_of!` |
| Iteration | No iterators, only `front()`/`back()`/`pop_*()` | Missing `list_for_each_entry` |
| Node position | Must be first field in struct | Limits layout flexibility |

---

## 6. Hardware Abstraction Layer

**Directory:** `kernel/src/arch/x86/`

### 6.1 Global Descriptor Table (GDT)

**File:** `kernel/src/arch/x86/gdt.rs`

#### Segment Layout

| Selector | Description |
|----------|-------------|
| 0x00 | Null descriptor |
| 0x08 | Kernel code (Ring 0) |
| 0x10 | Kernel data (Ring 0) |
| 0x18 | User code (Ring 3) |
| 0x20 | User data (Ring 3) |
| 0x28 | TSS (reserved) |

All segments use flat memory model (base=0, limit=4GB).

### 6.2 Interrupt Descriptor Table (IDT)

**File:** `kernel/src/arch/x86/idt.rs`

- 256 interrupt gates
- CPU exceptions: 0-31
- Hardware IRQs: 32-47

### 6.3 Programmable Interrupt Controller (PIC)

**File:** `kernel/src/arch/x86/pic.rs`

#### Configuration

| PIC | IRQs | Interrupt Range |
|-----|------|-----------------|
| Master | 0-7 | INT 32-39 |
| Slave | 8-15 | INT 40-47 |

#### Emulator-Optimized Setup

```rust
pub fn init_for_emulator() {
    // Only enable needed IRQs:
    // - IRQ 0: Timer (PIT)
    // - IRQ 1: Keyboard
    set_mask(0b11111100, 0b11111111);
}
```

### 6.4 Programmable Interval Timer (PIT)

**File:** `kernel/src/arch/x86/pit.rs`

- **Frequency:** 1000 Hz (1ms resolution)
- **Purpose:** Frame timing (~59.7 fps)

### 6.5 Port I/O

**File:** `kernel/src/arch/x86/io.rs`

Low-level `inb`/`outb`/`inl`/`outl` functions for hardware communication.

---

## 7. Device Drivers

**Directory:** `kernel/src/drivers/`

### 7.1 PS/2 Keyboard

**File:** `kernel/src/drivers/keyboard.rs`

Interrupt-driven PS/2 keyboard driver with scan code translation.

### 7.2 VGA Text Mode

**File:** `kernel/src/drivers/vga.rs`

Debug output via VGA text mode (80x25).

### 7.3 PS/2 Mouse

**File:** `kernel/src/drivers/mouse.rs`

PS/2 mouse driver.

### 7.4 Synaptics Touchpad

**File:** `kernel/src/drivers/synaptics.rs`

Synaptics touchpad support.

### 7.5 Hardware Constants

**File:** `kernel/src/drivers/armada_e500_hw.rs`

Hardware constants for Compaq Armada E500 (development hardware).

---

## 8. Storage Subsystem

**Directory:** `kernel/src/storage/`

### 8.1 PCI Enumeration

**File:** `kernel/src/storage/pci.rs`

#### Configuration Space Access

```rust
pub fn pci_config_read(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let address = 0x80000000 
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    outl(0xCF8, address);
    inl(0xCFC)
}
```

#### IDE Controller Detection

- Class: 0x01 (Mass Storage)
- Subclass: 0x01 (IDE)

### 8.2 ATA/IDE Driver

**File:** `kernel/src/storage/ata.rs`

#### I/O Ports

```rust
pub mod primary {
    pub const DATA: u16 = 0x1F0;
    pub const ERROR: u16 = 0x1F1;
    pub const SECTOR_COUNT: u16 = 0x1F2;
    pub const LBA_LOW: u16 = 0x1F3;
    pub const LBA_MID: u16 = 0x1F4;
    pub const LBA_HIGH: u16 = 0x1F5;
    pub const DRIVE_HEAD: u16 = 0x1F6;
    pub const STATUS: u16 = 0x1F7;
    pub const COMMAND: u16 = 0x1F7;
}
```

#### Commands

| Command | Value | Description |
|---------|-------|-------------|
| READ SECTORS | 0x20 | PIO read |
| WRITE SECTORS | 0x30 | PIO write |
| IDENTIFY | 0xEC | Device identification |

#### Error Handling

- Polling with timeouts to prevent infinite loops
- Returns `Result<T, &'static str>` for fallible operations
- Proper BSY/DRQ status checking

#### Timeout Constants

```rust
const TIMEOUT_BSY: u32 = 100_000;
const TIMEOUT_DRQ: u32 = 100_000;
const TIMEOUT_IDENTIFY: u32 = 500_000;
```

### 8.3 FAT32 Filesystem

**File:** `kernel/src/storage/fat32.rs`

#### Features

- Read-only implementation (eliminates corruption risks)
- MBR-partitioned and raw VBR support
- Cluster chain traversal with termination checks
- .GB and .GBC file detection

#### Boot Sector Validation

```rust
// Check signature
if sector[510] != 0x55 || sector[511] != 0xAA {
    return Err("Invalid signature");
}

// Validate BPB parameters
if bytes_per_sector < 512 || bytes_per_sector > 4096 { return Err("Bad BPS"); }
if sectors_per_cluster == 0 { return Err("Bad SPC"); }
if num_fats == 0 { return Err("Bad FATs"); }
if root_cluster < 2 { return Err("Bad root"); }
```

#### ROM Discovery

```rust
// Check extension - case insensitive (.GB or .GBC)
let ext0 = sector[offset + 8].to_ascii_uppercase();
let ext1 = sector[offset + 9].to_ascii_uppercase();
let ext2 = sector[offset + 10].to_ascii_uppercase();
let is_gb = ext0 == b'G' && ext1 == b'B' && (ext2 == b' ' || ext2 == b'C');
```

#### Limitations

- No long filename (LFN) support — only 8.3 format
- Root directory only — no subdirectory traversal
- Limited to 16 ROM files per scan
- No directory caching

### 8.4 Save File System

**File:** `kernel/src/storage/savefile.rs`

#### Disk Layout

Save area starts at sector 0x10000 (32MB offset):

| Slot | Sectors | Size |
|------|---------|------|
| 0 | 0x10000-0x1003F | 32KB |
| 1 | 0x10040-0x1007F | 32KB |
| ... | ... | ... |
| 15 | 0x103C0-0x103FF | 32KB |

#### Save Header Structure

```rust
pub struct SaveHeader {
    magic: [u8; 4],         // "GBSV"
    version: u8,            // Format version
    rom_name: [u8; 16],     // ROM title for matching
    ram_size: u32,          // SRAM size in bytes
    checksum: u32,          // FNV-1a hash of ROM name
}
```

#### ROM Name Hashing

```rust
fn hash_rom_name(name: &str) -> u32 {
    let mut hash: u32 = 0x811c9dc5; // FNV-1a offset basis
    for byte in name.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193); // FNV prime
    }
    hash
}
```

#### Debounced Save Mechanism

```rust
pub struct SaveTracker {
    frames_since_write: u32,
    pending_save: bool,
}

const SAVE_DEBOUNCE_FRAMES: u32 = 120;  // ~2 seconds at 60fps

impl SaveTracker {
    pub fn tick(&mut self, device: &mut Device) -> bool {
        if device.check_and_reset_ram_updated() {
            self.frames_since_write = 0;
            self.pending_save = true;
            return false;
        }
        
        if self.pending_save {
            self.frames_since_write += 1;
            if self.frames_since_write >= SAVE_DEBOUNCE_FRAMES {
                self.pending_save = false;
                return true;  // Trigger save now
            }
        }
        false
    }
}
```

---

## 9. Graphics Pipeline

**Directory:** `kernel/src/graphics/`

### 9.1 VGA Mode 13h

**File:** `kernel/src/graphics/vga_mode13h.rs`

#### Configuration

| Parameter | Value |
|-----------|-------|
| Resolution | 320×200 |
| Color Depth | 8-bit (256 colors) |
| Framebuffer | 0xA0000 (64KB) |
| Memory Model | Linear |

#### Why Mode 13h?

- Simple linear framebuffer — no bank switching
- No VESA/GOP complexity
- Perfect for 160×144 Game Boy screen (scales cleanly)
- Direct palette control via VGA DAC

### 9.2 Palette Management

**File:** `kernel/src/graphics/vga_palette.rs`

#### VGA DAC Programming

The VGA DAC uses 6-bit color channels (0-63), while Game Boy Color uses 5-bit (0-31).

```rust
pub fn sync_gbc_bg_palettes(gbc_palettes: &[u8]) {
    // Program VGA DAC with GBC palette values
    // GBC: 5-bit per channel (0-31)
    // VGA: 6-bit per channel (0-63)
    // Conversion: vga_val = gbc_val * 2
}

pub fn sync_gbc_sprite_palettes(gbc_palettes: &[u8]) {
    // Similar for sprite palettes
}

pub fn sync_dmg_palettes(bgp: u8, obp0: u8, obp1: u8) {
    // DMG (original Game Boy) uses 4-shade grayscale
}
```

#### Palette Layout (256 VGA colors)

| Index Range | Usage |
|-------------|-------|
| 0-31 | Background palette 0 |
| 32-63 | Background palette 1 |
| ... | ... |
| 224-255 | Sprite palette 7 |

### 9.3 Double Buffering

**File:** `kernel/src/graphics/double_buffer.rs`

#### Architecture

```
┌─────────────────┐     VSync Wait     ┌─────────────────┐
│   Back Buffer   │ ──────────────────→│   VGA Memory    │
│   (RAM 64KB)    │     Fast Copy      │   (0xA0000)     │
└─────────────────┘                    └─────────────────┘
        ↑                                      │
        │ Render                               │ Display
        │                                      ↓
   ┌─────────┐                           ┌─────────┐
   │ Emulator│                           │ Monitor │
   │   GPU   │                           │         │
   └─────────┘                           └─────────┘
```

#### VSync Wait

```rust
fn wait_vsync() {
    unsafe {
        // Wait for any current retrace to end
        while (inb(VGA_STATUS_REG) & 0x08) != 0 {
            core::hint::spin_loop();
        }
        // Wait for next retrace to start
        while (inb(VGA_STATUS_REG) & 0x08) == 0 {
            core::hint::spin_loop();
        }
    }
}
```

#### Buffer Flip

```rust
fn copy_to_vga() {
    unsafe {
        core::ptr::copy_nonoverlapping(
            BACK_BUFFER.as_ptr(),
            VGA_BUFFER,
            BUFFER_SIZE  // 64KB
        );
    }
}

pub fn flip_vsync() {
    wait_vsync();
    copy_to_vga();
}
```

#### Game Boy Screen Blit

```rust
pub fn blit_gb_to_backbuffer(pal_data: &[u8]) {
    let buffer = back_buffer();
    
    for y in 0..GB_HEIGHT {  // 144 lines
        let src_offset = y * GB_WIDTH;  // 160 pixels
        let dst_offset = (GB_Y + y) * SCREEN_WIDTH + GB_X;
        
        buffer[dst_offset..dst_offset + GB_WIDTH]
            .copy_from_slice(&pal_data[src_offset..src_offset + GB_WIDTH]);
    }
}
```

---

## 10. Game Boy Emulator Core

**Directory:** `kernel/src/gameboy/`

### 10.1 Device Wrapper

**File:** `kernel/src/gameboy/device.rs`

High-level emulator interface:

```rust
pub struct Device {
    cpu: CPU,
    // GPU, MMU, etc. are accessed through CPU
}

impl Device {
    pub fn new_cgb(rom_data: Vec<u8>, skip_boot: bool) -> StrResult<Self>;
    pub fn do_cycle(&mut self) -> u32;
    pub fn keydown(&mut self, key: KeypadKey);
    pub fn keyup(&mut self, key: KeypadKey);
    pub fn check_and_reset_gpu_updated(&mut self) -> bool;
    pub fn check_and_reset_ram_updated(&mut self) -> bool;
    pub fn get_pal_data(&self) -> &[u8];
    pub fn get_cbgpal(&self) -> &[u8];
    pub fn get_csprit(&self) -> &[u8];
    pub fn mode(&self) -> GbMode;
    pub fn romname(&self) -> String;
    pub fn ram_is_battery_backed(&self) -> bool;
}
```

### 10.2 CPU (LR35902)

**File:** `kernel/src/gameboy/cpu.rs`

#### Features

- Complete LR35902 instruction set
- Proper cycle timing
- HALT bug handling
- Interrupt handling with priorities

#### Instruction Execution

```rust
pub fn do_cycle(&mut self) -> u32 {
    self.update_ime();
    let ticks = self.handle_interrupts();
    if ticks > 0 { return ticks; }
    
    if self.halted { return 4; }
    
    let opcode = self.fetchbyte();
    self.execute(opcode)
}

fn execute(&mut self, opcode: u8) -> u32 {
    match opcode {
        0x00 => 4,  // NOP
        0x01 => { let v = self.fetchword(); self.reg.set_bc(v); 12 }  // LD BC,nn
        0x76 => {   // HALT
            self.halted = true;
            if !self.ime && (self.mmu.inte & self.mmu.intf) != 0 {
                self.halt_bug = true;
            }
            4
        }
        // ... 256 opcodes + CB prefix
    }
}
```

### 10.3 GPU (PPU)

**File:** `kernel/src/gameboy/gpu.rs`

#### Scanline Rendering

```rust
pub fn do_cycle(&mut self, ticks: u32) {
    if !self.lcd_on { return; }
    
    self.modeclock += ticks;
    
    // Full line takes 456 ticks
    if self.modeclock >= 456 {
        self.modeclock -= 456;
        self.line = (self.line + 1) % 154;
        self.check_interrupt_lyc();
        
        // VBlank starts at line 144
        if self.line >= 144 && self.mode != 1 {
            self.change_mode(1);
        }
    }
}
```

#### Mode Transitions

| Mode | Duration | Description |
|------|----------|-------------|
| 2 | 80 cycles | OAM search |
| 3 | 168-291 cycles | Pixel transfer |
| 0 | 85-208 cycles | HBlank |
| 1 | 4560 cycles | VBlank (10 lines) |

### 10.4 Memory Management Unit (MMU)

**File:** `kernel/src/gameboy/mmu.rs`

Memory map handling, I/O registers, and component dispatching.

### 10.5 Memory Bank Controllers (MBC)

**Directory:** `kernel/src/gameboy/mbc/`

#### MBC Trait

```rust
pub trait MBC: Send {
    fn readrom(&self, addr: u16) -> u8;
    fn readram(&self, addr: u16) -> u8;
    fn writerom(&mut self, addr: u16, value: u8);
    fn writeram(&mut self, addr: u16, value: u8);
    fn check_and_reset_ram_updated(&mut self) -> bool;
    fn is_battery_backed(&self) -> bool;
    fn loadram(&mut self, ramdata: &[u8]) -> StrResult<()>;
    fn dumpram(&self) -> Vec<u8>;
    fn romname(&self) -> String;
}
```

#### Supported MBCs

| File | Cartridge Type | Description |
|------|----------------|-------------|
| `mbc0.rs` | 0x00 | No MBC (32KB ROMs) |
| `mbc1.rs` | 0x01-0x03 | Most common, bank switching |
| `mbc2.rs` | 0x05-0x06 | Built-in 512×4 bit RAM |
| `mbc3.rs` | 0x0F-0x13 | RTC support |
| `mbc5.rs` | 0x19-0x1E | GBC standard, 8MB ROM support |

#### MBC Selection

```rust
match data[0x147] {
    0x00 => mbc0::MBC0::new(data),
    0x01..=0x03 => mbc1::MBC1::new(data),
    0x05..=0x06 => mbc2::MBC2::new(data),
    0x0F..=0x13 => mbc3::MBC3::new(data),
    0x19..=0x1E => mbc5::MBC5::new(data),
    _ => Err("Unsupported MBC type"),
}
```

### 10.6 Other Components

| File | Purpose |
|------|---------|
| `register.rs` | CPU register file |
| `keypad.rs` | Joypad emulation |
| `timer.rs` | Timer/DIV registers |
| `serial.rs` | Serial port (stub) |
| `gbmode.rs` | DMG/CGB mode detection |
| `display.rs` | Display scaling |
| `input.rs` | Input mapping |

---

## 11. Overlay System

**Directory:** `kernel/src/overlay/`

Real-time Pokémon game information display.

### 11.1 Game Detection

```rust
pub enum Game {
    Red, Blue, Yellow,      // Gen 1
    Gold, Silver, Crystal,  // Gen 2
    Unknown,
}

impl Game {
    pub fn detect(rom_name: &str) -> Self {
        let upper = rom_name.to_uppercase();
        if upper.contains("RED") { Game::Red }
        else if upper.contains("BLUE") { Game::Blue }
        else if upper.contains("YELLOW") { Game::Yellow }
        else if upper.contains("GOLD") { Game::Gold }
        else if upper.contains("SILVER") { Game::Silver }
        else if upper.contains("CRYSTAL") { Game::Crystal }
        else { Game::Unknown }
    }
}
```

### 11.2 RAM Addresses

**File:** `kernel/src/overlay/ram_layout.rs`

#### Gen 1 (Red/Blue/Yellow)

| Address | Data |
|---------|------|
| 0xD158 | Player name |
| 0xD34A | Rival name |
| 0xD347 | Money (BCD) |
| 0xD356 | Badges |
| 0xD163 | Party count |
| 0xD16B | Party data |
| 0xD35E | Map ID |
| 0xD057 | Battle type |
| 0xCFE5 | Enemy Pokémon |

#### Gen 2 (Gold/Silver/Crystal)

| Address | Data |
|---------|------|
| 0xD47D | Player name |
| 0xD84E | Money |
| 0xD57C | Johto badges |
| 0xD57D | Kanto badges |
| 0xDCD7 | Party count |
| 0xDCDF | Party data |

### 11.3 Screen Layout

**File:** `kernel/src/overlay/game_overlay.rs`

```
+--------+------------------+--------+
| LEFT   |                  | RIGHT  |
| PANEL  |    GAME BOY      | PANEL  |
| x<80   |     SCREEN       | x>240  |
|        |    80-240        |        |
| Map    |                  | Player |
| Badges |                  | Name   |
| Lead   |                  | Money  |
| Pokemon|                  | Bag    |
| -Moves |                  |        |
| -Stats |                  | Enemy  |
|        |                  |(battle)|
+--------+------------------+--------+
|        | BOTTOM: Party HP bars     |
+--------+---------------------------+
```

### 11.4 Dirty Region Tracking

**File:** `kernel/src/overlay/dirty_region.rs`

Optimizes rendering by only updating changed elements:

```rust
pub struct DirtyTracker {
    prev_trainer_info: TrainerInfo,
    prev_party_state: [PokemonState; 6],
    prev_battle_state: Option<BattleState>,
}

impl DirtyTracker {
    pub fn check_trainer_dirty(&mut self, current: &TrainerInfo) -> bool {
        if *current != self.prev_trainer_info {
            self.prev_trainer_info = current.clone();
            true
        } else {
            false
        }
    }
}
```

**Performance Impact:** Up to **90% CPU savings** during stable gameplay.

### 11.5 Lookup Tables

| File | Contents |
|------|----------|
| `pokemon_names.rs` | 251 Pokémon names (Gen 1+2) |
| `move_names.rs` | 251 move names |
| `move_pp.rs` | Base PP values, PP Up calculation |
| `catch_rate.rs` | Catch rates (3-255), tier classification |
| `map_names.rs` | Location names (Gen 1: byte, Gen 2: group+number) |
| `item_names.rs` | Item names |

---

## 12. Event Chains Framework

**Directory:** `kernel/src/event_chains/`

A `no_std`-compatible event processing pipeline for complex workflows.

### 12.1 Design Philosophy

- **NOT for hot paths** — memory allocation, scheduler, interrupt handlers
- **FOR complex workflows** — driver initialization, syscall handling
- Fixed-capacity arrays instead of `Vec`
- No heap allocation in core operations

### 12.2 Core Components

#### ChainableEvent Trait

```rust
pub trait ChainableEvent {
    fn execute(&self, ctx: &mut EventContext) -> EventResult;
    fn name(&self) -> &'static str;
}
```

#### EventContext

Fixed-capacity key-value store (max 32 entries):

```rust
pub struct EventContext {
    entries: [(Option<&'static str>, Option<ContextValue>); 32],
    count: usize,
}

pub enum ContextValue {
    U32(u32),
    I32(i32),
    Bool(bool),
    Str(&'static str),
    // ... common types
}
```

#### EventChain

```rust
pub struct EventChain {
    events: [Option<Box<dyn ChainableEvent>>; 16],    // Max 16 events
    middleware: [Option<Box<dyn EventMiddleware>>; 8], // Max 8 middleware
    fault_mode: FaultMode,
}
```

#### Fault Tolerance Modes

```rust
pub enum FaultMode {
    Strict,     // Stop on any failure
    Lenient,    // Continue on all failures, collect them
    BestEffort, // Continue on event failures, stop on middleware failures
}
```

### 12.3 Execution Model

- **Events:** FIFO order (first added = first executed)
- **Middleware:** LIFO order (last added = first executed, wraps events)

### 12.4 Built-in Middleware

| Middleware | Purpose |
|------------|---------|
| `LoggingMiddleware` | Logs event execution |
| `PermissionMiddleware` | Checks privilege rings |
| `AuditMiddleware` | Records audit entries |
| `TimingMiddleware` | Measures execution time |
| `RetryMiddleware` | Retries failed events |

### 12.5 Usage in Codebase

#### Driver Initialization

**File:** `kernel/src/drivers/init.rs`

Fallback hierarchy with graceful degradation:

```rust
// Tries: ATI Rage GPU → VESA fallback
// Tries: Synaptics touchpad → PS/2 mouse fallback
```

#### Syscall Handler

**File:** `kernel/src/syscall/mod.rs`

Middleware pipeline for logging, auditing, and permission checking.

---

## 13. Build System

### 13.1 Docker Build (Recommended)

**Files:** `Dockerfile`, `docker-build.sh`

```bash
# Build Docker image
docker build -t gb-os-builder .

# Build without ROM
docker run --rm -v $(pwd)/output:/output gb-os-builder

# Build with embedded ROM
docker run --rm -v $(pwd)/output:/output \
    -v /path/to/game.gb:/input/game.gb:ro \
    gb-os-builder
```

### 13.2 Build Script

**File:** `build.sh`

#### Build Stages

1. **Assemble Bootloader**
   ```bash
   nasm -f bin -o build/boot.bin boot/boot.asm
   nasm -f bin -o build/stage2.bin boot/stage2.asm
   ```

2. **Build Kernel**
   ```bash
   cd kernel
   cargo +nightly build --release --target ../i686-rustacean.json \
       -Z build-std=core,alloc
   ```

3. **Convert ELF to Binary**
   ```bash
   objcopy -O binary target/.../rustacean build/kernel.bin
   ```

4. **Create Floppy Image**
   ```bash
   dd if=/dev/zero of=build/gameboy-system.img bs=512 count=2880
   dd if=build/boot.bin of=build/gameboy-system.img bs=512 count=1 conv=notrunc
   dd if=build/stage2.bin of=build/gameboy-system.img bs=512 seek=1 conv=notrunc
   dd if=build/kernel.bin of=build/gameboy-system.img bs=512 seek=33 conv=notrunc
   ```

5. **Create ISO Image**
   ```bash
   xorriso -as mkisofs -o build/gameboy-system.iso \
       -V "GAMEBOY" -b boot/boot.img -no-emul-boot \
       -boot-load-size 4 -boot-info-table iso/
   ```

### 13.3 Makefile Targets

```makefile
make gameboy      # Build GameBoy edition
make run-gb       # Run in QEMU (floppy)
make run-gb-cd    # Run in QEMU (CD-ROM)
make docker       # Build via Docker
make clean        # Remove build artifacts
```

### 13.4 Target Specification

**File:** `i686-rustacean.json`

```json
{
    "llvm-target": "i686-unknown-none",
    "data-layout": "e-m:e-p:32:32-...",
    "arch": "x86",
    "target-endian": "little",
    "target-pointer-width": "32",
    "os": "none",
    "executables": true,
    "linker-flavor": "ld.lld",
    "linker": "rust-lld",
    "panic-strategy": "abort",
    "disable-redzone": true,
    "features": "+soft-float"
}
```

### 13.5 Linker Script

**File:** `kernel/linker.ld`

```ld
OUTPUT_FORMAT("elf32-i386")
ENTRY(_start)

SECTIONS {
    . = 0x100000;           /* Kernel at 1MB */
    
    .text : ALIGN(4K) {
        *(.text.boot)       /* Entry point MUST be first */
        *(.text .text.*)
    }
    
    .rodata : ALIGN(4K) { *(.rodata .rodata.*) }
    .data : ALIGN(4K) { *(.data .data.*) }
    .bss : ALIGN(4K) {
        __bss_start = .;
        *(.bss .bss.*)
        __bss_end = .;
    }
}
```

### 13.6 Output Files

```
output/
├── gameboy-system.img   # 1.44MB floppy image
├── gameboy-system.iso   # CD-ROM image (El Torito)
├── kernel.bin           # Raw kernel binary
└── mkgamedisk           # ROM embedding tool
```

---

## 14. Memory Map

```
Address         Size      Contents
────────────────────────────────────────────────────────────────
0x00000000      4KB       Real Mode IVT + BIOS Data Area
0x00000500      72B       Boot info structure
0x00001000      ~2KB      E820 memory map
0x00007C00      512B      Stage 1 bootloader
0x00007E00      16KB      Stage 2 bootloader
0x00020000      256KB     Kernel (temporary load location)
0x00040000      2MB       ROM (temporary load location)
0x00090000      64KB      Stack (grows down from 0x90000)
0x000A0000      64KB      VGA framebuffer
0x00100000      256KB     Kernel (final location @ 1MB)
0x00300000      2MB       ROM (final location @ 3MB)
0x01000000      4MB       Heap (16MB - 20MB)
```

---

## 15. Performance Analysis

### 15.1 Emulation Loop

- **Target:** 59.7275 Hz (16.75ms per frame)
- **Cycles per frame:** 70,224
- **Timer resolution:** 1ms (PIT at 1000 Hz)

```rust
const CYCLES_PER_FRAME: u32 = 70224;
const TICKS_PER_FRAME: u32 = 17;  // ~60 FPS

loop {
    // Run CPU cycles
    let mut cycles = 0;
    while cycles < CYCLES_PER_FRAME {
        cycles += device.do_cycle();
    }
    
    // Frame timing
    let target = last_frame_ticks.wrapping_add(TICKS_PER_FRAME);
    while pit::ticks().wrapping_sub(target) > 0x8000_0000 {
        unsafe { asm!("hlt"); }
    }
}
```

### 15.2 Graphics Pipeline

#### Performance Characteristics

| Operation | Size | Frequency | Notes |
|-----------|------|-----------|-------|
| GB screen blit | 23KB | Every GPU update | Could track dirty scanlines |
| VSync flip | 64KB | Every frame | Fixed cost |
| Palette sync | ~192B | When GPU updates | Conditional |
| Overlay render | Variable | When data changes | Dirty tracking |

#### Identified Bottleneck

The 23KB blit copies 160×144 bytes every frame. **Optimization opportunity:** Track dirty scanlines in GPU and only copy changed regions.

### 15.3 Dirty Region Tracking

The overlay system achieves **up to 90% CPU savings** during stable gameplay by:

1. Caching previous frame's data
2. Comparing current vs. cached state
3. Only redrawing changed regions
4. Using per-element dirty flags

### 15.4 Design Trade-offs

| Decision | Benefits | Trade-offs |
|----------|----------|------------|
| **Bare-metal** | Complete control, predictable timing | No memory protection, single bug crashes system |
| **VGA Mode 13h** | Simple framebuffer, direct palette control | 256 colors, low resolution |
| **Polling I/O** | Simple, debuggable | CPU cycles in busy-wait |
| **Read-only FS** | No corruption risk | Can't create files |
| **Bump allocator** | Simple, no fragmentation | No memory reclamation |

---

## 16. Known Limitations

### 16.1 Emulator

| Feature | Status |
|---------|--------|
| **Audio** | Not implemented (no APU) |
| **MBC6, MBC7, HuC1, HuC3** | Unsupported |
| **Link cable** | Serial port exists, no multiplayer |
| **CGB double-speed** | Implemented but not thoroughly tested |
| **PPU timing** | May not be cycle-accurate for edge cases |

### 16.2 Storage

| Limitation | Impact |
|------------|--------|
| 8.3 filenames only | No LFN support |
| Root directory only | No subdirectories |
| 16 ROMs max | Per filesystem scan |
| 16 save slots | Overwrites slot 0 when full |
| 32MB save offset | No partition table awareness |

### 16.3 Boot

| Feature | Status |
|---------|--------|
| UEFI | Planned but not implemented |
| 256KB kernel limit | Stage 2 limitation |
| 2MB ROM limit | Memory buffer size |
| No compression | Raw binary loading |

### 16.4 Error Handling

- Error messages are generic string literals
- No retry logic for transient hardware failures
- Timeout values are hardcoded magic numbers

---

## 17. Future Directions

### 17.1 ARM Port (Pi Zero 2 W)

**Target Hardware:**
- Raspberry Pi Zero 2 W (quad-core Cortex-A53)
- Waveshare RP2040-PiZero board
- 1.3-inch LCD Game Console HAT
- GamePi20 handheld kit

**Benefits:**
- Eliminates x86 boot complexity
- Direct framebuffer access
- Cleaner bare-metal development

### 17.2 Multi-Platform Strategy

| Platform | Status | Notes |
|----------|--------|-------|
| x86 Legacy BIOS | ✅ Complete | Current implementation |
| UEFI | 🔄 Planned | Simplifies hardware access |
| ARM64 | 🔄 Planned | Pi Zero 2 W, Allwinner H618 |
| ARM32 | 🔄 Planned | Broader compatibility |

### 17.3 Recommended Improvements

1. **Dirty scanline tracking** in GPU to optimize blitting
2. **More specific error types** beyond string literals
3. **APU implementation** for audio support
4. **Complete UEFI boot** for modern systems
5. **Retry logic** for transient hardware failures
6. **LFN support** for long filenames
7. **Directory traversal** for ROM organization

---

## 18. Complete File Reference

### 18.1 Boot System

| File | Lines | Purpose |
|------|-------|---------|
| `boot/boot.asm` | ~150 | Stage 1 bootloader (512 bytes) |
| `boot/stage2.asm` | ~600 | Stage 2 bootloader (~16KB) |

### 18.2 Kernel Core

| File | Purpose |
|------|---------|
| `kernel/src/main.rs` | Entry point, emulation loop |
| `kernel/src/boot_info.rs` | Boot info structure parsing |
| `kernel/src/defensive.rs` | Safety checks, panic handling |
| `kernel/src/rom_browser.rs` | ROM selection UI |
| `kernel/src/lib.rs` | Crate root |
| `kernel/Cargo.toml` | Dependencies (none external) |
| `kernel/linker.ld` | Linker script |

### 18.3 Architecture (x86)

| File | Purpose |
|------|---------|
| `kernel/src/arch/x86/mod.rs` | Module definitions |
| `kernel/src/arch/x86/gdt.rs` | Global Descriptor Table |
| `kernel/src/arch/x86/idt.rs` | Interrupt Descriptor Table |
| `kernel/src/arch/x86/pic.rs` | 8259 PIC driver |
| `kernel/src/arch/x86/pit.rs` | Programmable Interval Timer |
| `kernel/src/arch/x86/io.rs` | Port I/O (inb/outb) |

### 18.4 Memory Management

| File | Purpose |
|------|---------|
| `kernel/src/mm/mod.rs` | Module definitions, init |
| `kernel/src/mm/heap.rs` | Bump allocator (4MB @ 16MB) |
| `kernel/src/mm/pmm.rs` | Physical memory manager |
| `kernel/src/mm/intrusive.rs` | Zero-allocation linked lists |

### 18.5 Device Drivers

| File | Purpose |
|------|---------|
| `kernel/src/drivers/mod.rs` | Module definitions |
| `kernel/src/drivers/keyboard.rs` | PS/2 keyboard driver |
| `kernel/src/drivers/vga.rs` | VGA text mode (debug) |
| `kernel/src/drivers/mouse.rs` | PS/2 mouse driver |
| `kernel/src/drivers/synaptics.rs` | Synaptics touchpad |
| `kernel/src/drivers/armada_e500_hw.rs` | Hardware constants |
| `kernel/src/drivers/init.rs` | Driver initialization chain |

### 18.6 Storage

| File | Purpose |
|------|---------|
| `kernel/src/storage/mod.rs` | Module definitions, init |
| `kernel/src/storage/pci.rs` | PCI enumeration |
| `kernel/src/storage/ata.rs` | ATA/IDE driver |
| `kernel/src/storage/fat32.rs` | FAT32 filesystem |
| `kernel/src/storage/savefile.rs` | Save game persistence |

### 18.7 Graphics

| File | Purpose |
|------|---------|
| `kernel/src/graphics/mod.rs` | Module definitions |
| `kernel/src/graphics/vga_mode13h.rs` | VGA primitives |
| `kernel/src/graphics/vga_palette.rs` | GBC palette management |
| `kernel/src/graphics/double_buffer.rs` | Flicker-free rendering |

### 18.8 GUI

| File | Purpose |
|------|---------|
| `kernel/src/gui/mod.rs` | Module definitions |
| `kernel/src/gui/layout.rs` | Screen layout constants |
| `kernel/src/gui/font_8x8.rs` | 8×8 bitmap font |
| `kernel/src/gui/font_4x6.rs` | Compact 4×6 font |

### 18.9 Game Boy Emulator

| File | Purpose |
|------|---------|
| `kernel/src/gameboy/mod.rs` | Module definitions |
| `kernel/src/gameboy/device.rs` | High-level emulator API |
| `kernel/src/gameboy/cpu.rs` | LR35902 CPU |
| `kernel/src/gameboy/gpu.rs` | PPU (160×144 rendering) |
| `kernel/src/gameboy/mmu.rs` | Memory Management Unit |
| `kernel/src/gameboy/register.rs` | CPU registers |
| `kernel/src/gameboy/keypad.rs` | Joypad emulation |
| `kernel/src/gameboy/timer.rs` | Timer/DIV registers |
| `kernel/src/gameboy/serial.rs` | Serial port (stub) |
| `kernel/src/gameboy/gbmode.rs` | DMG/CGB mode detection |
| `kernel/src/gameboy/display.rs` | Display scaling |
| `kernel/src/gameboy/input.rs` | Input mapping |

### 18.10 Memory Bank Controllers

| File | Purpose |
|------|---------|
| `kernel/src/gameboy/mbc/mod.rs` | MBC trait, selection |
| `kernel/src/gameboy/mbc/mbc0.rs` | No MBC (32KB ROMs) |
| `kernel/src/gameboy/mbc/mbc1.rs` | MBC1 (most common) |
| `kernel/src/gameboy/mbc/mbc2.rs` | MBC2 |
| `kernel/src/gameboy/mbc/mbc3.rs` | MBC3 (RTC support) |
| `kernel/src/gameboy/mbc/mbc5.rs` | MBC5 (GBC standard) |

### 18.11 Overlay System

| File | Purpose |
|------|---------|
| `kernel/src/overlay/mod.rs` | Module definitions, game detection |
| `kernel/src/overlay/ram_layout.rs` | Game RAM addresses |
| `kernel/src/overlay/game_overlay.rs` | Rendering logic |
| `kernel/src/overlay/dirty_region.rs` | Update optimization |
| `kernel/src/overlay/pokemon_names.rs` | Species names (251) |
| `kernel/src/overlay/move_names.rs` | Move names |
| `kernel/src/overlay/map_names.rs` | Location names |
| `kernel/src/overlay/item_names.rs` | Item names |
| `kernel/src/overlay/move_pp.rs` | PP calculations |
| `kernel/src/overlay/catch_rate.rs` | Catch rate data |

### 18.12 Event Chains

| File | Purpose |
|------|---------|
| `kernel/src/event_chains/mod.rs` | Module definitions |
| `kernel/src/event_chains/chain.rs` | EventChain, StaticChain |
| `kernel/src/event_chains/context.rs` | EventContext (key-value store) |
| `kernel/src/event_chains/middleware.rs` | Built-in middleware |
| `kernel/src/event_chains/result.rs` | EventResult type |

### 18.13 Syscall

| File | Purpose |
|------|---------|
| `kernel/src/syscall/mod.rs` | Syscall dispatcher |

### 18.14 Build System

| File | Purpose |
|------|---------|
| `Dockerfile` | Build environment |
| `docker-build.sh` | Docker build script |
| `build.sh` | Main build script |
| `Makefile` | Build targets |
| `i686-rustacean.json` | Rust target specification |

### 18.15 Statistics

| Metric | Value |
|--------|-------|
| **Total source files** | ~60 |
| **Rust code** | ~15,000 lines |
| **Assembly code** | ~2,000 lines |
| **External dependencies** | 0 |

---

## Appendix A: Main Emulation Loop

Complete reference implementation:

```rust
fn run_gameboy_emulator_with_rom(rom_ptr: *const u8, rom_size: usize) -> ! {
    // Initialize timing
    arch::x86::pit::set_frequency(1000);
    
    // Initialize graphics
    double_buffer::init();
    init_overlay();
    
    // Create emulator instance
    let rom_data = unsafe { slice::from_raw_parts(rom_ptr, rom_size).to_vec() };
    let mut device = Device::new_cgb(rom_data, false).unwrap();
    
    // Load existing save
    if device.ram_is_battery_backed() {
        let _ = savefile::load_sram(&mut device);
    }
    
    // Detect game for overlay
    let game = Game::detect(&device.romname());
    let input_state = InputState::new();
    let mut save_tracker = SaveTracker::new();
    
    // Frame timing constants
    const CYCLES_PER_FRAME: u32 = 70224;
    const TICKS_PER_FRAME: u32 = 17;
    let mut last_frame_ticks = pit::ticks();
    
    loop {
        set_last_operation(OperationId::FrameStart);
        
        // 1. Execute one frame of CPU cycles
        set_last_operation(OperationId::CpuCycle);
        let mut cycles = 0;
        while cycles < CYCLES_PER_FRAME {
            cycles += device.do_cycle();
        }
        
        // 2. Check for saves (debounced)
        if save_tracker.tick(&mut device) {
            let _ = savefile::save_sram(&device);
        }
        
        // 3. Render if GPU updated
        if device.check_and_reset_gpu_updated() {
            set_last_operation(OperationId::GpuRender);
            
            // Sync palettes to VGA DAC
            if device.mode() == GbMode::Color {
                vga_palette::sync_gbc_bg_palettes(device.get_cbgpal());
                vga_palette::sync_gbc_sprite_palettes(device.get_csprit());
            } else {
                let (palb, pal0, pal1) = device.get_dmg_palettes();
                vga_palette::sync_dmg_palettes(palb, pal0, pal1);
            }
            
            // Blit Game Boy screen to back buffer
            set_last_operation(OperationId::VgaBlit);
            double_buffer::blit_gb_to_backbuffer(device.get_pal_data());
            
            // Render overlay (if supported game)
            let reader = RamReader::new(device.mmu(), game);
            render_overlay_efficient(double_buffer::back_buffer(), &reader, game);
            
            // Flip with VSync
            double_buffer::flip_vsync();
        }
        
        // 4. Process keyboard input
        set_last_operation(OperationId::KeyboardPoll);
        while let Some(key) = keyboard::get_key() {
            if let Some(gb_key) = input_state.map_keycode(key.keycode) {
                if key.pressed { device.keydown(gb_key); }
                else { device.keyup(gb_key); }
            }
        }
        
        // 5. Frame timing (wait for next frame)
        set_last_operation(OperationId::FrameEnd);
        let target = last_frame_ticks.wrapping_add(TICKS_PER_FRAME);
        while pit::ticks().wrapping_sub(target) > 0x8000_0000 {
            unsafe { asm!("hlt"); }
        }
        last_frame_ticks = target;
    }
}
```

---

## Appendix B: Boot Process Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              POWER ON                                        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ BIOS loads MBR (512 bytes) to 0x7C00                                        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ STAGE 1 BOOTLOADER (boot/boot.asm)                                          │
│ ├── Disable interrupts, set up segments                                     │
│ ├── Detect boot media (DL register)                                         │
│ │   ├── DL < 0x80 → Floppy (CHS via INT 13h/02h)                           │
│ │   └── DL ≥ 0x80 → CD/HDD/USB (LBA via INT 13h/42h)                       │
│ ├── Load Stage 2 (32 sectors) to 0x7E00                                     │
│ ├── Verify magic bytes "GR"                                                 │
│ └── Jump to Stage 2                                                         │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ STAGE 2 BOOTLOADER (boot/stage2.asm)                                        │
│ ├── Query E820 memory map                                                   │
│ ├── Enable A20 line                                                         │
│ ├── Set VGA Mode 13h (320×200×256)                                          │
│ ├── Load kernel (256KB) to temporary location                               │
│ ├── Load ROM (optional) to temporary location                               │
│ ├── Set up GDT (flat memory model)                                          │
│ ├── Enable protected mode (set PE bit in CR0)                               │
│ ├── Copy kernel to 0x100000 (1MB)                                           │
│ ├── Copy ROM to 0x300000 (3MB)                                              │
│ ├── Build boot info structure at 0x500                                      │
│ └── Far jump to kernel entry point                                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ KERNEL ENTRY (kernel/src/main.rs :: _start)                                 │
│ ├── Draw progress pixels to VGA                                             │
│ ├── Set up stack at 0x90000                                                 │
│ └── Call kernel_main()                                                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ KERNEL INITIALIZATION (kernel_main)                                         │
│ ├── Parse boot info from 0x500                                              │
│ ├── Initialize GDT                                                          │
│ ├── Initialize IDT                                                          │
│ ├── Initialize memory manager (heap, PMM)                                   │
│ ├── Initialize stack guard                                                  │
│ ├── Initialize storage (PCI → ATA → FAT32)                                  │
│ ├── Enable interrupts                                                       │
│ └── Branch:                                                                 │
│     ├── ROM embedded? → Start emulation                                     │
│     └── No ROM? → Show ROM browser                                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ EMULATION LOOP                                                              │
│ ├── Execute 70,224 CPU cycles per frame                                     │
│ ├── Check/debounce save writes                                              │
│ ├── Sync palettes to VGA DAC                                                │
│ ├── Blit Game Boy screen to back buffer                                     │
│ ├── Render overlay (if Pokémon game)                                        │
│ ├── Flip buffers with VSync                                                 │
│ ├── Process keyboard input                                                  │
│ └── Wait for next frame (PIT timing)                                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*Document generated from comprehensive project analysis*  
*GB-OS v0.0.6 — December 2025*
