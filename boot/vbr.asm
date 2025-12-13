; ============================================================================
; vbr.asm - Volume Boot Record for RetroFutureGB System Partition
; ============================================================================
;
; This is the first sector of the SYSTEM partition. It:
;   1. Sets up segments and stack
;   2. Passes partition info to stage 2 via boot info structure
;   3. Loads the stage 2 bootloader from the partition
;   4. Jumps to stage 2
;
; The system partition layout:
;   Sector 0:     This VBR
;   Sector 1-32:  Stage 2 bootloader (16KB)
;   Sector 33+:   Kernel and data
;
; On entry from MBR:
;   DL = boot drive
;   SI = pointer to partition entry (in MBR at 0x0600)
;
; Boot Info Protocol:
;   We write boot info at 0x500 with:
;     0x00: Magic 'VBRP' (indicates partition boot)
;     0x04: Partition start LBA (32-bit)
;     0x08: Boot drive
;
; Assemble: nasm -f bin -o vbr.bin vbr.asm
; ============================================================================

[BITS 16]
[ORG 0x7C00]

; ============================================================================
; Constants
; ============================================================================

BOOT_INFO_ADDR      equ 0x0500
VBR_MAGIC           equ 0x50524256  ; 'VBRP' - VBR Partition boot

STAGE2_LOAD_ADDR    equ 0x7E00      ; Load stage 2 here
STAGE2_SECTORS      equ 32          ; 16KB for stage 2
STAGE2_START_SECTOR equ 1           ; Relative to partition start

STAGE2_MAGIC        equ 0x5247      ; 'GR' - expected at start of stage2

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
    mov     sp, 0x7C00
    sti

    ; Save boot drive
    mov     [boot_drive], dl

    ; Get partition start LBA from MBR partition entry
    ; SI points to the 16-byte partition entry (at 0x0600 + 0x1BE typically)
    ; But we need to handle the case where SI might not be valid

    ; First, check if SI points to a valid partition entry
    ; by checking if we can read the LBA from it
    mov     eax, [si + 8]           ; Offset 8 = Starting LBA
    test    eax, eax
    jz      .find_partition         ; If 0, try to find our partition
    mov     [partition_start], eax
    jmp     .have_partition

.find_partition:
    ; MBR didn't pass valid partition info - try to find ourselves
    ; Read MBR and look for our partition (type 0x7F)
    mov     eax, 0
    mov     cx, 1
    mov     bx, 0x0600              ; Read MBR to 0x0600
    call    read_sector_lba
    jc      .error_partition

    ; Scan partition table at 0x0600 + 0x1BE
    mov     si, 0x0600 + 0x1BE
    mov     cx, 4
.scan_loop:
    cmp     byte [si + 4], 0x7F     ; Our partition type
    je      .found_it
    add     si, 16
    loop    .scan_loop
    jmp     .error_partition

.found_it:
    mov     eax, [si + 8]
    mov     [partition_start], eax

.have_partition:
    ; Write boot info structure at 0x500
    mov     di, BOOT_INFO_ADDR

    ; Magic 'VBRP'
    mov     dword [di], VBR_MAGIC

    ; Partition start LBA
    mov     eax, [partition_start]
    mov     [di + 4], eax

    ; Boot drive
    mov     al, [boot_drive]
    mov     [di + 8], al

    ; Display boot message
    mov     si, msg_vbr
    call    print_string

    ; Check for LBA extensions
    mov     ah, 0x41
    mov     bx, 0x55AA
    mov     dl, [boot_drive]
    int     0x13
    jc      .no_lba
    cmp     bx, 0xAA55
    jne     .no_lba
    mov     byte [use_lba], 1
    jmp     .load_stage2

.no_lba:
    mov     byte [use_lba], 0

.load_stage2:
    ; Calculate absolute LBA of stage 2
    mov     eax, [partition_start]
    add     eax, STAGE2_START_SECTOR
    mov     [current_lba], eax

    ; Set up destination
    mov     word [load_segment], STAGE2_LOAD_ADDR >> 4
    mov     word [load_offset], 0
    mov     word [sectors_remaining], STAGE2_SECTORS

.load_loop:
    cmp     word [sectors_remaining], 0
    je      .load_done

    ; How many sectors this iteration? (max 64 for safety)
    mov     ax, [sectors_remaining]
    cmp     ax, 64
    jbe     .count_ok
    mov     ax, 64
.count_ok:
    mov     [sectors_to_read], ax

    cmp     byte [use_lba], 1
    je      .read_lba

    ; CHS read (fallback for very old systems)
    ; This is simplified and may not work for large partitions
    mov     ax, 0x0201              ; Read 1 sector
    mov     bx, [load_offset]
    mov     es, [load_segment]
    mov     cx, 0x0002              ; Cylinder 0, Sector 2
    mov     dh, 0                   ; Head 0
    mov     dl, [boot_drive]
    int     0x13
    jc      disk_error

    ; Only read 1 sector at a time in CHS mode
    mov     word [sectors_to_read], 1
    jmp     .advance

.read_lba:
    ; Set up DAP
    mov     eax, [current_lba]
    mov     [dap_lba], eax
    mov     word [dap_lba + 4], 0
    mov     ax, [sectors_to_read]
    mov     [dap_count], ax
    mov     ax, [load_offset]
    mov     [dap_offset], ax
    mov     ax, [load_segment]
    mov     [dap_segment], ax

    ; Perform LBA read
    mov     si, dap
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jc      disk_error

.advance:
    ; Progress indicator
    mov     al, '.'
    call    print_char

    ; Update counters
    mov     ax, [sectors_to_read]
    sub     [sectors_remaining], ax

    ; Update LBA
    movzx   eax, word [sectors_to_read]
    add     [current_lba], eax

    ; Update load address
    mov     ax, [sectors_to_read]
    shl     ax, 5                   ; * 32 (paragraphs per sector)
    add     [load_segment], ax

    jmp     .load_loop

.load_done:
    ; Verify stage 2 magic
    mov     ax, STAGE2_LOAD_ADDR >> 4
    mov     es, ax
    xor     bx, bx
    cmp     word [es:bx], STAGE2_MAGIC
    jne     invalid_stage2

    ; Pass boot info to stage 2:
    ;   DL = boot drive
    mov     dl, [boot_drive]

    ; Jump to stage 2 (skip the 2-byte magic)
    jmp     0x0000:STAGE2_LOAD_ADDR + 2

.error_partition:
    mov     si, msg_no_part
    call    print_string
    jmp     halt

; ============================================================================
; read_sector_lba - Read single sector using LBA
; Input: EAX = LBA, BX = buffer offset (segment 0)
; ============================================================================

read_sector_lba:
    push    si

    mov     [dap_lba], eax
    mov     word [dap_lba + 4], 0
    mov     word [dap_count], 1
    mov     [dap_offset], bx
    mov     word [dap_segment], 0

    mov     si, dap
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13

    pop     si
    ret

; ============================================================================
; Error Handlers
; ============================================================================

disk_error:
    mov     si, msg_disk_err
    call    print_string
    jmp     halt

invalid_stage2:
    mov     si, msg_invalid
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
; print_char - Print single character
; ============================================================================

print_char:
    push    ax
    push    bx
    mov     ah, 0x0E
    mov     bx, 0x0007
    int     0x10
    pop     bx
    pop     ax
    ret

; ============================================================================
; Data
; ============================================================================

boot_drive:         db 0
use_lba:            db 0
partition_start:    dd 0
current_lba:        dd 0
load_segment:       dw 0
load_offset:        dw 0
sectors_remaining:  dw 0
sectors_to_read:    dw 0

; Disk Address Packet
align 4
dap:
    db 0x10                     ; Size of DAP
    db 0                        ; Reserved
dap_count:
    dw 0                        ; Sector count
dap_offset:
    dw 0                        ; Offset
dap_segment:
    dw 0                        ; Segment
dap_lba:
    dd 0                        ; LBA low
    dd 0                        ; LBA high

; Messages
msg_vbr:        db 'VBR', 0
msg_disk_err:   db ' Disk!', 0
msg_invalid:    db ' Bad!', 0
msg_no_part:    db ' NoPart!', 0

; ============================================================================
; Padding and Signature
; ============================================================================

times 510 - ($ - $$) db 0
dw 0xAA55
