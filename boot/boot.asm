; ============================================================================
; boot.asm - Stage 1 Bootloader for gb-os (Compact)
; ============================================================================
;
; Minimal boot sector: detect media, load stage2, jump.
;   - DL < 0x80: Floppy (CHS)
;   - DL >= 0x80: HDD/CD (LBA)
;
; Assemble: nasm -f bin -o boot.bin boot.asm
; ============================================================================

[BITS 16]
[ORG 0x7C00]

STAGE2      equ 0x7E00
SECTORS     equ 32                  ; 16KB for stage2
BOOT_INFO   equ 0x0500

start:
    cli
    xor     ax, ax
    mov     ds, ax
    mov     es, ax
    mov     ss, ax
    mov     sp, 0x7C00
    sti

    mov     [boot_drv], dl
    mov     [BOOT_INFO], dl         ; Pass to stage2

    mov     si, msg
    call    puts

    ; Floppy or LBA?
    cmp     dl, 0x80
    jb      .floppy

    ; Check LBA support
    mov     ah, 0x41
    mov     bx, 0x55AA
    int     0x13
    jc      .floppy
    cmp     bx, 0xAA55
    jne     .floppy

    ; === LBA Mode ===
    mov     byte [BOOT_INFO+1], 1
    mov     cx, SECTORS
    mov     word [dap_buf], STAGE2
    mov     dword [dap_lba], 1

.lba_loop:
    mov     si, dap
    mov     dl, [boot_drv]
    mov     ah, 0x42
    int     0x13
    jc      .err
    add     word [dap_buf], 512
    inc     dword [dap_lba]
    loop    .lba_loop
    jmp     .done

    ; === Floppy Mode (CHS) ===
.floppy:
    mov     byte [BOOT_INFO+1], 0
    xor     ax, ax
    mov     dl, [boot_drv]
    int     0x13                    ; Reset

    mov     di, STAGE2
    mov     cx, SECTORS
    mov     word [lba], 1

.fdd_loop:
    push    cx

    ; LBA -> CHS (18 spt, 2 heads)
    mov     ax, [lba]
    xor     dx, dx
    mov     bx, 18
    div     bx                      ; AX=cyl*2+head, DX=sect
    mov     cl, dl
    inc     cl                      ; Sector 1-18
    xor     dx, dx
    shr     ax, 1                   ; AX=cyl, CF=head
    mov     ch, al
    adc     dh, 0                   ; DH=head

    mov     bx, di
    mov     ax, 0x0201
    mov     dl, [boot_drv]
    int     0x13
    jc      .err

    add     di, 512
    inc     word [lba]
    pop     cx
    loop    .fdd_loop

.done:
    mov     si, msg_ok
    call    puts
    cmp     word [STAGE2], 0x5247   ; 'GR' magic
    jne     .err
    mov     dl, [boot_drv]
    jmp     STAGE2 + 2

.err:
    mov     si, msg_er
    call    puts
.hlt:
    cli
    hlt
    jmp     .hlt

puts:
    mov     ah, 0x0E
.lp:
    lodsb
    test    al, al
    jz      .dn
    int     0x10
    jmp     .lp
.dn:
    ret

; Data
msg:        db 'gb-os', 0
msg_ok:     db ' OK', 13, 10, 0
msg_er:     db ' E!', 0
boot_drv:   db 0
lba:        dw 0

; DAP (16 bytes)
dap:        db 0x10, 0
            dw 1
dap_buf:    dw STAGE2
            dw 0
dap_lba:    dd 0, 0

times 510 - ($ - $$) db 0
dw 0xAA55
