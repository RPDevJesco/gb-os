; ============================================================================
; boot.asm - Stage 1 Bootloader for GameBoy OS
; ============================================================================
; 
; Fits in 512-byte boot sector. Loads stage 2 and jumps to it.
;
; Memory map during boot:
;   0x0000:0x7C00  - This bootloader (512 bytes)
;   0x0000:0x7E00  - Stage 2 loaded here
;   0x0000:0x0500  - Boot info structure
;
; Assemble: nasm -f bin -o boot.bin boot.asm
; ============================================================================

[BITS 16]
[ORG 0x7C00]

; ============================================================================
; Constants
; ============================================================================

STAGE2_SEGMENT  equ 0x0000
STAGE2_OFFSET   equ 0x7E00
STAGE2_SECTORS  equ 32          ; 16KB for stage 2
BOOT_DRIVE      equ 0x0500

; Floppy geometry (1.44MB)
SECTORS_PER_TRACK equ 18
HEADS           equ 2

; ============================================================================
; Entry Point
; ============================================================================

start:
    cli
    xor     ax, ax
    mov     ds, ax
    mov     es, ax
    mov     ss, ax
    mov     sp, 0x7C00
    sti

    mov     [BOOT_DRIVE], dl

    ; Clear screen
    mov     ax, 0x0003
    int     0x10

    ; Display loading message
    mov     si, msg_loading
    call    print_string

    ; Reset disk
    xor     ax, ax
    mov     dl, [BOOT_DRIVE]
    int     0x13
    jc      disk_error

    ; Load stage 2
    mov     word [cur_lba], 1
    mov     word [sectors_rem], STAGE2_SECTORS
    mov     word [dest_ptr], STAGE2_OFFSET

.load_loop:
    cmp     word [sectors_rem], 0
    je      .load_done

    ; Convert LBA to CHS
    mov     ax, [cur_lba]
    xor     dx, dx
    mov     cx, SECTORS_PER_TRACK
    div     cx                  ; AX = track, DX = sector-1
    push    dx
    xor     dx, dx
    mov     cx, HEADS
    div     cx                  ; AX = cylinder, DX = head
    mov     ch, al              ; Cylinder
    mov     dh, dl              ; Head
    pop     ax
    inc     al
    mov     cl, al              ; Sector

    ; Read one sector
    mov     ax, STAGE2_SEGMENT
    mov     es, ax
    mov     bx, [dest_ptr]

    mov     ah, 0x02
    mov     al, 1
    mov     dl, [BOOT_DRIVE]
    int     0x13
    jc      disk_error

    ; Progress dot
    mov     al, '.'
    mov     ah, 0x0E
    int     0x10

    ; Next sector
    inc     word [cur_lba]
    dec     word [sectors_rem]
    add     word [dest_ptr], 512
    jmp     .load_loop

.load_done:
    ; Verify stage 2 magic
    cmp     word [STAGE2_OFFSET], 0x5441  ; 'AT'
    jne     magic_error

    mov     si, msg_ok
    call    print_string

    ; Jump to stage 2
    mov     dl, [BOOT_DRIVE]
    jmp     0x0000:STAGE2_OFFSET + 2

; ============================================================================
; Error Handlers
; ============================================================================

disk_error:
    mov     si, msg_disk_err
    call    print_string
    jmp     halt

magic_error:
    mov     si, msg_magic_err
    call    print_string
    jmp     halt

halt:
    cli
    hlt
    jmp     halt

; ============================================================================
; Print String (SI = pointer)
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

; ============================================================================
; Data
; ============================================================================

cur_lba:        dw 0
sectors_rem:    dw 0
dest_ptr:       dw 0

msg_loading:    db 'GameBoy OS', 13, 10, 'Loading', 0
msg_ok:         db ' OK', 13, 10, 0
msg_disk_err:   db 13, 10, 'Disk error!', 0
msg_magic_err:  db 13, 10, 'Bad stage2!', 0

; ============================================================================
; Boot Signature
; ============================================================================

times 510 - ($ - $$) db 0
dw 0xAA55
