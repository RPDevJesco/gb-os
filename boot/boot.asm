; ============================================================================
; boot.asm - Stage 1 Bootloader for RetroFuture GB
; ============================================================================
;
; Minimal 512-byte boot sector using LBA extensions (no floppy emulation).
; Loads stage 2 and jumps to it.
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

    ; Check for LBA extensions support
    mov     ah, 0x41
    mov     bx, 0x55AA
    mov     dl, [BOOT_DRIVE_ADDR]
    int     0x13
    jc      .no_lba
    cmp     bx, 0xAA55
    jne     .no_lba
    jmp     .load_stage2

.no_lba:
    mov     si, msg_no_lba
    jmp     halt

.load_stage2:
    ; Load stage 2 using LBA extensions
    ; Set up DAP (Disk Address Packet)
    mov     word [dap_sectors], STAGE2_SECTORS
    mov     word [dap_offset], STAGE2_OFFSET
    mov     word [dap_segment], 0
    mov     dword [dap_lba_lo], 1    ; Start at LBA 1 (after boot sector)
    mov     dword [dap_lba_hi], 0

    ; Perform LBA read
    mov     si, dap
    mov     ah, 0x42
    mov     dl, [BOOT_DRIVE_ADDR]
    int     0x13
    jc      disk_error

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
; Disk Address Packet (DAP) for LBA reads
; ============================================================================

align 4
dap:
    db 0x10                     ; Size of DAP (16 bytes)
    db 0                        ; Reserved
dap_sectors:
    dw 0                        ; Number of sectors to read
dap_offset:
    dw 0                        ; Destination offset
dap_segment:
    dw 0                        ; Destination segment
dap_lba_lo:
    dd 0                        ; LBA low 32 bits
dap_lba_hi:
    dd 0                        ; LBA high 32 bits

; ============================================================================
; Data
; ============================================================================

msg_boot:       db 'RetroFutureGB', 13, 10, 'Loading', 0
msg_ok:         db ' OK', 13, 10, 0
msg_disk_err:   db 13, 10, 'Disk error!', 0
msg_stage2_err: db 13, 10, 'Stage2 bad!', 0
msg_no_lba:     db 13, 10, 'No LBA!', 0

; ============================================================================
; Boot Sector Padding and Signature
; ============================================================================

times 510 - ($ - $$) db 0
dw 0xAA55
