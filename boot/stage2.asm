; ============================================================================
; stage2.asm - Stage 2 Bootloader for gb-os (No-Emulation Support)
; ============================================================================
;
; This bootloader supports booting from:
;   1. Floppy disk (CHS addressing)
;   2. CD-ROM via El Torito no-emulation (LBA addressing)
;   3. Hard disk / USB (LBA addressing)
;
; The boot media type is passed from stage1 at BOOT_INFO_ADDR+1
;
; Tasks:
;   1. Query E820 memory map
;   2. Enable A20 line
;   3. Set VGA mode 13h (320x200x256)
;   4. Load kernel to 0x100000 (1MB)
;   5. Load ROM to 0x300000 (3MB) if present
;   6. Switch to 32-bit protected mode
;   7. Jump to kernel
;
; Installation feature:
;   - If booted from CD, can install to HDD/USB
;   - Kernel will handle installation UI
;
; Assemble: nasm -f bin -o stage2.bin stage2.asm
;           nasm -f bin -DGAMEBOY_MODE -o stage2-gameboy.bin stage2.asm
; ============================================================================

[BITS 16]
[ORG 0x7E00]

; ============================================================================
; Magic Number (verified by stage 1)
; ============================================================================

dw 0x5247                       ; 'GR' magic (GameBoy Retro)

; ============================================================================
; Constants
; ============================================================================

BOOT_INFO_ADDR      equ 0x0500
E820_BASE           equ 0x1000      ; E820 map stored here
E820_MAX_ENTRIES    equ 64

KERNEL_LOAD_SEG     equ 0x2000      ; Kernel loaded to 0x20000 initially
KERNEL_DEST_ADDR    equ 0x100000    ; Final kernel location (1MB)
KERNEL_SECTORS_FDD  equ 512         ; 256KB for kernel (floppy sectors)
KERNEL_SECTORS_CD   equ 128         ; 256KB for kernel (CD sectors)
KERNEL_START_FDD    equ 33          ; Kernel starts at sector 33 on floppy
KERNEL_START_CD     equ 9           ; Kernel starts at CD sector 9

ROM_LOAD_SEG        equ 0x4000      ; ROM loaded to 0x40000 initially
ROM_DEST_ADDR       equ 0x300000    ; Final ROM location (3MB)
ROM_HEADER_SECTOR   equ 289         ; ROM header at sector 289 (floppy)
ROM_HEADER_CD       equ 145         ; ROM header CD sector

; Boot media types (from stage1)
BOOT_MEDIA_FLOPPY   equ 0x00
BOOT_MEDIA_CDROM    equ 0x01
BOOT_MEDIA_HDD      equ 0x02

; Floppy geometry
SECTORS_PER_TRACK   equ 18
HEADS               equ 2

; ============================================================================
; Entry Point
; ============================================================================

stage2_entry:
    ; Get boot info from stage1
    mov     al, [BOOT_INFO_ADDR]
    mov     [boot_drive], al
    mov     al, [BOOT_INFO_ADDR + 1]
    mov     [boot_media], al

    ; Display banner
    mov     si, msg_banner
    call    print_string

    ; Show boot media type
    mov     si, msg_media
    call    print_string
    mov     al, [boot_media]
    cmp     al, BOOT_MEDIA_CDROM
    je      .show_cd
    cmp     al, BOOT_MEDIA_HDD
    je      .show_hdd
    mov     si, msg_floppy
    jmp     .show_media_done
.show_cd:
    mov     si, msg_cdrom
    jmp     .show_media_done
.show_hdd:
    mov     si, msg_hdd
.show_media_done:
    call    print_string
    mov     si, msg_newline
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

    ; Select loading method based on boot media
    mov     al, [boot_media]
    cmp     al, BOOT_MEDIA_CDROM
    je      .load_kernel_cd
    cmp     al, BOOT_MEDIA_HDD
    je      .load_kernel_lba
    jmp     .load_kernel_floppy

.load_kernel_floppy:
    call    load_kernel_floppy
    jmp     .kernel_loaded

.load_kernel_cd:
    call    load_kernel_cd
    jmp     .kernel_loaded

.load_kernel_lba:
    call    load_kernel_lba
    jmp     .kernel_loaded

.kernel_loaded:
    mov     si, msg_ok
    call    print_string

    ; ------------------------------------------------------------------------
    ; Step 5: Load ROM (if present)
    ; ------------------------------------------------------------------------
%ifdef GAMEBOY_MODE
    mov     si, msg_rom
    call    print_string

    mov     al, [boot_media]
    cmp     al, BOOT_MEDIA_CDROM
    je      .load_rom_cd
    cmp     al, BOOT_MEDIA_HDD
    je      .load_rom_lba
    jmp     .load_rom_floppy

.load_rom_floppy:
    call    load_rom_floppy
    jmp     .rom_check

.load_rom_cd:
    call    load_rom_cd
    jmp     .rom_check

.load_rom_lba:
    call    load_rom_lba
    jmp     .rom_check

.rom_check:
    jc      .no_rom
    mov     si, msg_ok
    call    print_string
    jmp     .step6

.no_rom:
    mov     si, msg_none
    call    print_string
%endif

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

    ; Enable protected mode (set PE bit in CR0)
    mov     eax, cr0
    or      al, 1
    mov     cr0, eax

    ; Far jump to 32-bit code
    jmp     0x08:protected_mode_entry

; ============================================================================
; Query E820 Memory Map
; ============================================================================

query_e820:
    push    es
    push    bp
    mov     bp, sp

    ; Set up destination
    xor     ax, ax
    mov     es, ax
    mov     di, E820_BASE + 4       ; Skip count field

    xor     ebx, ebx                ; Continuation value
    xor     si, si                  ; Entry counter

.loop:
    mov     eax, 0xE820
    mov     ecx, 24                 ; Entry size
    mov     edx, 0x534D4150         ; 'SMAP'
    int     0x15

    jc      .done                   ; Carry set = end or error

    cmp     eax, 0x534D4150         ; Check signature
    jne     .error

    ; Valid entry
    inc     si
    add     di, 24

    ; Check for end
    test    ebx, ebx
    jz      .done

    ; Check max entries
    cmp     si, E820_MAX_ENTRIES
    jl      .loop

.done:
    ; Store entry count
    mov     [E820_BASE], si
    clc
    jmp     .exit

.error:
    stc

.exit:
    mov     sp, bp
    pop     bp
    pop     es
    ret

; ============================================================================
; Enable A20 Line
; ============================================================================

enable_a20:
    ; Try BIOS method first
    mov     ax, 0x2401
    int     0x15
    jnc     .done

    ; Try keyboard controller method
    call    .wait_kbd
    mov     al, 0xAD                ; Disable keyboard
    out     0x64, al
    call    .wait_kbd

    mov     al, 0xD0                ; Read output port
    out     0x64, al
    call    .wait_data
    in      al, 0x60
    push    ax

    call    .wait_kbd
    mov     al, 0xD1                ; Write output port
    out     0x64, al
    call    .wait_kbd

    pop     ax
    or      al, 2                   ; Set A20 bit
    out     0x60, al
    call    .wait_kbd

    mov     al, 0xAE                ; Enable keyboard
    out     0x64, al
    call    .wait_kbd

.done:
    ret

.wait_kbd:
    in      al, 0x64
    test    al, 2
    jnz     .wait_kbd
    ret

.wait_data:
    in      al, 0x64
    test    al, 1
    jz      .wait_data
    ret

; ============================================================================
; Verify A20 Line
; ============================================================================

verify_a20:
    push    ds
    push    es

    xor     ax, ax
    mov     ds, ax
    not     ax
    mov     es, ax

    mov     si, 0x0500
    mov     di, 0x0510

    mov     al, [ds:si]
    push    ax
    mov     al, [es:di]
    push    ax

    mov     byte [ds:si], 0x00
    mov     byte [es:di], 0xFF

    cmp     byte [ds:si], 0xFF

    pop     ax
    mov     [es:di], al
    pop     ax
    mov     [ds:si], al

    pop     es
    pop     ds

    je      .a20_off
    clc
    ret

.a20_off:
    stc
    ret

; ============================================================================
; Load Kernel - Floppy (CHS)
; ============================================================================

load_kernel_floppy:
    push    es
    push    bp

    ; Set up destination
    mov     ax, KERNEL_LOAD_SEG
    mov     es, ax
    xor     bx, bx

    mov     word [current_lba], KERNEL_START_FDD
    mov     word [sectors_left], KERNEL_SECTORS_FDD

.load_loop:
    cmp     word [sectors_left], 0
    je      .done

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

    ; Read
    mov     bp, 3
.retry:
    mov     ah, 0x02
    mov     al, 1
    mov     dl, [boot_drive]
    int     0x13
    jnc     .read_ok

    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13
    dec     bp
    jnz     .retry
    jmp     .error

.read_ok:
    add     bx, 512
    jnc     .no_wrap
    mov     ax, es
    add     ax, 0x1000
    mov     es, ax
    xor     bx, bx
.no_wrap:
    inc     word [current_lba]
    dec     word [sectors_left]
    jmp     .load_loop

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
; Load Kernel - CD-ROM (LBA with 2048-byte sectors)
; ============================================================================

load_kernel_cd:
    push    es
    push    bp

    mov     dword [dap_lba_low], KERNEL_START_CD
    mov     dword [dap_lba_high], 0
    mov     word [sectors_left], KERNEL_SECTORS_CD
    mov     word [load_segment], KERNEL_LOAD_SEG
    mov     word [load_offset], 0

.load_loop:
    cmp     word [sectors_left], 0
    je      .done

    ; Set up DAP
    mov     word [dap_size], 0x10
    mov     word [dap_reserved], 0
    mov     word [dap_count], 1
    mov     ax, [load_offset]
    mov     [dap_offset], ax
    mov     ax, [load_segment]
    mov     [dap_segment], ax

    ; Extended read
    mov     si, dap_size
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jc      .error

    ; Advance (2048-byte sectors)
    add     word [load_offset], 2048
    jnc     .no_wrap
    add     word [load_segment], 0x1000
    mov     word [load_offset], 0
.no_wrap:
    inc     dword [dap_lba_low]
    dec     word [sectors_left]
    jmp     .load_loop

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
; Load Kernel - HDD/USB (LBA with 512-byte sectors)
; ============================================================================

load_kernel_lba:
    push    es
    push    bp

    mov     dword [dap_lba_low], KERNEL_START_FDD
    mov     dword [dap_lba_high], 0
    mov     word [sectors_left], KERNEL_SECTORS_FDD
    mov     word [load_segment], KERNEL_LOAD_SEG
    mov     word [load_offset], 0

.load_loop:
    cmp     word [sectors_left], 0
    je      .done

    ; Set up DAP
    mov     word [dap_size], 0x10
    mov     word [dap_reserved], 0
    mov     word [dap_count], 1
    mov     ax, [load_offset]
    mov     [dap_offset], ax
    mov     ax, [load_segment]
    mov     [dap_segment], ax

    ; Extended read
    mov     si, dap_size
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jc      .error

    ; Advance (512-byte sectors)
    add     word [load_offset], 512
    jnc     .no_wrap
    add     word [load_segment], 0x1000
    mov     word [load_offset], 0
.no_wrap:
    inc     dword [dap_lba_low]
    dec     word [sectors_left]
    jmp     .load_loop

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
; Load ROM - Floppy (CHS) [GameBoy Mode Only]
; ============================================================================

%ifdef GAMEBOY_MODE
load_rom_floppy:
    push    es
    push    bp

    ; First, read the ROM header at sector 289
    mov     ax, ROM_LOAD_SEG
    mov     es, ax
    xor     bx, bx

    ; LBA to CHS for header sector
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

    ; Read header
    mov     ah, 0x02
    mov     al, 1
    mov     dl, [boot_drive]
    int     0x13
    jc      .no_rom

    ; Check for 'GBOY' magic
    cmp     dword [es:0], 0x594F4247
    jne     .no_rom

    ; Get ROM size
    mov     eax, [es:4]
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

    ; Calculate sectors needed
    mov     eax, [rom_size]
    add     eax, 511
    shr     eax, 9
    mov     [rom_sectors], ax

    ; Load ROM data starting at sector 290
    mov     word [current_lba], ROM_HEADER_SECTOR + 1
    mov     word [load_segment], ROM_LOAD_SEG
    mov     word [load_offset], 0

.load_loop:
    mov     ax, [rom_sectors]
    test    ax, ax
    jz      .done

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

    ; Read
    mov     bp, 3
.retry:
    mov     ah, 0x02
    mov     al, 1
    mov     dl, [boot_drive]
    int     0x13
    jnc     .read_ok

    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13
    dec     bp
    jnz     .retry
    jmp     .no_rom

.read_ok:
    add     word [load_offset], 512
    jnc     .no_wrap
    add     word [load_segment], 0x1000
    mov     word [load_offset], 0
.no_wrap:
    inc     word [current_lba]
    dec     word [rom_sectors]
    jmp     .load_loop

.done:
    mov     dword [rom_addr], ROM_LOAD_SEG * 16
    pop     bp
    pop     es
    clc
    ret

.no_rom:
    mov     dword [rom_addr], 0
    mov     dword [rom_size], 0
    pop     bp
    pop     es
    stc
    ret

; ============================================================================
; Load ROM - CD-ROM (LBA with 2048-byte sectors) [GameBoy Mode Only]
; ============================================================================

load_rom_cd:
    push    es
    push    bp

    ; Read ROM header at CD sector
    mov     dword [dap_lba_low], ROM_HEADER_CD
    mov     dword [dap_lba_high], 0
    mov     word [dap_size], 0x10
    mov     word [dap_reserved], 0
    mov     word [dap_count], 1
    mov     word [dap_offset], 0
    mov     word [dap_segment], ROM_LOAD_SEG

    mov     si, dap_size
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jc      .no_rom

    ; Check for 'GBOY' magic
    mov     ax, ROM_LOAD_SEG
    mov     es, ax
    cmp     dword [es:0], 0x594F4247
    jne     .no_rom

    ; Get ROM size
    mov     eax, [es:4]
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

    ; Calculate CD sectors needed (2048-byte sectors)
    mov     eax, [rom_size]
    add     eax, 2047
    shr     eax, 11              ; Divide by 2048
    mov     [rom_sectors], ax

    ; Load ROM data starting at next CD sector
    mov     dword [dap_lba_low], ROM_HEADER_CD + 1
    mov     word [load_segment], ROM_LOAD_SEG
    mov     word [load_offset], 0

.load_loop:
    mov     ax, [rom_sectors]
    test    ax, ax
    jz      .done

    ; Set up DAP
    mov     word [dap_size], 0x10
    mov     word [dap_reserved], 0
    mov     word [dap_count], 1
    mov     ax, [load_offset]
    mov     [dap_offset], ax
    mov     ax, [load_segment]
    mov     [dap_segment], ax

    mov     si, dap_size
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jc      .no_rom

    add     word [load_offset], 2048
    jnc     .no_wrap
    add     word [load_segment], 0x1000
    mov     word [load_offset], 0
.no_wrap:
    inc     dword [dap_lba_low]
    dec     word [rom_sectors]
    jmp     .load_loop

.done:
    mov     dword [rom_addr], ROM_LOAD_SEG * 16
    pop     bp
    pop     es
    clc
    ret

.no_rom:
    mov     dword [rom_addr], 0
    mov     dword [rom_size], 0
    pop     bp
    pop     es
    stc
    ret

; ============================================================================
; Load ROM - HDD/USB (LBA with 512-byte sectors) [GameBoy Mode Only]
; ============================================================================

load_rom_lba:
    push    es
    push    bp

    ; Read ROM header
    mov     dword [dap_lba_low], ROM_HEADER_SECTOR
    mov     dword [dap_lba_high], 0
    mov     word [dap_size], 0x10
    mov     word [dap_reserved], 0
    mov     word [dap_count], 1
    mov     word [dap_offset], 0
    mov     word [dap_segment], ROM_LOAD_SEG

    mov     si, dap_size
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jc      .no_rom

    ; Check for 'GBOY' magic
    mov     ax, ROM_LOAD_SEG
    mov     es, ax
    cmp     dword [es:0], 0x594F4247
    jne     .no_rom

    ; Get ROM size
    mov     eax, [es:4]
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

    ; Calculate sectors needed
    mov     eax, [rom_size]
    add     eax, 511
    shr     eax, 9
    mov     [rom_sectors], ax

    ; Load ROM data
    mov     dword [dap_lba_low], ROM_HEADER_SECTOR + 1
    mov     word [load_segment], ROM_LOAD_SEG
    mov     word [load_offset], 0

.load_loop:
    mov     ax, [rom_sectors]
    test    ax, ax
    jz      .done

    mov     word [dap_size], 0x10
    mov     word [dap_reserved], 0
    mov     word [dap_count], 1
    mov     ax, [load_offset]
    mov     [dap_offset], ax
    mov     ax, [load_segment]
    mov     [dap_segment], ax

    mov     si, dap_size
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jc      .no_rom

    add     word [load_offset], 512
    jnc     .no_wrap
    add     word [load_segment], 0x1000
    mov     word [load_offset], 0
.no_wrap:
    inc     dword [dap_lba_low]
    dec     word [rom_sectors]
    jmp     .load_loop

.done:
    mov     dword [rom_addr], ROM_LOAD_SEG * 16
    pop     bp
    pop     es
    clc
    ret

.no_rom:
    mov     dword [rom_addr], 0
    mov     dword [rom_size], 0
    pop     bp
    pop     es
    stc
    ret
%endif

; ============================================================================
; Print String (16-bit)
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

halt:
    cli
.loop:
    hlt
    jmp     .loop

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
; DAP (Disk Address Packet) for INT 13h extensions
; ============================================================================

align 4
dap_size:       dw 0x10
dap_reserved:   dw 0
dap_count:      dw 1
dap_offset:     dw 0
dap_segment:    dw 0
dap_lba_low:    dd 0
dap_lba_high:   dd 0

; ============================================================================
; Data Section (16-bit accessible)
; ============================================================================

boot_drive:     db 0
boot_media:     db 0
vga_mode:       db 0
fb_address:     dd 0
fb_width:       dw 0
fb_height:      dw 0
fb_bpp:         db 0
fb_pitch:       dw 0

; Loading variables
sectors_left:   dw 0
current_lba:    dw 0
load_segment:   dw 0
load_offset:    dw 0
rom_sectors:    dw 0

; ROM info
rom_addr:       dd 0
rom_size:       dd 0
rom_title:      times 32 db 0

; Messages
msg_banner:     db 13, 10, '=== gb-os ===', 13, 10, 0
msg_media:      db ' Media: ', 0
msg_floppy:     db 'Floppy', 0
msg_cdrom:      db 'CD-ROM', 0
msg_hdd:        db 'HDD/USB', 0
msg_newline:    db 13, 10, 0
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
    mov     edi, KERNEL_DEST_ADDR
    mov     ecx, (KERNEL_SECTORS_FDD * 512) / 4
    cld
    rep movsd

%ifdef GAMEBOY_MODE
    ; Copy ROM from 0x40000 to 3MB (0x300000) if loaded
    mov     eax, [rom_addr]
    test    eax, eax
    jz      .no_rom_copy

    mov     esi, 0x40000            ; Source: temp buffer
    mov     edi, ROM_DEST_ADDR      ; Dest: 3MB
    mov     ecx, [rom_size]
    add     ecx, 3
    shr     ecx, 2                  ; Divide by 4 for dword copy
    rep movsd

    ; Update rom_addr to final location
    mov     dword [rom_addr], ROM_DEST_ADDR
%endif

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

    ; Boot media type at offset 72
    xor     eax, eax
    mov     al, [boot_media]
    mov     [0x500 + 72], eax

    ; Boot drive at offset 76
    xor     eax, eax
    mov     al, [boot_drive]
    mov     [0x500 + 76], eax

    ; Quick visual confirmation - draw corner pixels
    mov     byte [0xA0000], 0x0F            ; Top-left white
    mov     byte [0xA013F], 0x0F            ; Top-right white (319)
    mov     byte [0xAF8C0], 0x0F            ; Bottom-left white (199*320)
    mov     byte [0xAF9FF], 0x0F            ; Bottom-right white

    ; Jump to kernel with boot info pointer in EAX
    mov     eax, 0x500
    jmp     KERNEL_DEST_ADDR

; ============================================================================
; Pad to exactly 16KB (32 sectors of 512 bytes = 8 CD sectors of 2048 bytes)
; ============================================================================

times 16384 - ($ - $$) db 0
