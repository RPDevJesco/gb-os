; ============================================================================
; boot.asm - El Torito No-Emulation Boot
; ============================================================================

[BITS 16]
[ORG 0x7C00]

STAGE2_ADDR     equ 0x7E00
STAGE2_MAGIC    equ 0x5247
STAGE2_LOADED   equ 1536        ; Bytes of stage2 in first CD sector
CD_SECTORS_NEED equ 8           ; CD sectors for rest of stage2

start:
    jmp     short real_start
    nop
times 8 - ($ - $$) db 0
bi_pvd:      dd 0
bi_file:     dd 0
bi_length:   dd 0
bi_csum:     dd 0
times 64 - ($ - $$) db 0

real_start:
    cli
    xor     ax, ax
    mov     ds, ax
    mov     es, ax
    mov     ss, ax
    mov     sp, 0x7C00
    sti
    mov     [boot_drive], dl

    ; Store CD boot info for stage2:
    ; 0x500: 'CDRM' magic (not VBR)
    ; 0x504: bi_file (CD sector of boot image)
    ; 0x508: sector size (2048 for CD)
    mov     dword [0x500], 'CDRM'
    mov     eax, [bi_file]
    mov     [0x504], eax
    mov     dword [0x508], 2048

    mov     ax, 0x0003
    int     0x10

    ; Verify stage2 magic preloaded at 0x7E00
    cmp     word [STAGE2_ADDR], STAGE2_MAGIC
    jne     .err
    mov     al, 'P'
    call    pch

    ; Load rest of stage2 from CD sector bi_file+1
    mov     eax, [bi_file]
    inc     eax
    mov     [dap_lba], eax
    mov     dword [dap_lba+4], 0
    mov     word [dap_cnt], CD_SECTORS_NEED
    mov     word [dap_off], 0
    mov     word [dap_seg], (STAGE2_ADDR + STAGE2_LOADED) >> 4

    mov     si, dap
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jc      .err

    mov     al, '!'
    call    pch

    mov     dl, [boot_drive]
    jmp     0x0000:STAGE2_ADDR + 2

.err:
    mov     al, 'X'
    call    pch
    cli
.h: hlt
    jmp     .h

pch:
    mov     ah, 0x0E
    int     0x10
    ret

boot_drive: db 0
align 4
dap:        db 0x10, 0
dap_cnt:    dw 0
dap_off:    dw 0
dap_seg:    dw 0
dap_lba:    dq 0

times 510 - ($ - $$) db 0
dw 0xAA55
