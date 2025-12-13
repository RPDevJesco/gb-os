; ============================================================================
; boot.asm - Stage 1 Bootloader for RetroFuture GB (Universal Boot)
; ============================================================================
;
; Minimal 512-byte boot sector that works with any boot media:
;   - Floppy disk
;   - Hard disk
;   - USB drive
;   - CD-ROM (El Torito no-emulation)
;
; Uses LBA with CHS fallback. Loads stage 2 and jumps to it.
;
; Memory map:
;   0x0500  - Boot info structure (passed to stage2/kernel)
;   0x7C00  - This bootloader (512 bytes)
;   0x7E00  - Stage 2 loaded here (16KB)
;
; Assemble: nasm -f bin -o boot.bin boot.asm
; ============================================================================

[BITS 16]
[ORG 0x7C00]

; ============================================================================
; Constants
; ============================================================================

STAGE2_ADDR         equ 0x7E00
STAGE2_SECTORS      equ 32          ; 16KB for stage 2
STAGE2_MAGIC        equ 0x5247      ; 'GR' expected at start

; Fallback floppy geometry (for CHS mode)
SECTORS_PER_TRACK   equ 18
HEADS               equ 2

; ============================================================================
; Boot Info Table (patched by genisoimage -boot-info-table)
; Located at offset 8 in boot sector
; ============================================================================

; The boot info table is filled in by genisoimage/xorriso at these offsets:
;   Offset 8:  bi_pvd    - LBA of primary volume descriptor
;   Offset 12: bi_file   - LBA of boot file (THIS boot sector!)
;   Offset 16: bi_length - Length of boot file in bytes
;   Offset 20: bi_csum   - Checksum
;   Offset 24-56: reserved

; ============================================================================
; Entry Point
; ============================================================================

start:
    ; Jump over boot info table area
    jmp     short real_start
    nop

; Boot info table - filled by genisoimage -boot-info-table
; These MUST be at exact offsets from start of boot sector
times 8 - ($ - $$) db 0         ; Pad to offset 8
bi_pvd:     dd 0                 ; Offset 8:  Primary Volume Descriptor LBA
bi_file:    dd 0                 ; Offset 12: Boot file LBA (our location!)
bi_length:  dd 0                 ; Offset 16: Boot file length
bi_csum:    dd 0                 ; Offset 20: Checksum
bi_reserved: times 40 db 0       ; Offset 24-63: Reserved

real_start:
    ; Set up segments
    cli
    xor     ax, ax
    mov     ds, ax
    mov     es, ax
    mov     ss, ax
    mov     sp, 0x7C00              ; Stack grows down from bootloader
    sti

    ; Save boot drive (BIOS passes it in DL)
    mov     [boot_drive], dl

    ; Check if boot info table was patched (bi_file != 0)
    ; If so, we're booting from CD and need to use it as base LBA
    mov     eax, [bi_file]
    mov     [boot_lba_base], eax    ; Store for our own use

    ; Also write to 0x504 so stage2 can access it
    ; (0x500 is reserved for VBR magic check)
    mov     [0x504], eax

    ; Clear screen and set 80x25 text mode
    mov     ax, 0x0003
    int     0x10

    ; Display loading message
    mov     si, msg_boot
    call    print_string

    ; Check for LBA support
    call    check_lba
    mov     [use_lba], al

    ; Reset disk system
    xor     ax, ax
    mov     dl, [boot_drive]
    int     0x13

    ; Load stage 2
    call    load_stage2
    jc      load_error

    ; Verify stage 2 magic
    cmp     word [STAGE2_ADDR], STAGE2_MAGIC
    jne     bad_stage2

    ; Success - jump to stage 2
    ; Pass boot drive in DL
    mov     dl, [boot_drive]
    jmp     0x0000:STAGE2_ADDR + 2  ; Skip magic bytes

; ============================================================================
; check_lba - Check if BIOS supports LBA extensions
; Returns: AL = 1 if supported, 0 otherwise
; ============================================================================

check_lba:
    push    bx
    push    dx

    mov     ah, 0x41
    mov     bx, 0x55AA
    mov     dl, [boot_drive]
    int     0x13
    jc      .no_lba
    cmp     bx, 0xAA55
    jne     .no_lba

    mov     al, 1
    jmp     .done

.no_lba:
    xor     al, al

.done:
    pop     dx
    pop     bx
    ret

; ============================================================================
; load_stage2 - Load stage 2 bootloader
; Returns: CF set on error
; ============================================================================

load_stage2:
    push    es
    push    bx

    ; Initialize load parameters
    mov     word [cur_lba], 1           ; Stage 2 starts at sector 1
    mov     word [sectors_rem], STAGE2_SECTORS
    mov     word [dest_seg], STAGE2_ADDR >> 4
    mov     word [dest_off], 0

.load_loop:
    cmp     word [sectors_rem], 0
    je      .done

    ; Determine how many sectors to read (max 16 at a time)
    mov     ax, [sectors_rem]
    cmp     ax, 16
    jbe     .count_ok
    mov     ax, 16
.count_ok:
    mov     [read_count], ax

    ; Set up destination
    mov     ax, [dest_seg]
    mov     es, ax
    mov     bx, [dest_off]

    ; Try LBA if available
    cmp     byte [use_lba], 1
    jne     .use_chs

    ; LBA read - add boot_lba_base for CD boot support
    mov     ax, [read_count]
    mov     [dap_count], ax
    mov     ax, [dest_off]
    mov     [dap_off], ax
    mov     ax, [dest_seg]
    mov     [dap_seg], ax

    ; Calculate actual LBA = boot_lba_base + cur_lba
    mov     eax, [boot_lba_base]
    movzx   ebx, word [cur_lba]
    add     eax, ebx
    mov     [dap_lba], eax
    mov     dword [dap_lba + 4], 0

    mov     si, dap
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jnc     .advance
    ; Fall through to CHS on error

.use_chs:
    ; CHS read (one sector at a time for safety)
    mov     ax, [cur_lba]
    call    lba_to_chs

    mov     ax, [dest_seg]
    mov     es, ax
    mov     bx, [dest_off]

    mov     ah, 0x02
    mov     al, 1                       ; Read 1 sector in CHS mode
    mov     dl, [boot_drive]
    int     0x13
    jc      .error

    ; In CHS mode, only advance by 1
    mov     word [read_count], 1

.advance:
    ; Progress dot
    mov     al, '.'
    mov     ah, 0x0E
    xor     bx, bx
    int     0x10

    ; Update counters
    mov     ax, [read_count]
    sub     [sectors_rem], ax
    add     [cur_lba], ax

    ; Update destination (sectors * 512 / 16 = sectors * 32)
    mov     ax, [read_count]
    shl     ax, 5
    add     [dest_seg], ax

    jmp     .load_loop

.done:
    pop     bx
    pop     es
    clc
    ret

.error:
    pop     bx
    pop     es
    stc
    ret

; ============================================================================
; lba_to_chs - Convert LBA to CHS
; Input: AX = LBA
; Output: CH = cylinder, CL = sector, DH = head
; ============================================================================

lba_to_chs:
    push    bx

    ; Sector = (LBA % sectors_per_track) + 1
    xor     dx, dx
    mov     bx, SECTORS_PER_TRACK
    div     bx
    inc     dl
    mov     cl, dl              ; CL = sector

    ; Head = (LBA / sectors_per_track) % heads
    ; Cylinder = (LBA / sectors_per_track) / heads
    xor     dx, dx
    mov     bx, HEADS
    div     bx
    mov     dh, dl              ; DH = head
    mov     ch, al              ; CH = cylinder

    pop     bx
    ret

; ============================================================================
; Error Handlers
; ============================================================================

load_error:
    mov     si, msg_disk_err
    call    print_string
    jmp     halt

bad_stage2:
    mov     si, msg_stage2_err
    call    print_string
    jmp     halt

halt:
.loop:
    hlt
    jmp     .loop

; ============================================================================
; print_string - Print null-terminated string
; ============================================================================

print_string:
    pusha
    mov     ah, 0x0E
    mov     bx, 0x0007
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
; Data Section
; ============================================================================

boot_drive:     db 0
use_lba:        db 0
boot_lba_base:  dd 0                ; Base LBA from boot info table (CD boot)
cur_lba:        dw 0
sectors_rem:    dw 0
dest_seg:       dw 0
dest_off:       dw 0
read_count:     dw 0

; Disk Address Packet (must be aligned)
align 4
dap:
    db 0x10                     ; Size of packet (16 bytes)
    db 0                        ; Reserved
dap_count:
    dw 0                        ; Sectors to read
dap_off:
    dw 0                        ; Destination offset
dap_seg:
    dw 0                        ; Destination segment
dap_lba:
    dd 0                        ; LBA (low 32 bits)
    dd 0                        ; LBA (high 32 bits)

; Messages
msg_boot:       db 'RetroGB', 0
msg_disk_err:   db 13, 10, 'Disk!', 0
msg_stage2_err: db 13, 10, 'Stage2!', 0

; ============================================================================
; Boot Sector Padding and Signature
; ============================================================================

times 510 - ($ - $$) db 0
dw 0xAA55
