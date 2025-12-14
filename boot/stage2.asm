; ============================================================================
; stage2.asm - Stage 2 Bootloader for RetroFuture GB
; ============================================================================
;
; Clean, minimal bootloader for Game Boy emulation:
;   1. Query E820 memory map
;   2. Enable A20 line
;   3. Set VGA mode 13h (320x200x256) - perfect for 160x144 GB scaled 2x
;   4. Load kernel to 0x100000 (1MB)
;   5. Load ROM to 0x300000 (3MB) if present
;   6. Switch to 32-bit protected mode
;   7. Jump to kernel
;
; Assemble: nasm -f bin -o stage2.bin stage2.asm
; ============================================================================

[BITS 16]
[ORG 0x7E00]

; ============================================================================
; Magic Number (verified by stage 1)
; ============================================================================

dw 0x5247                       ; 'GR' magic (GameBoy Retro)

; ============================================================================
; Entry Point
; ============================================================================

stage2_entry:
    ; Save boot drive
    mov     [boot_drive], dl

    ; Display banner
    mov     si, msg_banner
    call    print_string

    ; ------------------------------------------------------------------------
    ; Step 1: Query E820 Memory Map
    ; ------------------------------------------------------------------------
    mov     si, msg_e820
    call    print_string

    call    query_e820
    jc      .e820_fail

    mov     si, msg_ok
    call    print_string
    jmp     .step2

.e820_fail:
    mov     si, msg_fail
    call    print_string
    jmp     halt

    ; ------------------------------------------------------------------------
    ; Step 2: Enable A20 Line
    ; ------------------------------------------------------------------------
.step2:
    mov     si, msg_a20
    call    print_string

    call    enable_a20
    call    verify_a20
    jc      .a20_fail

    mov     si, msg_ok
    call    print_string
    jmp     .step3

.a20_fail:
    mov     si, msg_fail
    call    print_string
    jmp     halt

    ; ------------------------------------------------------------------------
    ; Step 3: Set VGA Mode 13h (320x200, 256 colors)
    ; ------------------------------------------------------------------------
.step3:
    mov     si, msg_vga
    call    print_string

    ; Set VGA mode 13h
    mov     ax, 0x0013
    int     0x10

    ; Store video info for kernel
    mov     byte [vga_mode], 0x13
    mov     dword [fb_address], 0xA0000
    mov     word [fb_width], 320
    mov     word [fb_height], 200
    mov     byte [fb_bpp], 8
    mov     word [fb_pitch], 320

    mov     si, msg_ok
    call    print_string

    ; ------------------------------------------------------------------------
    ; Step 4: Load Kernel
    ; ------------------------------------------------------------------------
    mov     si, msg_kernel
    call    print_string

    call    load_kernel
    jc      .kernel_fail

    mov     si, msg_ok
    call    print_string
    jmp     .step5

.kernel_fail:
    mov     si, msg_fail
    call    print_string
    jmp     halt

    ; ------------------------------------------------------------------------
    ; Step 5: Load ROM (if present)
    ; ------------------------------------------------------------------------
.step5:
    mov     si, msg_rom
    call    print_string

    call    load_rom
    jc      .no_rom

    mov     si, msg_ok
    call    print_string
    jmp     .step6

.no_rom:
    mov     si, msg_none
    call    print_string
    ; ROM is optional, continue anyway

    ; ------------------------------------------------------------------------
    ; Step 6: Enter Protected Mode
    ; ------------------------------------------------------------------------
.step6:
    mov     si, msg_pmode
    call    print_string

    ; Disable interrupts
    cli

    ; Load GDT
    lgdt    [gdt_descriptor]

    ; Set PE bit in CR0
    mov     eax, cr0
    or      eax, 1
    mov     cr0, eax

    ; Far jump to 32-bit code (flushes pipeline, loads CS)
    jmp     0x08:protected_mode_entry

; ============================================================================
; Halt
; ============================================================================

halt:
    cli
.loop:
    hlt
    jmp     .loop

; ============================================================================
; E820 Memory Map Query
; ============================================================================

E820_BASE   equ 0x1000          ; Store map at 0x1000
E820_MAX    equ 64

query_e820:
    push    es
    push    di
    push    ebx
    push    ecx
    push    edx

    xor     ax, ax
    mov     es, ax
    mov     di, E820_BASE + 4   ; Leave space for entry count
    xor     ebx, ebx            ; Continuation = 0
    xor     bp, bp              ; Entry counter
    mov     edx, 0x534D4150     ; 'SMAP'

.loop:
    mov     eax, 0xE820
    mov     ecx, 24             ; 24-byte entries
    int     0x15

    jc      .done               ; Carry = end/error
    cmp     eax, 0x534D4150     ; Verify SMAP returned
    jne     .error

    add     di, 24
    inc     bp
    cmp     bp, E820_MAX
    jge     .done

    test    ebx, ebx            ; EBX=0 means last entry
    jnz     .loop

.done:
    mov     [E820_BASE], bp     ; Store count
    pop     edx
    pop     ecx
    pop     ebx
    pop     di
    pop     es
    clc
    ret

.error:
    pop     edx
    pop     ecx
    pop     ebx
    pop     di
    pop     es
    stc
    ret

; ============================================================================
; A20 Enable
; ============================================================================

enable_a20:
    ; Try BIOS first
    mov     ax, 0x2401
    int     0x15
    jnc     .done

    ; Keyboard controller method
    call    .wait_kbd
    mov     al, 0xAD            ; Disable keyboard
    out     0x64, al

    call    .wait_kbd
    mov     al, 0xD0            ; Read output port
    out     0x64, al

    call    .wait_kbd_data
    in      al, 0x60
    push    ax

    call    .wait_kbd
    mov     al, 0xD1            ; Write output port
    out     0x64, al

    call    .wait_kbd
    pop     ax
    or      al, 2               ; Set A20 bit
    out     0x60, al

    call    .wait_kbd
    mov     al, 0xAE            ; Re-enable keyboard
    out     0x64, al

    call    .wait_kbd

.done:
    ret

.wait_kbd:
    in      al, 0x64
    test    al, 2
    jnz     .wait_kbd
    ret

.wait_kbd_data:
    in      al, 0x64
    test    al, 1
    jz      .wait_kbd_data
    ret

verify_a20:
    push    es
    push    ds
    push    di
    push    si

    ; Test using 0x600/0x610 to avoid boot_info at 0x500
    xor     ax, ax
    mov     es, ax
    mov     di, 0x0600

    mov     ax, 0xFFFF
    mov     ds, ax
    mov     si, 0x0610

    mov     byte [es:di], 0x00
    mov     byte [ds:si], 0xFF

    cmp     byte [es:di], 0xFF
    je      .disabled

    pop     si
    pop     di
    pop     ds
    pop     es
    clc
    ret

.disabled:
    pop     si
    pop     di
    pop     ds
    pop     es
    stc
    ret

; ============================================================================
; Load Kernel (using LBA extensions - no floppy geometry)
; ============================================================================

KERNEL_SECTOR   equ 36          ; After boot(4 sectors for CD alignment) + stage2(32)
KERNEL_SECTORS  equ 200         ; 100KB kernel (94KB actual + headroom)
KERNEL_LOAD_SEG equ 0x2000      ; Load to 0x20000
KERNEL_LOAD_OFF equ 0x0000

load_kernel:
    push    es
    push    bp

    mov     word [sectors_left], KERNEL_SECTORS
    mov     dword [current_lba], KERNEL_SECTOR
    mov     word [load_segment], KERNEL_LOAD_SEG
    mov     word [load_offset], KERNEL_LOAD_OFF

.loop:
    cmp     word [sectors_left], 0
    je      .done

    ; Calculate how many sectors to read (max 64 at a time for safety)
    mov     ax, [sectors_left]
    cmp     ax, 64
    jbe     .count_ok
    mov     ax, 64
.count_ok:
    mov     [sectors_to_read], ax

    ; Set up DAP for LBA read
    mov     word [dap_count], ax
    mov     ax, [load_offset]
    mov     [dap_offset], ax
    mov     ax, [load_segment]
    mov     [dap_segment], ax
    mov     eax, [current_lba]
    mov     [dap_lba], eax
    mov     dword [dap_lba + 4], 0

    ; Perform LBA read with retry
    mov     bp, 3
.retry:
    mov     si, dap
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jnc     .read_ok

    ; Reset and retry
    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13
    dec     bp
    jnz     .retry
    jmp     .error

.read_ok:
    ; Progress dot (write to VGA memory since we're in mode 13h)
    push    es
    push    di
    mov     ax, 0xA000
    mov     es, ax
    mov     di, [sectors_left]      ; Use sector count as X position
    mov     byte [es:di], 0x0F      ; White pixel
    pop     di
    pop     es

    ; Update counters
    movzx   eax, word [sectors_to_read]
    sub     [sectors_left], ax
    add     [current_lba], eax

    ; Update load address (sectors_to_read * 32 paragraphs per sector)
    mov     ax, [sectors_to_read]
    shl     ax, 5                   ; * 32 (paragraphs per sector)
    add     [load_segment], ax

    jmp     .loop

.done:
    pop     bp
    pop     es
    clc
    ret

.error:
    pop     bp
    pop     es
    stc
    ret

; ============================================================================
; Load ROM (if present) - using LBA extensions
; ============================================================================
;
; ROM loading now supports larger ROMs without floppy size restrictions.
; The ROM can be embedded in the disk image at a specific sector, or
; the kernel can load it from a FAT16 partition at runtime.
;
; ROM Header format (at ROM_HEADER_SECTOR):
;   0x00: 'GBOY' magic (4 bytes)
;   0x04: ROM size in bytes (4 bytes, little-endian)
;   0x08: ROM title (32 bytes, null-terminated)
;   0x28: Reserved (padding to 512 bytes)
;
; ROM data follows immediately after the header sector.

ROM_HEADER_SECTOR equ 289       ; Where ROM header is stored
ROM_LOAD_SEG      equ 0x3000    ; Temporary load buffer at 0x30000
ROM_DEST_ADDR     equ 0x300000  ; Final ROM location at 3MB

load_rom:
    push    es
    push    bp

    ; Load ROM header sector using LBA
    mov     word [dap_count], 1
    mov     word [dap_offset], 0
    mov     word [dap_segment], ROM_LOAD_SEG
    mov     dword [dap_lba], ROM_HEADER_SECTOR
    mov     dword [dap_lba + 4], 0

    mov     si, dap
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jc      .no_rom

    ; Check for 'GBOY' magic at start of header
    mov     ax, ROM_LOAD_SEG
    mov     es, ax
    cmp     dword [es:0], 0x594F4247    ; 'GBOY'
    jne     .no_rom

    ; Get ROM size from header (offset 4, little-endian)
    mov     eax, [es:4]
    mov     [rom_size], eax

    ; Copy title (offset 8, 32 bytes)
    mov     si, 8
    mov     di, rom_title
    mov     cx, 32
.copy_title:
    mov     al, [es:si]
    mov     [di], al
    inc     si
    inc     di
    loop    .copy_title

    ; Calculate sectors needed for ROM
    mov     eax, [rom_size]
    add     eax, 511
    shr     eax, 9              ; Divide by 512
    mov     [rom_sectors], ax

    ; Load ROM data starting at sector after header
    mov     dword [current_lba], ROM_HEADER_SECTOR + 1
    mov     word [load_segment], ROM_LOAD_SEG
    mov     word [load_offset], 0
    mov     ax, [rom_sectors]
    mov     [sectors_left], ax

.load_loop:
    cmp     word [sectors_left], 0
    je      .load_done

    ; Calculate how many sectors to read (max 64 at a time)
    mov     ax, [sectors_left]
    cmp     ax, 64
    jbe     .rom_count_ok
    mov     ax, 64
.rom_count_ok:
    mov     [sectors_to_read], ax

    ; Set up DAP for LBA read
    mov     word [dap_count], ax
    mov     ax, [load_offset]
    mov     [dap_offset], ax
    mov     ax, [load_segment]
    mov     [dap_segment], ax
    mov     eax, [current_lba]
    mov     [dap_lba], eax
    mov     dword [dap_lba + 4], 0

    ; Perform LBA read with retry
    mov     bp, 3
.rom_retry:
    mov     si, dap
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jnc     .rom_read_ok

    ; Reset and retry
    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13
    dec     bp
    jnz     .rom_retry
    jmp     .no_rom

.rom_read_ok:
    ; Update counters
    movzx   eax, word [sectors_to_read]
    sub     [sectors_left], ax
    add     [current_lba], eax

    ; Update load address
    mov     ax, [sectors_to_read]
    shl     ax, 5                   ; * 32 (paragraphs per sector)
    add     [load_segment], ax

    jmp     .load_loop

.load_done:
    ; Mark ROM as loaded
    mov     dword [rom_addr], ROM_LOAD_SEG * 16  ; Physical address 0x30000

    pop     bp
    pop     es
    clc
    ret

.no_rom:
    ; No ROM found - kernel will try to load from partition
    mov     dword [rom_addr], 0
    mov     dword [rom_size], 0

    pop     bp
    pop     es
    stc                         ; Set carry to indicate no ROM
    ret

; ============================================================================
; Disk Address Packet (DAP) for LBA reads
; ============================================================================

align 4
dap:
    db 0x10                     ; Size of DAP (16 bytes)
    db 0                        ; Reserved
dap_count:
    dw 0                        ; Number of sectors to read
dap_offset:
    dw 0                        ; Destination offset
dap_segment:
    dw 0                        ; Destination segment
dap_lba:
    dd 0                        ; LBA low 32 bits
    dd 0                        ; LBA high 32 bits

; Variables
sectors_left:   dw 0
current_lba:    dd 0            ; Changed to 32-bit for larger disk support
load_segment:   dw 0
load_offset:    dw 0
rom_sectors:    dw 0
sectors_to_read: dw 0

; ============================================================================
; Print String (16-bit, works before mode switch)
; ============================================================================

print_string:
    pusha
    mov     ah, 0x0E
.loop:
    lodsb
    test    al, al
    jz      .done
    int     0x10
    jmp     .loop
.done:
    popa
    ret

; ============================================================================
; GDT
; ============================================================================

align 16
gdt_start:
    dq 0                            ; Null descriptor

    ; Code segment 0x08: base=0, limit=4GB, 32-bit, executable
    dw 0xFFFF                       ; Limit low
    dw 0                            ; Base low
    db 0                            ; Base mid
    db 10011010b                    ; Access: P=1, DPL=0, S=1, E=1, R=1
    db 11001111b                    ; Flags: G=1, D=1, Limit high=F
    db 0                            ; Base high

    ; Data segment 0x10: base=0, limit=4GB, 32-bit, writable
    dw 0xFFFF
    dw 0
    db 0
    db 10010010b                    ; Access: P=1, DPL=0, S=1, W=1
    db 11001111b
    db 0
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

; ============================================================================
; 32-bit Protected Mode Entry
; ============================================================================

[BITS 32]

protected_mode_entry:
    ; Set up data segments
    mov     ax, 0x10
    mov     ds, ax
    mov     es, ax
    mov     fs, ax
    mov     gs, ax
    mov     ss, ax
    mov     esp, 0x90000            ; Stack below EBDA

    ; Copy kernel from 0x20000 to 1MB (0x100000)
    mov     esi, 0x20000
    mov     edi, 0x100000
    mov     ecx, (KERNEL_SECTORS * 512) / 4
    cld
    rep movsd

    ; Copy ROM from 0x30000 to 3MB (0x300000) if loaded
    mov     eax, [rom_addr]
    test    eax, eax
    jz      .no_rom_copy

    mov     esi, 0x30000            ; Source: temp buffer
    mov     edi, ROM_DEST_ADDR      ; Dest: 3MB
    mov     ecx, [rom_size]
    add     ecx, 3
    shr     ecx, 2                  ; Divide by 4 for dword copy
    rep movsd

    ; Update rom_addr to final location
    mov     dword [rom_addr], ROM_DEST_ADDR

.no_rom_copy:
    ; Build boot info structure at 0x500
    mov     edi, 0x500

    ; Magic: 'GBOY' (0x594F4247)
    mov     dword [edi + 0], 0x594F4247

    ; E820 map pointer
    mov     dword [edi + 4], E820_BASE

    ; VGA mode (0x13 = 320x200x256)
    mov     dword [edi + 8], 0x13

    ; Framebuffer address (0xA0000)
    mov     dword [edi + 12], 0xA0000

    ; Width (320)
    mov     dword [edi + 16], 320

    ; Height (200)
    mov     dword [edi + 20], 200

    ; BPP (8)
    mov     dword [edi + 24], 8

    ; Pitch (320)
    mov     dword [edi + 28], 320

    ; ROM address
    mov     eax, [rom_addr]
    mov     [edi + 32], eax

    ; ROM size
    mov     eax, [rom_size]
    mov     [edi + 36], eax

    ; ROM title (32 bytes at offset 40)
    mov     esi, rom_title
    lea     edi, [0x500 + 40]
    mov     ecx, 8                  ; 32 bytes = 8 dwords
    rep movsd

    ; Quick visual confirmation - draw corner pixels
    mov     byte [0xA0000], 0x0F            ; Top-left white
    mov     byte [0xA013F], 0x0F            ; Top-right white (319)
    mov     byte [0xAF8C0], 0x0F            ; Bottom-left white (199*320)
    mov     byte [0xAF9FF], 0x0F            ; Bottom-right white

    ; Jump to kernel with boot info pointer in EAX
    mov     eax, 0x500
    jmp     0x100000

; ============================================================================
; Data Section (16-bit accessible)
; ============================================================================

[BITS 16]

boot_drive:     db 0
vga_mode:       db 0
fb_address:     dd 0
fb_width:       dw 0
fb_height:      dw 0
fb_bpp:         db 0
fb_pitch:       dw 0

; ROM info (populated by load_rom)
rom_addr:       dd 0
rom_size:       dd 0
rom_title:      times 32 db 0

; Messages (short to save space)
msg_banner:     db 13, 10
                db '=== RetroFuture GB ===', 13, 10, 0
msg_e820:       db ' E820..', 0
msg_a20:        db ' A20..', 0
msg_vga:        db ' VGA..', 0
msg_kernel:     db ' Kernel..', 0
msg_rom:        db ' ROM..', 0
msg_pmode:      db ' PM', 0
msg_ok:         db 'ok', 13, 10, 0
msg_fail:       db 'FAIL', 13, 10, 0
msg_none:       db 'none', 13, 10, 0

; ============================================================================
; Pad to exactly 16KB (32 sectors)
; ============================================================================

times 16384 - ($ - $$) db 0
