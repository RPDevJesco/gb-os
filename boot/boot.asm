; ============================================================================
; boot.asm - Stage 1 Bootloader for RetroFuture GB
; ============================================================================
;
; Minimal 512-byte boot sector. Loads stage 2 and jumps to it.
;
; Memory map:
;   0x7C00  - This bootloader (512 bytes)
;   0x7E00  - Stage 2 loaded here (16KB)
;   0x0500  - Boot info structure (passed to kernel)
;
; Assemble: nasm -f bin -o boot.bin boot.asm
; ============================================================================

[BITS 16]
[ORG 0x7C00]

; ============================================================================
; Constants
; ============================================================================

STAGE2_OFFSET   equ 0x7E00
STAGE2_SECTORS  equ 32              ; 16KB for stage 2
BOOT_DRIVE_ADDR equ 0x0500          ; Store boot drive here temporarily

; Floppy geometry (1.44MB)
SECTORS_PER_TRACK equ 18
HEADS           equ 2

; ============================================================================
; Entry Point
; ============================================================================

start:
    ; Set up segments
    cli
    xor     ax, ax
    mov     ds, ax
    mov     es, ax
    mov     ss, ax
    mov     sp, 0x7C00              ; Stack grows down from bootloader
    sti

    ; Save boot drive (BIOS passes it in DL)
    mov     [BOOT_DRIVE_ADDR], dl

    ; Set 80x25 text mode and clear screen
    mov     ax, 0x0003
    int     0x10

    ; Display loading message
    mov     si, msg_boot
    call    print_string

    ; Reset disk system
    xor     ax, ax
    mov     dl, [BOOT_DRIVE_ADDR]
    int     0x13
    jc      disk_error

    ; Load stage 2 sector by sector
    mov     word [cur_lba], 1       ; Start at LBA 1 (after boot sector)
    mov     word [sectors_rem], STAGE2_SECTORS
    mov     word [dest_ptr], STAGE2_OFFSET

.load_loop:
    cmp     word [sectors_rem], 0
    je      .load_done

    ; Convert LBA to CHS
    mov     ax, [cur_lba]
    xor     dx, dx
    mov     cx, SECTORS_PER_TRACK
    div     cx                      ; AX = track*heads + head, DX = sector-1
    push    dx                      ; Save sector-1
    xor     dx, dx
    mov     cx, HEADS
    div     cx                      ; AX = cylinder, DX = head
    mov     ch, al                  ; CH = cylinder
    mov     dh, dl                  ; DH = head
    pop     ax
    inc     al
    mov     cl, al                  ; CL = sector (1-based)

    ; Set up for read
    mov     bx, [dest_ptr]
    mov     si, 3                   ; Retry count

.retry:
    mov     ah, 0x02                ; BIOS read sectors
    mov     al, 1                   ; One sector at a time
    mov     dl, [BOOT_DRIVE_ADDR]
    int     0x13
    jnc     .read_ok

    ; Reset and retry
    xor     ax, ax
    mov     dl, [BOOT_DRIVE_ADDR]
    int     0x13
    dec     si
    jnz     .retry
    jmp     disk_error

.read_ok:
    ; Progress dot
    mov     ax, 0x0E2E
    int     0x10

    ; Advance
    add     word [dest_ptr], 512
    inc     word [cur_lba]
    dec     word [sectors_rem]
    jmp     .load_loop

.load_done:
    ; Print OK
    mov     si, msg_ok
    call    print_string

    ; Verify stage 2 magic
    cmp     word [STAGE2_OFFSET], 0x5247  ; 'GR' for GameBoy Retro
    jne     stage2_error

    ; Jump to stage 2 (skip magic bytes)
    mov     dl, [BOOT_DRIVE_ADDR]
    jmp     0x0000:STAGE2_OFFSET + 2

; ============================================================================
; Error Handlers
; ============================================================================

disk_error:
    mov     si, msg_disk_err
    jmp     halt

stage2_error:
    mov     si, msg_stage2_err
    jmp     halt

halt:
    call    print_string
    cli
.loop:
    hlt
    jmp     .loop

; ============================================================================
; Print String (SI = null-terminated string)
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
; Data
; ============================================================================

msg_boot:       db 'gb-os', 13, 10, 'Loading', 0
msg_ok:         db ' OK', 13, 10, 0
msg_disk_err:   db 13, 10, 'Disk error!', 0
msg_stage2_err: db 13, 10, 'Stage2 bad!', 0

; Variables
cur_lba:        dw 0
sectors_rem:    dw 0
dest_ptr:       dw 0

; ============================================================================
; Boot Sector Padding and Signature
; ============================================================================

times 510 - ($ - $$) db 0
dw 0xAA55
