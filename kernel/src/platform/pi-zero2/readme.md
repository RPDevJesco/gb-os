# Pi Zero 2 W Memory Layout

## Hardware Specifications

| Component | Specification |
|-----------|---------------|
| SoC | BCM2710A1 |
| CPU | Quad-core Cortex-A53 @ 1GHz |
| RAM | 512MB LPDDR2 |
| Peripheral Base | 0x3F000000 |

## Memory Allocation

| Region | Size | Purpose |
|--------|------|---------|
| GPU | 32MB | VideoCore firmware, framebuffer |
| Kernel | 10MB | Code, stacks, DMA, page tables |
| Heap | 470MB | Dynamic allocation, payloads |

## Physical Address Map

```
Address         Size      Region
──────────────────────────────────────────────────────────────
0x0000_0000     512KB     Reserved (GPU vectors, ARM stubs)
                          ⚠️  DO NOT USE
──────────────────────────────────────────────────────────────
0x0008_0000     2MB       Kernel code (.text, .rodata, .data, .bss)
                          ← kernel8.img loaded here by GPU firmware
0x0028_0000     256KB     CPU Stacks (64KB × 4 cores)
                          Core 0: 0x0028_0000 - 0x0028_FFFF (top: 0x0029_0000)
                          Core 1: 0x0029_0000 - 0x0029_FFFF (top: 0x002A_0000)
                          Core 2: 0x002A_0000 - 0x002A_FFFF (top: 0x002B_0000)
                          Core 3: 0x002B_0000 - 0x002B_FFFF (top: 0x002C_0000)
0x002C_0000     256KB     DMA Buffers
                          - Mailbox: 0x002C_0000 (256 bytes)
                          - EMMC:    0x002C_1000 (64KB)
                          - Pool:    0x002D_1000+
0x0030_0000     1MB       Page Tables (for MMU if enabled)
                          - L1 (TTBR0): 0x0030_0000 (4KB)
                          - L2 tables:  0x0030_1000+
0x0040_0000     6.5MB     Reserved (kernel growth space)
──────────────────────────────────────────────────────────────
0x00A8_0000     470MB     HEAP
                          ← All dynamic allocations
0x1E00_0000     ────      ARM memory ends
──────────────────────────────────────────────────────────────
0x1E00_0000     32MB      GPU Memory (not accessible to ARM)
                          - VideoCore firmware
                          - Framebuffer (allocated via mailbox)
0x2000_0000     ────      Physical RAM ends (512MB)
──────────────────────────────────────────────────────────────
0x3F00_0000     16MB      Peripheral I/O (MMIO)
0x4000_0000     ────      ARM Local Peripherals
```

## Linker Symbols

### Kernel
- `__kernel_start` - Start of kernel code (0x0008_0000)
- `__kernel_end` - End of kernel region (0x00A8_0000)
- `__bss_start`, `__bss_end` - BSS section (zero on boot)

### Stacks
- `_stack_top_core0` through `_stack_top_core3` - Stack tops (grow down)
- `_stack_bottom_core0` through `_stack_bottom_core3` - Stack limits
- `_stack_top` - Alias for core 0 stack top

### DMA
- `__dma_start`, `__dma_end` - DMA region bounds
- `_mailbox_buffer` - 256-byte mailbox buffer (16-byte aligned)
- `_emmc_buffer` - 64KB EMMC DMA buffer

### Page Tables
- `__pgtbl_start`, `__pgtbl_end` - Page table region
- `_ttbr0` - L1 translation table (4KB aligned)
- `_l2_tables` - L2 tables start

### Heap
- `_heap_start` - Heap base (0x00A8_0000)
- `_heap_end` - Heap limit (0x1E00_0000)

## Bus Address Conversion

The VideoCore GPU uses different bus addresses to access RAM:

| ARM Physical | VC Bus (Uncached) | VC Bus (Cached) |
|--------------|-------------------|-----------------|
| 0x0XXX_XXXX | 0xCXXX_XXXX | 0x4XXX_XXXX |

**For DMA operations, always use uncached addresses (0xC...):**
```rust
fn phys_to_bus(addr: usize) -> u32 {
    (addr as u32) | 0xC000_0000
}
```

## config.txt Settings

```ini
arm_64bit=1
gpu_mem=32
enable_uart=1
core_freq=250
```

## Files

| File | Description |
|------|-------------|
| `memory_map.rs` | Rust constants and helpers |
| `linker.ld` | Linker script with memory regions |
| `config.txt` | GPU firmware configuration |
