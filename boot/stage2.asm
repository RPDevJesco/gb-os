; ============================================================================
; stage2.asm - Stage 2 Bootloader for RetroFuture GB (Universal Boot)
; ============================================================================
;
; Universal bootloader that works from:
;   - Floppy disk (legacy)
;   - Hard disk partition (installed via installer)
;   - CD-ROM (El Torito no-emulation mode)
;   - USB drive
;
; Boot Info Protocol:
;   On entry, check for boot info at 0x500:
;     - If magic 'VBRP' present: booting from partition (use partition-relative LBA)
;     - Otherwise: booting from raw media (use absolute LBA)
;
; Steps:
;   1. Query E820 memory map
;   2. Enable A20 line
;   3. Set VGA mode 13h (320x200x256)
;   4. Load kernel to 0x100000 (1MB) using unreal mode
;   5. Load ROM to 0x300000 (3MB) if present
;   6. Switch to 32-bit protected mode
;   7. Jump to kernel
;
; Assemble: nasm -f bin -o stage2.bin stage2.asm
; ============================================================================

[BITS 16]
[ORG 0x7E00]

; ============================================================================
; Magic Number (verified by stage 1 or VBR)
; ============================================================================

dw 0x5247                       ; 'GR' magic (GameBoy Retro)

; ============================================================================
; Constants
; ============================================================================

BOOT_INFO_ADDR      equ 0x0500
VBR_MAGIC           equ 0x50524256  ; 'VBRP' - VBR passed info
E820_MAP_ADDR       equ 0x1000

; Kernel location
KERNEL_LOAD_SEG     equ 0x2000      ; Temporary load at 0x20000
KERNEL_LOAD_OFF     equ 0x0000
KERNEL_DEST_ADDR    equ 0x100000    ; Final location at 1MB
KERNEL_START_SECTOR equ 65          ; Relative sector for kernel (after stage2)
KERNEL_SECTORS      equ 256         ; 128KB max kernel

; ROM location
ROM_LOAD_SEG        equ 0x3000      ; Temporary at 0x30000
ROM_DEST_ADDR       equ 0x300000    ; Final at 3MB
ROM_HEADER_SECTOR   equ 321         ; After kernel (65 + 256)
ROM_MAX_SECTORS     equ 4096        ; 2MB max ROM

; ============================================================================
; Entry Point
; ============================================================================

stage2_entry:
    ; Save boot drive (passed in DL)
    mov     [boot_drive], dl

    ; Check for VBR boot info at 0x500 (installed HDD boot)
    mov     eax, [BOOT_INFO_ADDR]
    cmp     eax, VBR_MAGIC
    jne     .check_cd

    ; Partition boot - get partition start LBA from boot info
    mov     eax, [BOOT_INFO_ADDR + 4]   ; Partition start LBA
    mov     [partition_start], eax
    mov     byte [boot_type], 1         ; Partition boot
    mov     dword [cd_sector_size], 512 ; Normal sectors
    jmp     .continue

.check_cd:
    ; Check for CD boot marker 'CDRM' at 0x500
    cmp     eax, 'CDRM'
    jne     .raw_boot

    ; CD boot - get bi_file from 0x504
    mov     eax, [0x504]
    mov     [cd_base_sector], eax
    mov     byte [boot_type], 2         ; CD boot
    mov     dword [partition_start], 0
    mov     dword [cd_sector_size], 2048
    jmp     .continue

.raw_boot:
    ; Raw media boot (floppy, USB) - partition start is 0
    mov     dword [partition_start], 0
    mov     dword [cd_base_sector], 0
    mov     byte [boot_type], 0         ; Raw boot
    mov     dword [cd_sector_size], 512

.continue:
    ; Display banner
    mov     si, msg_banner
    call    print_string

    ; Check for LBA extensions
    call    check_lba_support
    mov     [use_lba], al

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

    mov     ax, 0x0013
    int     0x10

    mov     byte [vga_mode], 0x13
    mov     dword [fb_address], 0xA0000
    mov     word [fb_width], 320
    mov     word [fb_height], 200
    mov     byte [fb_bpp], 8
    mov     word [fb_pitch], 320

    mov     si, msg_ok
    call    print_string

    ; ------------------------------------------------------------------------
    ; Step 4: Enter Unreal Mode for >1MB access
    ; ------------------------------------------------------------------------
    mov     si, msg_unreal
    call    print_string

    call    enter_unreal_mode

    ; Test unreal mode: write to 1MB using FS, read back, verify
    mov     edi, 0x100000       ; 1MB
    mov     eax, 0x12345678
    a32 mov [fs:edi], eax       ; Write to 1MB using FS
    a32 mov eax, [fs:edi]       ; Read back using FS
    cmp     eax, 0x12345678
    jne     .unreal_fail

    mov     si, msg_ok
    call    print_string
    jmp     .step5

.unreal_fail:
    ; Print what we got back for debugging
    call    print_hex_dword
    mov     si, msg_fail
    call    print_string
    jmp     halt

    ; ------------------------------------------------------------------------
    ; Step 5: Load Kernel
    ; ------------------------------------------------------------------------
.step5:
    mov     si, msg_kernel
    call    print_string

    call    load_kernel
    jc      .kernel_fail

    mov     si, msg_ok
    call    print_string
    jmp     .step6

.kernel_fail:
    mov     si, msg_fail
    call    print_string
    jmp     halt

    ; ------------------------------------------------------------------------
    ; Step 6: Load ROM (if present)
    ; ------------------------------------------------------------------------
.step6:
    mov     si, msg_rom
    call    print_string

    call    load_rom
    jc      .no_rom

    mov     si, msg_ok
    call    print_string

    ; DEBUG: Print first 8 bytes of ROM at 0x300000 using unreal mode
    ; Use FS for high memory access (FS has 4GB limit)
    mov     si, msg_romdbg
    call    print_string

    mov     ebx, ROM_DEST_ADDR      ; 0x300000
    mov     cx, 8
.dbg_loop:
    a32 mov al, [fs:ebx]            ; Read byte from 0x300000+ using FS
    call    print_hex_byte
    mov     al, ' '
    call    print_char
    inc     ebx
    loop    .dbg_loop

    mov     si, msg_crlf
    call    print_string

    jmp     .step7

.no_rom:
    mov     si, msg_none
    call    print_string

    ; ------------------------------------------------------------------------
    ; Step 7: Build boot info structure and switch to protected mode
    ; ------------------------------------------------------------------------
.step7:
    mov     si, msg_boot
    call    print_string

    call    build_boot_info

    ; Disable interrupts for mode switch
    cli

    ; Load GDT
    lgdt    [gdt_descriptor]

    ; Enable protected mode
    mov     eax, cr0
    or      eax, 1
    mov     cr0, eax

    ; Far jump to flush pipeline and load CS
    jmp     0x08:protected_mode_entry

; ============================================================================
; check_lba_support - Check if BIOS supports LBA extensions
; Returns: AL = 1 if LBA supported, 0 otherwise
; ============================================================================

check_lba_support:
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
; read_sectors_lba - Read sectors using LBA (with CHS fallback)
; Input:
;   EAX = LBA (relative to partition/image start, in 512-byte sectors)
;   CX  = Number of 512-byte sectors
;   ES:BX = Destination buffer
; Returns: CF set on error
; ============================================================================

read_sectors_lba:
    push    eax
    push    ebx
    push    ecx
    push    edx
    push    si
    push    edi

    ; Check if CD boot mode
    cmp     byte [boot_type], 2
    je      .cd_mode

    ; Normal mode: Add partition start to get absolute LBA
    add     eax, [partition_start]
    mov     [dap_lba], eax
    mov     dword [dap_lba + 4], 0

    ; Set up DAP
    mov     [dap_count], cx
    mov     [dap_offset], bx
    mov     ax, es
    mov     [dap_segment], ax

    ; Try LBA first
    cmp     byte [use_lba], 1
    jne     .try_chs

    mov     si, dap
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jnc     .success

.try_chs:
    ; Fall back to CHS for floppy/older systems
    mov     eax, [dap_lba]
    xor     edx, edx
    mov     ebx, 18
    div     ebx
    inc     dl
    mov     cl, dl
    xor     edx, edx
    mov     ebx, 2
    div     ebx
    mov     dh, dl
    mov     ch, al

    mov     ax, es
    push    ax
    mov     ax, [dap_segment]
    mov     es, ax
    mov     bx, [dap_offset]
    mov     al, [dap_count]
    mov     ah, 0x02
    mov     dl, [boot_drive]
    int     0x13
    pop     ax
    mov     es, ax
    jc      .error
    jmp     .success

.cd_mode:
    ; CD mode: sectors are 2048 bytes, need to convert
    ; EAX = 512-byte sector relative to boot image
    ; Convert to byte offset, then to CD sector

    ; Save destination
    mov     [cd_dest_seg], es
    mov     [cd_dest_off], bx
    mov     [cd_sectors_want], cx

    ; Calculate byte offset: byte_off = EAX * 512
    shl     eax, 9              ; * 512
    mov     [cd_byte_offset], eax

    ; Calculate CD sector: cd_sector = cd_base_sector + byte_off / 2048
    shr     eax, 11             ; / 2048
    add     eax, [cd_base_sector]
    mov     [cd_read_sector], eax

    ; Calculate offset within CD sector: cd_off = byte_off % 2048
    mov     eax, [cd_byte_offset]
    and     eax, 2047           ; % 2048
    mov     [cd_start_offset], eax

    ; Calculate total bytes needed
    movzx   eax, word [cd_sectors_want]
    shl     eax, 9              ; * 512
    mov     [cd_bytes_need], eax

    ; Calculate how many CD sectors to read
    ; cd_sectors = ceil((cd_start_offset + cd_bytes_need) / 2048)
    add     eax, [cd_start_offset]
    add     eax, 2047           ; for ceiling
    shr     eax, 11             ; / 2048
    mov     [cd_sectors_read], eax

.cd_read_loop:
    cmp     dword [cd_bytes_need], 0
    je      .success

    ; Read one CD sector to temp buffer
    mov     eax, [cd_read_sector]
    mov     [dap_lba], eax
    mov     dword [dap_lba + 4], 0
    mov     word [dap_count], 1
    mov     word [dap_offset], 0
    mov     word [dap_segment], CD_TEMP_SEG

    mov     si, dap
    mov     ah, 0x42
    mov     dl, [boot_drive]
    int     0x13
    jc      .error

    ; Copy relevant bytes from temp buffer to destination
    ; Source: CD_TEMP_SEG:cd_start_offset
    ; Dest: cd_dest_seg:cd_dest_off
    ; Count: min(2048 - cd_start_offset, cd_bytes_need)

    mov     eax, 2048
    sub     eax, [cd_start_offset]  ; bytes available in this sector
    cmp     eax, [cd_bytes_need]
    jbe     .copy_count_ok
    mov     eax, [cd_bytes_need]
.copy_count_ok:
    mov     ecx, eax                ; ECX = bytes to copy

    ; Load all values while DS is still segment 0
    mov     si, word [cd_start_offset]  ; source offset in temp buffer
    mov     di, [cd_dest_off]           ; dest offset
    mov     ax, [cd_dest_seg]           ; dest segment
    mov     bx, ax                      ; save dest segment in BX

    ; Set up segments for copy - this WILL destroy unreal mode DS limit
    push    ds
    push    es

    mov     ax, CD_TEMP_SEG
    mov     ds, ax                  ; DS:SI = source (temp buffer)

    mov     es, bx                  ; ES:DI = destination

    ; Copy ECX bytes
    cld
    rep movsb

    pop     es
    pop     ds

    ; DS is now restored to 0 (from push/pop)
    ; FS still has 4GB limit for high memory access

    ; Update destination offset (DI was advanced by rep movsb amount)
    mov     [cd_dest_off], di

    ; Update bytes remaining (ECX is now 0, use original count)
    mov     eax, 2048
    sub     eax, [cd_start_offset]
    cmp     eax, [cd_bytes_need]
    jbe     .sub_ok
    mov     eax, [cd_bytes_need]
.sub_ok:
    sub     [cd_bytes_need], eax

    ; Next CD sector starts at offset 0
    mov     dword [cd_start_offset], 0
    inc     dword [cd_read_sector]

    jmp     .cd_read_loop

.restore_unreal:
    ; Quick unreal mode restore - restores FS for copy_high
    call    restore_unreal_fs
    ret

.success:
    clc
    jmp     .done

.error:
    stc

.done:
    pop     edi
    pop     si
    pop     edx
    pop     ecx
    pop     ebx
    pop     eax
    ret

; CD read temporary variables
CD_TEMP_SEG         equ 0x1000      ; Temp buffer for CD sector reads
cd_dest_seg:        dw 0
cd_dest_off:        dw 0
cd_sectors_want:    dw 0
cd_byte_offset:     dd 0
cd_read_sector:     dd 0
cd_start_offset:    dd 0
cd_bytes_need:      dd 0
cd_sectors_read:    dd 0

; ============================================================================
; enter_unreal_mode - Enable 32-bit addressing in real mode
; Uses FS for 4GB access (DS/ES may be reloaded by other code)
; ============================================================================

enter_unreal_mode:
    cli

    ; Load GDT
    lgdt    [gdt_descriptor]

    ; Enter protected mode
    mov     eax, cr0
    or      al, 1
    mov     cr0, eax

    ; Load FS and GS with 4GB limit (selector 0x10)
    ; These segments are rarely touched by other code
    mov     ax, 0x10
    mov     fs, ax
    mov     gs, ax
    ; Also load DS/ES for compatibility
    mov     ds, ax
    mov     es, ax

    ; Return to real mode
    mov     eax, cr0
    and     al, 0xFE
    mov     cr0, eax

    ; Restore DS/ES to 0 for normal code
    ; FS/GS retain 4GB limit since we don't touch them
    xor     ax, ax
    mov     ds, ax
    mov     es, ax

    sti
    ret

; ============================================================================
; load_kernel - Load kernel using LBA and copy to high memory
; ============================================================================

load_kernel:
    push    es
    push    eax
    push    ebx
    push    ecx

    ; Load kernel in chunks to temporary buffer, then copy to 1MB+
    mov     dword [load_dest], KERNEL_DEST_ADDR
    mov     word [sectors_left], KERNEL_SECTORS
    mov     dword [current_sector], KERNEL_START_SECTOR

.load_loop:
    cmp     word [sectors_left], 0
    je      .done

    ; Read up to 64 sectors at a time (32KB)
    mov     ax, [sectors_left]
    cmp     ax, 64
    jbe     .count_ok
    mov     ax, 64
.count_ok:
    mov     [read_count], ax

    ; Read to temporary buffer
    mov     ax, KERNEL_LOAD_SEG
    mov     es, ax
    xor     bx, bx
    mov     eax, [current_sector]
    mov     cx, [read_count]
    call    read_sectors_lba
    jc      .error

    ; Restore FS 4GB limit - BIOS int 13h may have clobbered it
    call    restore_unreal_fs

    ; Copy from temp buffer to high memory using unreal mode
    mov     esi, KERNEL_LOAD_SEG * 16   ; Source: 0x20000
    mov     edi, [load_dest]             ; Dest: 1MB+
    movzx   ecx, word [read_count]
    shl     ecx, 7                       ; sectors * 128 = dwords
    call    copy_high

    ; Update counters
    movzx   eax, word [read_count]
    add     [current_sector], eax
    sub     [sectors_left], ax
    shl     eax, 9                       ; sectors * 512 = bytes
    add     [load_dest], eax

    ; Progress indicator
    mov     al, '.'
    call    print_char
    jmp     .load_loop

.done:
    pop     ecx
    pop     ebx
    pop     eax
    pop     es
    clc
    ret

.error:
    pop     ecx
    pop     ebx
    pop     eax
    pop     es
    stc
    ret

; ============================================================================
; copy_high - Copy data to address above 1MB using unreal mode
; Input: ESI = source (physical), EDI = dest (physical), ECX = dword count
; Uses FS segment which has 4GB limit
; ============================================================================

copy_high:
    push    eax
    push    ecx
    push    esi
    push    edi

.loop:
    test    ecx, ecx
    jz      .done

    ; Use FS for 32-bit addressing (FS has 4GB limit from unreal mode)
    a32 mov eax, [fs:esi]
    a32 mov [fs:edi], eax
    add     esi, 4
    add     edi, 4
    dec     ecx
    jmp     .loop

.done:
    pop     edi
    pop     esi
    pop     ecx
    pop     eax
    ret

; ============================================================================
; restore_unreal_fs - Restore FS segment with 4GB limit after BIOS interrupts
; BIOS int 13h can clobber segment registers, breaking unreal mode.
; This function re-establishes FS with a 4GB limit for copy_high to work.
; ============================================================================

restore_unreal_fs:
    push    eax

    cli

    ; Enter protected mode briefly
    mov     eax, cr0
    or      al, 1
    mov     cr0, eax

    ; Load FS with 4GB data segment selector
    mov     ax, 0x10
    mov     fs, ax

    ; Return to real mode - FS retains 4GB limit in hidden descriptor cache
    mov     eax, cr0
    and     al, 0xFE
    mov     cr0, eax

    sti

    pop     eax
    ret

; ============================================================================
; load_rom - Load ROM if present
; ============================================================================

load_rom:
    push    es
    push    eax
    push    ebx
    push    ecx

    ; Read ROM header sector
    mov     ax, ROM_LOAD_SEG
    mov     es, ax
    xor     bx, bx
    mov     eax, ROM_HEADER_SECTOR
    mov     cx, 1
    call    read_sectors_lba
    jc      .no_rom

    ; Check for 'GBOY' magic
    mov     ax, ROM_LOAD_SEG
    mov     es, ax
    cmp     dword [es:0], 0x594F4247    ; 'GBOY'
    jne     .no_rom

    ; Get ROM size from header
    mov     eax, [es:4]
    test    eax, eax
    jz      .no_rom
    mov     [rom_size], eax

    ; Copy title (offset 8, 32 bytes)
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

    ; Clamp to maximum
    cmp     ax, ROM_MAX_SECTORS
    jbe     .size_ok
    mov     word [rom_sectors], ROM_MAX_SECTORS
.size_ok:

    ; Load ROM data
    mov     dword [load_dest], ROM_DEST_ADDR
    mov     ax, [rom_sectors]
    mov     [sectors_left], ax
    mov     dword [current_sector], ROM_HEADER_SECTOR + 1

.load_loop:
    cmp     word [sectors_left], 0
    je      .done

    mov     ax, [sectors_left]
    cmp     ax, 64
    jbe     .count_ok
    mov     ax, 64
.count_ok:
    mov     [read_count], ax

    mov     ax, ROM_LOAD_SEG
    mov     es, ax
    xor     bx, bx
    mov     eax, [current_sector]
    mov     cx, [read_count]
    call    read_sectors_lba
    jc      .error

    ; DEBUG: Print first 2 bytes from temp buffer before copy_high
    ; Read values from temp buffer using ES
    mov     al, [es:0]          ; First byte at ROM_LOAD_SEG:0
    mov     ah, [es:1]          ; Second byte
    push    ax                  ; Save both bytes

    ; Print them (DS is still 0, so print functions work)
    pop     ax
    push    ax
    call    print_hex_byte      ; Print first byte (in AL)
    pop     ax
    mov     al, ah
    call    print_hex_byte      ; Print second byte
    mov     al, ' '
    call    print_char

    ; Restore FS 4GB limit - BIOS int 13h may have clobbered it
    call    restore_unreal_fs

    ; Copy to high memory
    mov     esi, ROM_LOAD_SEG * 16
    mov     edi, [load_dest]
    movzx   ecx, word [read_count]
    shl     ecx, 7
    call    copy_high

    movzx   eax, word [read_count]
    add     [current_sector], eax
    sub     [sectors_left], ax
    shl     eax, 9
    add     [load_dest], eax

    mov     al, '.'
    call    print_char
    jmp     .load_loop

.done:
    mov     dword [rom_addr], ROM_DEST_ADDR
    pop     ecx
    pop     ebx
    pop     eax
    pop     es
    clc
    ret

.no_rom:
.error:
    mov     dword [rom_addr], 0
    mov     dword [rom_size], 0
    pop     ecx
    pop     ebx
    pop     eax
    pop     es
    stc
    ret

; ============================================================================
; Query E820 Memory Map
; ============================================================================

query_e820:
    push    es
    push    di
    push    ebx
    push    ecx
    push    edx

    xor     ax, ax
    mov     es, ax
    mov     di, E820_MAP_ADDR + 4
    xor     ebx, ebx
    mov     [e820_count], ebx

.loop:
    mov     eax, 0xE820
    mov     ecx, 24
    mov     edx, 0x534D4150
    int     0x15
    jc      .done

    cmp     eax, 0x534D4150
    jne     .error

    add     di, 24
    inc     dword [e820_count]

    test    ebx, ebx
    jz      .done
    jmp     .loop

.done:
    mov     eax, [e820_count]
    mov     [E820_MAP_ADDR], eax
    clc
    jmp     .exit

.error:
    stc

.exit:
    pop     edx
    pop     ecx
    pop     ebx
    pop     di
    pop     es
    ret

; ============================================================================
; Enable A20 Line
; ============================================================================

enable_a20:
    ; Try BIOS method
    mov     ax, 0x2401
    int     0x15
    jnc     .done

    ; Try keyboard controller method
    call    .wait_kbd
    mov     al, 0xAD
    out     0x64, al
    call    .wait_kbd
    mov     al, 0xD0
    out     0x64, al
    call    .wait_kbd2
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

.wait_kbd2:
    in      al, 0x64
    test    al, 1
    jz      .wait_kbd2
    ret

; ============================================================================
; Verify A20 Line
; ============================================================================

verify_a20:
    push    es
    push    di
    push    si

    ; Test using 0x600/0x610 to avoid boot_info at 0x500
    xor     ax, ax
    mov     es, ax
    mov     di, 0x0600

    mov     ax, 0xFFFF
    mov     ds, ax
    mov     si, 0x0610

    mov     byte [es:di], 0x00
    mov     byte [ds:si], 0xFF

    cmp     byte [es:di], 0xFF
    je      .a20_disabled

    xor     ax, ax
    mov     ds, ax
    clc
    jmp     .done

.a20_disabled:
    xor     ax, ax
    mov     ds, ax
    stc

.done:
    pop     si
    pop     di
    pop     es
    ret

; ============================================================================
; Build Boot Info Structure
; ============================================================================

build_boot_info:
    push    es
    push    di

    xor     ax, ax
    mov     es, ax
    mov     di, BOOT_INFO_ADDR

    ; Magic: 'GBOY'
    mov     dword [es:di], 0x594F4247
    add     di, 4

    ; E820 map address
    mov     dword [es:di], E820_MAP_ADDR
    add     di, 4

    ; VGA mode
    movzx   eax, byte [vga_mode]
    mov     [es:di], eax
    add     di, 4

    ; Framebuffer address
    mov     eax, [fb_address]
    mov     [es:di], eax
    add     di, 4

    ; Screen dimensions
    movzx   eax, word [fb_width]
    mov     [es:di], eax
    add     di, 4
    movzx   eax, word [fb_height]
    mov     [es:di], eax
    add     di, 4

    ; BPP
    movzx   eax, byte [fb_bpp]
    mov     [es:di], eax
    add     di, 4

    ; Pitch
    movzx   eax, word [fb_pitch]
    mov     [es:di], eax
    add     di, 4

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
    mov     al, [si]
    mov     [es:di], al
    inc     si
    inc     di
    loop    .copy_title

    ; Boot type (offset 0x48)
    movzx   eax, byte [boot_type]
    mov     [es:di], eax
    add     di, 4

    ; Boot drive (offset 0x4C)
    movzx   eax, byte [boot_drive]
    mov     [es:di], eax
    add     di, 4

    ; Partition start LBA (offset 0x50)
    mov     eax, [partition_start]
    mov     [es:di], eax

    pop     di
    pop     es
    ret

; ============================================================================
; Print routines
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

print_char:
    push    ax
    push    bx
    mov     ah, 0x0E
    mov     bx, 0x0007
    int     0x10
    pop     bx
    pop     ax
    ret

; Print AL as 2 hex digits
print_hex_byte:
    push    ax
    push    cx
    mov     cl, al
    shr     al, 4
    call    .hex_digit
    mov     al, cl
    and     al, 0x0F
    call    .hex_digit
    pop     cx
    pop     ax
    ret
.hex_digit:
    add     al, '0'
    cmp     al, '9'
    jbe     .print
    add     al, 7
.print:
    call    print_char
    ret

; Print EAX as 8 hex digits
print_hex_dword:
    push    eax
    shr     eax, 16
    call    print_hex_word
    pop     eax
    call    print_hex_word
    ret

; Print AX as 4 hex digits
print_hex_word:
    push    ax
    mov     al, ah
    call    print_hex_byte
    pop     ax
    call    print_hex_byte
    ret

halt:
    mov     si, msg_halt
    call    print_string
.loop:
    hlt
    jmp     .loop

; ============================================================================
; Data
; ============================================================================

boot_drive:         db 0
boot_type:          db 0            ; 0 = raw, 1 = partition, 2 = CD
use_lba:            db 0
partition_start:    dd 0
cd_base_sector:     dd 0            ; bi_file for CD boot
cd_sector_size:     dd 512          ; 512 for normal, 2048 for CD

; E820 data
e820_count:         dd 0

; VGA info
vga_mode:           db 0
fb_address:         dd 0
fb_width:           dw 0
fb_height:          dw 0
fb_bpp:             db 0
fb_pitch:           dw 0

; ROM info
rom_addr:           dd 0
rom_size:           dd 0
rom_title:          times 32 db 0
rom_sectors:        dw 0

; Load variables
sectors_left:       dw 0
current_sector:     dd 0
load_dest:          dd 0
read_count:         dw 0

; DAP (Disk Address Packet)
align 4
dap:
    db 0x10                     ; Size
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
msg_banner:     db 'RetroFutureGB Stage2', 13, 10, 0
msg_e820:       db '  E820 memory map... ', 0
msg_a20:        db '  A20 gate... ', 0
msg_vga:        db '  VGA mode 13h... ', 0
msg_unreal:     db '  Unreal mode... ', 0
msg_kernel:     db '  Loading kernel', 0
msg_rom:        db 13, 10, '  Loading ROM', 0
msg_boot:       db 13, 10, '  Starting kernel...', 13, 10, 0
msg_ok:         db 'OK', 13, 10, 0
msg_fail:       db 'FAIL', 13, 10, 0
msg_none:       db 'none', 13, 10, 0
msg_halt:       db 13, 10, 'System halted.', 0
msg_romdbg:     db '  ROM@3M: ', 0
msg_crlf:       db 13, 10, 0

; ============================================================================
; GDT
; ============================================================================

align 16
gdt_start:
    dq 0                            ; Null descriptor

    ; Code segment 0x08
    dw 0xFFFF
    dw 0
    db 0
    db 10011010b
    db 11001111b
    db 0

    ; Data segment 0x10
    dw 0xFFFF
    dw 0
    db 0
    db 10010010b
    db 11001111b
    db 0
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

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
    mov     esp, 0x90000

    ; Jump to kernel at 1MB
    jmp     KERNEL_DEST_ADDR

; ============================================================================
; Padding - ensure stage2 is exactly 16KB (32 sectors)
; ============================================================================

times 16384 - ($ - $$) db 0
