; ============================================================================
; stage2.asm - Stage 2 Bootloader for Rustacean OS (with GameBoy Mode)
; ============================================================================
;
; Unified bootloader supporting two modes:
;   1. Normal Mode: Boots into Rustacean OS windowing GUI
;   2. GameBoy Mode: Loads ROM from game floppy, boots into emulator
;
; Mode Selection:
;   - If GAMEBOY_MODE is defined at assembly time, prompts for game floppy
;   - Otherwise, boots normally into Rustacean OS
;
; Boot Info Structure (at 0x500):
;   Offset  Size  Field
;   0x00    4     Magic ('RUST' or 'GBOY')
;   0x04    4     E820 map address
;   0x08    4     VESA enabled
;   0x0C    4     Framebuffer address
;   0x10    4     Screen width
;   0x14    4     Screen height
;   0x18    4     Bits per pixel
;   0x1C    4     Pitch
;   0x20    4     ROM address (GameBoy mode only)
;   0x24    4     ROM size (GameBoy mode only)
;   0x28    32    ROM title (GameBoy mode only)
;
; Assemble for GameBoy mode:  nasm -f bin -DGAMEBOY_MODE -o stage2.bin stage2.asm
; Assemble for normal mode:   nasm -f bin -o stage2.bin stage2.asm
; ============================================================================

[BITS 16]
[ORG 0x7E00]

; ============================================================================
; Magic Number (verified by stage 1)
; ============================================================================

dw 0x5441                       ; 'AT' magic

; ============================================================================
; Constants
; ============================================================================

; Magic values
%ifdef GAMEBOY_MODE
BOOT_MAGIC          equ 0x594F4247  ; 'GBOY'
%else
BOOT_MAGIC          equ 0x54535552  ; 'RUST'
%endif

; Memory addresses
E820_MAP            equ 0x600
E820_BASE           equ 0x600       ; Same as E820_MAP (entry count stored here)
E820_MAX            equ 32          ; Maximum E820 entries
BOOT_INFO           equ 0x500

; Game ROM loading (GameBoy mode)
GAME_HEADER_SEG     equ 0x1000
ROM_LOAD_ADDR       equ 0x02000000  ; 32MB mark
ROM_MAX_SIZE        equ 0x00180000  ; 1.5MB max
GAME_MAGIC          equ 0x594F4247  ; 'GBOY' header magic
ROM_START_SECTOR    equ 289         ; Sector where ROM header starts (after kernel)

; Kernel loading
KERNEL_SECTOR       equ 33
KERNEL_SECTORS      equ 256         ; 128KB
KERNEL_LOAD_SEG     equ 0x2000
KERNEL_DEST         equ 0x100000    ; 1MB

; Floppy geometry (1.44MB)
SECTORS_PER_TRACK   equ 18
HEADS               equ 2

; ============================================================================
; Entry Point
; ============================================================================

stage2_entry:
    mov     [boot_drive], dl

    ; Display banner
%ifdef GAMEBOY_MODE
    mov     si, msg_banner_gb
%else
    mov     si, msg_banner
%endif
    call    print_string

    ; ========================================================================
    ; Step 1: Query E820 Memory Map
    ; ========================================================================
    mov     si, msg_e820
    call    print_string
    call    query_e820
    jc      e820_error
    mov     si, msg_ok
    call    print_string

    ; ========================================================================
    ; Step 2: Enable A20 Line
    ; ========================================================================
    mov     si, msg_a20
    call    print_string
    call    enable_a20
    call    verify_a20
    jc      a20_error
    mov     si, msg_ok
    call    print_string

    ; ========================================================================
    ; Step 3: Setup VESA Graphics (or skip if SKIP_VESA defined)
    ; ========================================================================
%ifdef SKIP_VESA
    ; Skip VESA entirely - use VGA text mode
    mov     si, msg_vesa_skip
    call    print_string
    jmp     vesa_fallback
%else
    mov     si, msg_vesa
    call    print_string
    call    setup_vesa
    jc      vesa_fallback
    mov     si, msg_ok
    call    print_string
    jmp     vesa_done
%endif

vesa_fallback:
    mov     si, msg_vesa_fail
    call    print_string
    mov     byte [vesa_enabled], 0
    mov     dword [vesa_framebuffer], 0xB8000
    mov     word [vesa_width], 80
    mov     word [vesa_height], 25
    mov     byte [vesa_bpp], 16
    mov     word [vesa_pitch], 160

vesa_done:

%ifdef GAMEBOY_MODE
    ; ========================================================================
    ; Step 4: GameBoy Mode - Load ROM from embedded location
    ; ROM is stored at sector ROM_START_SECTOR in the same disk image
    ; ========================================================================
    mov     si, msg_loading_rom
    call    print_string

    call    load_embedded_rom
    jc      .no_game

    mov     si, msg_ok
    call    print_string

    ; Display ROM title
    mov     si, msg_rom_title
    call    print_string
    mov     si, rom_title
    call    print_string
    mov     si, msg_newline
    call    print_string
    jmp     .load_kernel_gb

.no_game:
    mov     si, msg_no_game
    call    print_string
    mov     dword [rom_addr], 0
    mov     dword [rom_size], 0

.load_kernel_gb:
%endif

    ; ========================================================================
    ; Step 5: Load Kernel
    ; ========================================================================
    mov     si, msg_kernel
    call    print_string
    call    do_load_kernel
    jc      kernel_error
    mov     si, msg_ok
    call    print_string

    ; ========================================================================
    ; Step 6: Enter Protected Mode
    ; ========================================================================
    mov     si, msg_pmode
    call    print_string

    ; Build boot info structure
    call    build_boot_info

    cli
    lgdt    [gdt_descriptor]

    mov     eax, cr0
    or      eax, 1
    mov     cr0, eax

    jmp     0x08:protected_mode_entry

; ============================================================================
; Error Handlers
; ============================================================================

e820_error:
    mov     si, msg_e820_fail
    jmp     halt_with_msg

a20_error:
    mov     si, msg_a20_fail
    jmp     halt_with_msg

kernel_error:
    mov     si, msg_kernel_fail
    jmp     halt_with_msg

halt_with_msg:
    call    print_string
    mov     si, msg_halt
    call    print_string
halt:
    cli
    hlt
    jmp     halt

; ============================================================================
; Query E820 Memory Map
; ============================================================================

query_e820:
    push    es
    push    di
    push    ebx
    push    ecx
    push    edx

    xor     ebx, ebx
    mov     di, E820_MAP + 4
    xor     bp, bp

.loop:
    mov     eax, 0xE820
    mov     ecx, 24
    mov     edx, 0x534D4150
    push    di
    int     0x15
    pop     di
    jc      .done
    cmp     eax, 0x534D4150
    jne     .error
    inc     bp
    add     di, 24
    test    ebx, ebx
    jnz     .loop

.done:
    mov     [E820_MAP], bp
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
; Enable A20 Line
; ============================================================================

enable_a20:
    mov     ax, 0x2401
    int     0x15
    jnc     .done
    call    .wait_kbd
    mov     al, 0xAD
    out     0x64, al
    call    .wait_kbd
    mov     al, 0xD0
    out     0x64, al
    call    .wait_kbd_data
    in      al, 0x60
    push    ax
    call    .wait_kbd
    mov     al, 0xD1
    out     0x64, al
    call    .wait_kbd
    pop     ax
    or      al, 2
    out     0x60, al
    call    .wait_kbd
    mov     al, 0xAE
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

; ============================================================================
; Verify A20 Line
; ============================================================================

verify_a20:
    push    es
    push    ds
    xor     ax, ax
    mov     es, ax
    not     ax
    mov     ds, ax
    mov     ax, [es:0x7DFE]
    push    ax
    mov     ax, [ds:0x7E0E]
    push    ax
    mov     byte [es:0x7DFE], 0x00
    mov     byte [ds:0x7E0E], 0xFF
    cmp     byte [es:0x7DFE], 0xFF
    pop     ax
    mov     [ds:0x7E0E], ax
    pop     ax
    mov     [es:0x7DFE], ax
    pop     ds
    pop     es
    je      .fail
    clc
    ret
.fail:
    stc
    ret

; ============================================================================
; Setup VESA Mode (with fallback chain)
; ============================================================================

; VESA mode constants
VESA_INFO       equ 0x2000
VESA_MODE_INFO  equ 0x2200
PREFERRED_MODE  equ 0x115       ; 800x600x32 (common)
FALLBACK_MODE   equ 0x112       ; 640x480x32
FALLBACK_MODE2  equ 0x111       ; 640x480x16
FALLBACK_MODE3  equ 0x101       ; 640x480x8

setup_vesa:
    push    es

    ; Get VBE info
    mov     ax, 0x4F00
    mov     di, VESA_INFO
    push    ds
    pop     es
    int     0x10
    cmp     ax, 0x004F
    jne     .use_vga_text

    ; Try preferred mode first (800x600x32)
    mov     cx, PREFERRED_MODE
    call    .try_mode
    jnc     .set_mode

    ; Try 640x480x32
    mov     cx, FALLBACK_MODE
    call    .try_mode
    jnc     .set_mode

    ; Try 640x480x16
    mov     cx, FALLBACK_MODE2
    call    .try_mode
    jnc     .set_mode

    ; Try 640x480x8
    mov     cx, FALLBACK_MODE3
    call    .try_mode
    jnc     .set_mode

    ; All VESA modes failed - use VGA text mode
.use_vga_text:
    ; Set 80x25 text mode (mode 3)
    mov     ax, 0x0003
    int     0x10

    ; Mark as VGA text mode
    mov     byte [vesa_enabled], 0
    mov     dword [vesa_framebuffer], 0xB8000
    mov     word [vesa_width], 80
    mov     word [vesa_height], 25
    mov     byte [vesa_bpp], 16
    mov     word [vesa_pitch], 160

    pop     es
    stc                         ; Indicate fallback
    ret

.try_mode:
    ; Get mode info
    push    cx
    mov     ax, 0x4F01
    mov     di, VESA_MODE_INFO
    int     0x10
    pop     cx
    cmp     ax, 0x004F
    jne     .try_fail

    ; Check if mode has linear framebuffer
    test    byte [VESA_MODE_INFO], 0x80
    jz      .try_fail

    clc
    ret
.try_fail:
    stc
    ret

.set_mode:
    ; Set the mode with linear framebuffer
    mov     ax, 0x4F02
    mov     bx, cx
    or      bx, 0x4000          ; Linear framebuffer bit
    int     0x10
    cmp     ax, 0x004F
    jne     .use_vga_text

    ; Save mode info for kernel
    mov     [vesa_mode], cx
    mov     byte [vesa_enabled], 1

    ; Copy relevant info
    mov     eax, [VESA_MODE_INFO + 40]  ; Physical framebuffer address
    mov     [vesa_framebuffer], eax
    mov     ax, [VESA_MODE_INFO + 18]   ; Width
    mov     [vesa_width], ax
    mov     ax, [VESA_MODE_INFO + 20]   ; Height
    mov     [vesa_height], ax
    mov     al, [VESA_MODE_INFO + 25]   ; BPP
    mov     [vesa_bpp], al
    mov     ax, [VESA_MODE_INFO + 16]   ; Pitch
    mov     [vesa_pitch], ax

    pop     es
    clc                         ; Success
    ret

%ifdef GAMEBOY_MODE
; ============================================================================
; Load Embedded ROM (from fixed sectors in boot image)
; ROM header at ROM_START_SECTOR, data follows immediately after
; ============================================================================

load_embedded_rom:
    push    es

    ; Reset drive
    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13

    ; Read ROM header sector using LBA
    mov     ax, ROM_START_SECTOR
    call    lba_to_chs_rom      ; Convert LBA to CHS

    mov     ax, GAME_HEADER_SEG
    mov     es, ax
    xor     bx, bx
    mov     bp, 5               ; Retries
.read_header_retry:
    mov     ah, 0x02
    mov     al, 1
    mov     dl, [boot_drive]
    int     0x13
    jnc     .header_ok
    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13
    dec     bp
    jnz     .read_header_retry
    jmp     .error

.header_ok:
    ; Verify magic
    mov     eax, [es:0]
    cmp     eax, GAME_MAGIC
    jne     .error

    ; Get ROM size
    mov     eax, [es:4]
    test    eax, eax
    jz      .error              ; Size 0 = no ROM embedded
    cmp     eax, ROM_MAX_SIZE
    ja      .error
    mov     [rom_size], eax

    ; Copy title
    mov     si, 8
    mov     di, rom_title
    mov     cx, 32
.copy_title:
    mov     al, [es:si]
    mov     [di], al
    inc     si
    inc     di
    loop    .copy_title

    ; Enable unreal mode for high memory access
    call    enable_unreal_mode

    ; Calculate sectors to load
    mov     eax, [rom_size]
    add     eax, 511
    shr     eax, 9
    mov     [sectors_to_load], ax

    mov     word [current_lba], ROM_START_SECTOR + 1  ; Data starts after header
    mov     dword [load_dest], ROM_LOAD_ADDR

.load_loop:
    cmp     word [sectors_to_load], 0
    je      .done

    ; Convert LBA to CHS
    mov     ax, [current_lba]
    call    lba_to_chs_rom

    ; Save CHS
    push    cx
    push    dx

    ; Read sector to temp buffer
    mov     ax, GAME_HEADER_SEG
    mov     es, ax
    xor     bx, bx
    mov     bp, 5               ; Retries
.read_sector_retry:
    pop     dx
    pop     cx
    push    cx
    push    dx
    mov     ah, 0x02
    mov     al, 1
    mov     dl, [boot_drive]
    int     0x13
    jnc     .sector_ok
    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13
    dec     bp
    jnz     .read_sector_retry
    pop     dx
    pop     cx
    jmp     .error

.sector_ok:
    pop     dx
    pop     cx

    ; Copy to high memory using unreal mode
    push    ds
    xor     ax, ax
    mov     ds, ax
    mov     esi, GAME_HEADER_SEG * 16
    mov     edi, [load_dest]
    mov     ecx, 512
.copy:
    mov     al, [esi]
    mov     [edi], al
    inc     esi
    inc     edi
    loop    .copy
    pop     ds

    add     dword [load_dest], 512
    inc     word [current_lba]
    dec     word [sectors_to_load]
    jmp     .load_loop

.done:
    mov     dword [rom_addr], ROM_LOAD_ADDR
    pop     es
    clc
    ret

.error:
    mov     dword [rom_addr], 0
    mov     dword [rom_size], 0
    pop     es
    stc
    ret

; Convert LBA in AX to CHS for ROM reading
; Returns: CH=cylinder, CL=sector, DH=head
lba_to_chs_rom:
    push    bx
    xor     dx, dx
    mov     bx, SECTORS_PER_TRACK
    div     bx                  ; AX = head*cyl, DX = sector-1
    push    dx                  ; Save sector
    xor     dx, dx
    mov     bx, HEADS
    div     bx                  ; AX = cylinder, DX = head
    mov     ch, al              ; Cylinder
    mov     dh, dl              ; Head
    pop     ax
    inc     al                  ; Sector (1-based)
    mov     cl, al
    pop     bx
    ret

%endif  ; GAMEBOY_MODE

; ============================================================================
; Load Kernel
; ============================================================================

do_load_kernel:
    push    es
    push    bp

    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13

    mov     word [sectors_left], KERNEL_SECTORS
    mov     word [current_lba], KERNEL_SECTOR
    mov     word [load_segment], KERNEL_LOAD_SEG
    mov     word [load_offset], 0

.load_loop:
    cmp     word [sectors_left], 0
    je      .copy_to_high

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

    mov     ax, [load_segment]
    mov     es, ax
    mov     bx, [load_offset]

    mov     bp, 3
.retry:
    push    bx
    push    cx
    mov     ah, 0x02
    mov     al, 1
    mov     dl, [boot_drive]
    int     0x13
    pop     cx
    pop     bx
    jnc     .success
    dec     bp
    jnz     .retry
    pop     bp
    pop     es
    stc
    ret

.success:
    inc     word [current_lba]
    dec     word [sectors_left]
    add     word [load_offset], 512
    cmp     word [load_offset], 0
    jne     .load_loop
    add     word [load_segment], 0x1000
    jmp     .load_loop

.copy_to_high:
    call    enable_unreal_mode
    push    ds
    xor     ax, ax
    mov     ds, ax
    mov     esi, KERNEL_LOAD_SEG * 16
    mov     edi, KERNEL_DEST
    mov     ecx, KERNEL_SECTORS
    shl     ecx, 9
.copy:
    mov     al, [esi]
    mov     [edi], al
    inc     esi
    inc     edi
    loop    .copy
    pop     ds

    pop     bp
    pop     es
    clc
    ret

enable_unreal_mode:
    push    ds
    push    es
    cli
    lgdt    [gdt_descriptor]
    mov     eax, cr0
    or      al, 1
    mov     cr0, eax
    mov     bx, 0x10
    mov     ds, bx
    mov     es, bx
    and     al, 0xFE
    mov     cr0, eax
    pop     es
    pop     ds
    sti
    ret

; ============================================================================
; Build Boot Info Structure
; ============================================================================

build_boot_info:
    push    es
    push    di

    xor     ax, ax
    mov     es, ax
    mov     di, BOOT_INFO

    ; Magic
    mov     dword [es:di], BOOT_MAGIC
    add     di, 4

    ; E820 map address
    mov     dword [es:di], E820_MAP
    add     di, 4

    ; VESA enabled
    xor     eax, eax
    mov     al, [vesa_enabled]
    mov     [es:di], eax
    add     di, 4

    ; Framebuffer address
    mov     eax, [vesa_framebuffer]
    mov     [es:di], eax
    add     di, 4

    ; Screen width
    xor     eax, eax
    mov     ax, [vesa_width]
    mov     [es:di], eax
    add     di, 4

    ; Screen height
    xor     eax, eax
    mov     ax, [vesa_height]
    mov     [es:di], eax
    add     di, 4

    ; Bits per pixel
    xor     eax, eax
    mov     al, [vesa_bpp]
    mov     [es:di], eax
    add     di, 4

    ; Pitch
    xor     eax, eax
    mov     ax, [vesa_pitch]
    mov     [es:di], eax
    add     di, 4

%ifdef GAMEBOY_MODE
    ; ROM address
    mov     eax, [rom_addr]
    mov     [es:di], eax
    add     di, 4

    ; ROM size
    mov     eax, [rom_size]
    mov     [es:di], eax
    add     di, 4

    ; ROM title (32 bytes)
    mov     si, rom_title
    mov     cx, 32
.copy_title:
    lodsb
    stosb
    loop    .copy_title
%else
    ; No ROM in normal mode
    mov     dword [es:di], 0    ; rom_addr
    add     di, 4
    mov     dword [es:di], 0    ; rom_size
    add     di, 4
    ; Skip title (32 bytes of zeros already there)
%endif

    pop     di
    pop     es
    ret

; ============================================================================
; Print String
; ============================================================================

print_string:
    push    ax
    push    bx
    mov     ah, 0x0E
    mov     bx, 0x0007
.loop:
    lodsb
    test    al, al
    jz      .done
    int     0x10
    jmp     .loop
.done:
    pop     bx
    pop     ax
    ret

; Print AL as two hex digits
print_hex_byte:
    push    ax
    push    bx
    push    cx
    mov     cl, al          ; Save byte
    mov     ah, 0x0E
    mov     bx, 0x0007

    ; High nibble
    mov     al, cl
    shr     al, 4
    add     al, '0'
    cmp     al, '9'
    jle     .high_ok
    add     al, 7           ; Convert to A-F
.high_ok:
    int     0x10

    ; Low nibble
    mov     al, cl
    and     al, 0x0F
    add     al, '0'
    cmp     al, '9'
    jle     .low_ok
    add     al, 7
.low_ok:
    int     0x10

    ; Space
    mov     al, ' '
    int     0x10

    pop     cx
    pop     bx
    pop     ax
    ret

; ============================================================================
; Protected Mode Entry
; ============================================================================

[BITS 32]

protected_mode_entry:
    mov     ax, 0x10
    mov     ds, ax
    mov     es, ax
    mov     fs, ax
    mov     gs, ax
    mov     ss, ax
    mov     esp, 0x90000

%ifdef GAMEBOY_MODE
    mov     byte [0xB8000], 'G'
    mov     byte [0xB8001], 0x2F
%else
    mov     byte [0xB8000], 'R'
    mov     byte [0xB8001], 0x2F
%endif

    mov     eax, BOOT_INFO
    jmp     KERNEL_DEST

; ============================================================================
; GDT
; ============================================================================

[BITS 16]

gdt_start:
    dq 0
    dw 0xFFFF, 0x0000
    db 0x00, 10011010b, 11001111b, 0x00
    dw 0xFFFF, 0x0000
    db 0x00, 10010010b, 11001111b, 0x00
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

; ============================================================================
; Data
; ============================================================================

boot_drive:         db 0
vesa_enabled:       db 0
vesa_mode:          dw 0
vesa_framebuffer:   dd 0
vesa_width:         dw 0
vesa_height:        dw 0
vesa_bpp:           db 0
vesa_pitch:         dw 0

%ifdef GAMEBOY_MODE
rom_addr:           dd 0
rom_size:           dd 0
rom_title:          times 33 db 0
sectors_to_load:    dw 0
load_dest:          dd 0
%endif

current_lba:        dw 0
sectors_left:       dw 0
load_segment:       dw 0
load_offset:        dw 0

; Messages
%ifdef GAMEBOY_MODE
msg_banner_gb:      db 13, 10
                    db '========================================', 13, 10
                    db '     RUSTACEAN OS - GameBoy Edition     ', 13, 10
                    db '========================================', 13, 10, 0
msg_loading_rom:    db '  [....] Loading embedded ROM', 0
msg_rom_title:      db '  Game: ', 0
msg_no_game:        db 13, '  [WARN] No ROM embedded in image', 13, 10, 0
msg_newline:        db 13, 10, 0
%else
msg_banner:         db 13, 10
                    db '========================================', 13, 10
                    db '      RUSTACEAN OS - Stage 2 Loader     ', 13, 10
                    db '========================================', 13, 10, 0
%endif

msg_e820:           db '  [....] Querying memory map', 0
msg_a20:            db '  [....] Enabling A20 line', 0
msg_vesa:           db '  [....] Setting up VESA 800x600', 0
msg_vesa_skip:      db '  [SKIP] VESA disabled, using VGA text', 13, 10, 0
msg_kernel:         db '  [....] Loading kernel', 0
msg_pmode:          db '  [....] Entering protected mode', 13, 10, 0
msg_ok:             db 13, '  [ OK ]', 13, 10, 0
msg_e820_fail:      db 13, '  [FAIL] E820 query failed!', 13, 10, 0
msg_a20_fail:       db 13, '  [FAIL] A20 enable failed!', 13, 10, 0
msg_vesa_fail:      db 13, '  [WARN] VESA unavailable', 13, 10, 0
msg_kernel_fail:    db 13, '  [FAIL] Kernel load failed!', 13, 10, 0
msg_halt:           db '  System halted.', 13, 10, 0

; Pad to 16KB
times 16384 - ($ - $$) db 0
