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
; Load Kernel
; ============================================================================

KERNEL_SECTOR   equ 33          ; After boot(1) + stage2(32)
KERNEL_SECTORS  equ 200         ; 100KB kernel (94KB actual + headroom)
KERNEL_LOAD_SEG equ 0x2000      ; Load to 0x20000
KERNEL_LOAD_OFF equ 0x0000

SECTORS_PER_TRACK equ 18
HEADS           equ 2

load_kernel:
    push    es
    push    bp

    ; Reset disk
    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13

    mov     word [sectors_left], KERNEL_SECTORS
    mov     word [current_lba], KERNEL_SECTOR
    mov     word [load_segment], KERNEL_LOAD_SEG
    mov     word [load_offset], KERNEL_LOAD_OFF

.loop:
    cmp     word [sectors_left], 0
    je      .done

    ; LBA to CHS conversion
    mov     ax, [current_lba]
    xor     dx, dx
    mov     cx, SECTORS_PER_TRACK
    div     cx                      ; AX = head + track*2, DX = sector-1
    push    dx
    xor     dx, dx
    mov     cx, HEADS
    div     cx                      ; AX = track, DX = head
    mov     ch, al                  ; Cylinder
    mov     dh, dl                  ; Head
    pop     ax
    inc     al
    mov     cl, al                  ; Sector (1-based)

    ; Destination
    mov     ax, [load_segment]
    mov     es, ax
    mov     bx, [load_offset]

    ; Read with retry
    mov     bp, 3
.retry:
    push    bx
    push    cx
    push    dx
    push    es

    mov     ah, 0x02
    mov     al, 1
    mov     dl, [boot_drive]
    int     0x13

    pop     es
    pop     dx
    pop     cx
    pop     bx

    jnc     .read_ok

    ; Reset and retry
    push    dx
    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13
    pop     dx
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

    ; Advance load address
    add     word [load_offset], 512
    jnc     .no_wrap
    add     word [load_segment], 0x1000
    mov     word [load_offset], 0
.no_wrap:
    inc     word [current_lba]
    dec     word [sectors_left]
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
; Load ROM (if present)
; ============================================================================

ROM_HEADER_SECTOR equ 289       ; Where ROM header is stored
ROM_LOAD_SEG      equ 0x3000    ; Temporary load buffer at 0x30000
ROM_DEST_ADDR     equ 0x300000  ; Final ROM location at 3MB

load_rom:
    push    es
    push    bp

    ; Load ROM header sector to temporary buffer
    mov     ax, ROM_LOAD_SEG
    mov     es, ax
    xor     bx, bx

    ; LBA to CHS for sector 289
    mov     ax, ROM_HEADER_SECTOR
    xor     dx, dx
    mov     cx, SECTORS_PER_TRACK
    div     cx
    push    dx
    xor     dx, dx
    mov     cx, HEADS
    div     cx
    mov     ch, al
    mov     dh, dl
    pop     ax
    inc     al
    mov     cl, al

    ; Read header sector
    mov     ah, 0x02
    mov     al, 1
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

    ; Load ROM data starting at sector 290
    mov     word [current_lba], ROM_HEADER_SECTOR + 1
    mov     word [load_segment], ROM_LOAD_SEG
    mov     word [load_offset], 0
    mov     ax, [rom_sectors]
    mov     [sectors_left], ax

.load_loop:
    cmp     word [sectors_left], 0
    je      .load_done

    ; LBA to CHS
    mov     ax, [current_lba]
    xor     dx, dx
    mov     cx, SECTORS_PER_TRACK
    div     cx
    push    dx
    xor     dx, dx
    mov     cx, HEADS
    div     cx
    mov     ch, al
    mov     dh, dl
    pop     ax
    inc     al
    mov     cl, al

    ; Destination
    mov     ax, [load_segment]
    mov     es, ax
    mov     bx, [load_offset]

    ; Read sector
    mov     bp, 3
.rom_retry:
    push    bx
    push    cx
    push    dx
    push    es

    mov     ah, 0x02
    mov     al, 1
    mov     dl, [boot_drive]
    int     0x13

    pop     es
    pop     dx
    pop     cx
    pop     bx

    jnc     .rom_read_ok

    push    dx
    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13
    pop     dx
    dec     bp
    jnz     .rom_retry
    jmp     .no_rom

.rom_read_ok:
    ; Advance
    add     word [load_offset], 512
    jnc     .rom_no_wrap
    add     word [load_segment], 0x1000
    mov     word [load_offset], 0
.rom_no_wrap:
    inc     word [current_lba]
    dec     word [sectors_left]
    jmp     .load_loop

.load_done:
    ; Mark ROM as loaded
    mov     dword [rom_addr], ROM_LOAD_SEG * 16  ; Physical address 0x30000

    pop     bp
    pop     es
    clc
    ret

.no_rom:
    ; No ROM found
    mov     dword [rom_addr], 0
    mov     dword [rom_size], 0

    pop     bp
    pop     es
    stc                         ; Set carry to indicate no ROM
    ret

; Variables
sectors_left:   dw 0
current_lba:    dw 0
load_segment:   dw 0
load_offset:    dw 0
rom_sectors:    dw 0

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
