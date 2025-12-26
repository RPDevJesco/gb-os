# UEFI Bootloader in Pure Rust

A minimal UEFI bootloader written entirely in Rust with **zero external dependencies**. No `uefi` crate, no `uefi-services`, just raw UEFI protocol definitions and manual ABI handling.

## Features

- **Console I/O**: Text output with colors, keyboard input
- **Memory Management**: Page allocation, pool allocation, memory map enumeration
- **File System**: Read files from the boot partition (FAT32)
- **Graphics**: GOP framebuffer access with basic drawing primitives
- **Boot Services**: Full access to UEFI boot services
- **Runtime Services**: System reset (reboot/shutdown)
- **Interactive Menu**: Built-in boot menu for testing

## Project Structure

```
uefi-bootloader/
├── Cargo.toml
├── build.sh                 # Build and run script
├── x86_64-uefi.json        # Custom target (optional)
└── src/
    ├── main.rs             # Entry point and boot menu
    ├── uefi/
    │   ├── mod.rs          # Module root
    │   ├── types.rs        # Core UEFI types (GUID, STATUS, etc.)
    │   ├── tables.rs       # System Table, Boot/Runtime Services
    │   └── protocols.rs    # Console, Graphics, File System protocols
    ├── console.rs          # print!/println! macros, input handling
    ├── memory.rs           # Page/pool allocation, memory map
    ├── fs.rs               # File system operations
    └── graphics.rs         # GOP framebuffer, basic drawing
```

## Building

### Prerequisites

```bash
# Install Rust nightly (required for abi_efiapi feature)
rustup default nightly

# Add UEFI target
rustup target add x86_64-unknown-uefi
```

### Build

```bash
# Build the bootloader
cargo build --release --target x86_64-unknown-uefi

# Or use the build script
chmod +x build.sh
./build.sh build
```

The resulting binary will be at:
```
target/x86_64-unknown-uefi/release/uefi-bootloader.efi
```

## Running

### With QEMU

```bash
# Install QEMU and OVMF
# Debian/Ubuntu:
sudo apt install qemu-system-x86 ovmf

# Fedora:
sudo dnf install qemu-system-x86 edk2-ovmf

# Run
./build.sh run
```

### On Real Hardware

1. Format a USB drive with FAT32
2. Create directory structure: `EFI/BOOT/`
3. Copy `uefi-bootloader.efi` to `EFI/BOOT/BOOTX64.EFI`
4. Boot from the USB drive with UEFI enabled

## Architecture

### UEFI Entry Point

The bootloader entry point follows the UEFI calling convention:

```rust
#[no_mangle]
pub extern "efiapi" fn efi_main(
    image_handle: EFI_HANDLE,
    system_table: *mut EFI_SYSTEM_TABLE,
) -> EFI_STATUS
```

### Memory Layout

```
┌─────────────────────────────────────┐
│          UEFI Firmware              │
├─────────────────────────────────────┤
│       Runtime Services Code         │  ← Preserved after ExitBootServices
├─────────────────────────────────────┤
│       Runtime Services Data         │  ← Preserved after ExitBootServices
├─────────────────────────────────────┤
│         Boot Services Code          │  ← Reclaimed after ExitBootServices
├─────────────────────────────────────┤
│         Boot Services Data          │  ← Reclaimed after ExitBootServices
├─────────────────────────────────────┤
│           Loader Code               │  ← Our bootloader
├─────────────────────────────────────┤
│           Loader Data               │  ← Our allocations
├─────────────────────────────────────┤
│        Conventional Memory          │  ← Free for kernel use
└─────────────────────────────────────┘
```

### Boot Process

1. **Initialization**
   - Receive `EFI_SYSTEM_TABLE` from firmware
   - Initialize console for output
   - Disable watchdog timer

2. **Interactive Menu**
   - Display boot options
   - Handle user input

3. **Kernel Loading**
   - Open file system from boot partition
   - Read kernel file into memory
   - Parse ELF/PE headers (if implemented)

4. **Exit Boot Services**
   - Get final memory map
   - Call `ExitBootServices()`
   - No more UEFI boot services available!

5. **Transfer to Kernel**
   - Set up machine state
   - Jump to kernel entry point
   - Pass boot information structure

## Key UEFI Concepts

### GUIDs

Every UEFI protocol is identified by a 128-bit GUID:

```rust
pub const EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID: EFI_GUID = EFI_GUID::new(
    0x9042a9de, 0x23dc, 0x4a38,
    [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a]
);
```

### Status Codes

All UEFI functions return an `EFI_STATUS`:

```rust
pub const EFI_SUCCESS: EFI_STATUS = 0;
pub const EFI_NOT_FOUND: EFI_STATUS = 14;
// High bit set = error
pub const EFI_ERROR_BIT: EFI_STATUS = 1 << 63;
```

### Calling Convention

UEFI uses the Microsoft x64 calling convention:
- First 4 args: RCX, RDX, R8, R9
- Return value: RAX
- Caller cleans stack
- 16-byte stack alignment

Rust handles this with `extern "efiapi"`.

## Extending the Bootloader

### Adding a New Protocol

1. Add the GUID to `types.rs`:
```rust
pub const MY_PROTOCOL_GUID: EFI_GUID = EFI_GUID::new(...);
```

2. Define the protocol structure in `protocols.rs`:
```rust
#[repr(C)]
pub struct MY_PROTOCOL {
    pub function_a: unsafe extern "efiapi" fn(...) -> EFI_STATUS,
    // ...
}
```

3. Locate the protocol using boot services:
```rust
let mut protocol: *mut c_void = core::ptr::null_mut();
((*boot_services).locate_protocol)(
    &MY_PROTOCOL_GUID,
    core::ptr::null_mut(),
    &mut protocol,
);
```

### Loading a Real Kernel

To load an ELF kernel:

1. Parse the ELF header to find:
   - Entry point address
   - Program headers (loadable segments)

2. Allocate pages at the correct virtual addresses

3. Copy segments from file to memory

4. Set up page tables (for higher-half kernels)

5. Pass boot info:
```rust
struct BootInfo {
    framebuffer: FramebufferInfo,
    memory_map: *const MemoryDescriptor,
    memory_map_entries: usize,
    rsdp_address: u64,  // ACPI root
}
```

## Why No Dependencies?

External UEFI crates are excellent, but building from scratch provides:

1. **Complete Understanding**: Know exactly what every byte does
2. **Minimal Binary Size**: Only include what you need
3. **Learning Experience**: Deep dive into UEFI specification
4. **Full Control**: No hidden abstractions or magic

## Resources

- [UEFI Specification](https://uefi.org/specifications)
- [OSDev Wiki: UEFI](https://wiki.osdev.org/UEFI)
- [rust-osdev/uefi-rs](https://github.com/rust-osdev/uefi-rs) - For comparison
- [Intel TianoCore](https://www.tianocore.org/) - Reference implementation

## License

MIT License - Use freely for your own OS projects!

---

*Built with ❤️ and zero dependencies*
