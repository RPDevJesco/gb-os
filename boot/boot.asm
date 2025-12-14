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

    ; Reset disk system first (improves compatibility)
    xor     ax, ax
    mov     dl, [BOOT_DRIVE_ADDR]
    int     0x13

    ; Check for LBA extensions support
    mov     ax, 0x4100              ; AH=41h, AL=0 (clean register)
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
    ; Read in small chunks (4 sectors at a time) for better compatibility
    ; Some BIOSes hang on large multi-sector reads
    mov     word [cur_lba], 1           ; Start at LBA 1 (after boot sector)
    mov     word [sectors_rem], STAGE2_SECTORS
    mov     word [dest_offset], STAGE2_OFFSET

.read_loop:
    cmp     word [sectors_rem], 0
    je      .read_done

    ; Calculate sectors to read this iteration (max 4)
    mov     ax, [sectors_rem]
    cmp     ax, 4
    jbe     .use_remaining
    mov     ax, 4
.use_remaining:
    mov     [dap_sectors], ax

    ; Set up DAP
    mov     ax, [dest_offset]
    mov     [dap_offset], ax
    mov     word [dap_segment], 0
    mov     eax, [cur_lba]
    mov     [dap_lba_lo], eax
    mov     dword [dap_lba_hi], 0

    ; Perform LBA read
    mov     si, dap
    mov     ax, 0x4200              ; AH=42h, AL=0 (clean register)
    mov     dl, [BOOT_DRIVE_ADDR]
    int     0x13
    jc      disk_error

    ; Print progress dot
    mov     ax, 0x0E2E
    int     0x10

    ; Advance to next chunk
    mov     ax, [dap_sectors]
    sub     [sectors_rem], ax
    add     [cur_lba], ax
    shl     ax, 9                   ; Multiply by 512
    add     [dest_offset], ax
    jmp     .read_loop

.read_done:
    ; Print OK
    mov     si, msg_ok
    call    print_string

    ; Debug: print first 2 bytes at stage2 location as hex
    mov     al, [STAGE2_OFFSET]
    call    print_hex_byte
    mov     al, [STAGE2_OFFSET + 1]
    call    print_hex_byte
    mov     al, ' '
    mov     ah, 0x0E
    int     0x10

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

; Print AL as 2 hex digits
print_hex_byte:
    push    ax
    push    bx
    mov     bl, al              ; Save original value
    shr     al, 4               ; High nibble
    call    .print_nibble
    mov     al, bl              ; Low nibble
    and     al, 0x0F
    call    .print_nibble
    pop     bx
    pop     ax
    ret
.print_nibble:
    cmp     al, 10
    jb      .digit
    add     al, 'A' - 10
    jmp     .print
.digit:
    add     al, '0'
.print:
    mov     ah, 0x0E
    int     0x10
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

; Variables for chunked reading
cur_lba:        dd 0                    ; Current LBA (32-bit for larger disks)
sectors_rem:    dw 0                    ; Sectors remaining to read
dest_offset:    dw 0                    ; Current destination offset

; ============================================================================
; Boot Sector Padding and Signature
; ============================================================================

times 510 - ($ - $$) db 0
dw 0xAA55
