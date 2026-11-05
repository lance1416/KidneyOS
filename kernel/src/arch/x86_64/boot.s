# x86_64 boot stub: the kernel's entry point (`_start`).
#
# GRUB loads the kernel with multiboot2 and jumps to `_start` in 32-bit
# protected mode with paging off (see "Machine state" in the multiboot2
# specification). This stub checks that the CPU supports long mode, maps
# memory, enables long mode and jumps to the Rust entry point `kmain` in the
# higher half, passing it the multiboot2 magic and boot information pointer:
#
#     extern "C" fn kmain(magic: u32, multiboot2_info_phys: usize) -> !
#
# Everything up to the jump to the higher half runs at its physical address,
# so it lives in the `.boot.*` sections, which build-support/x86_64.ld links
# at (virtual = physical) addresses below 4GiB. The temporary page tables map
# physical 0..1GiB twice, with 2MiB pages:
#
#   - identity mapped at 0..1GiB, for this stub (and, for now, the GDT and the
#     multiboot2 information);
#   - at KERNEL_BASE (-2GiB, 0xFFFFFFFF80000000) .. -1GiB, where the kernel is
#     linked.
#
# This file is assembled by `global_asm!` in boot.rs, in AT&T syntax.

.set MULTIBOOT2_HEADER_MAGIC, 0xE85250D6
.set MULTIBOOT2_ARCH_I386, 0
# What a multiboot2 bootloader leaves in eax (shared::multiboot2::EXPECTED_MAGIC).
.set MULTIBOOT2_BOOTLOADER_MAGIC, 0x36D76289

# Size of the stack kmain runs on.
.set KERNEL_STACK_SIZE, 0x10000

.set PAGE_SIZE, 0x1000
.set HUGE_PAGE_SIZE, 0x200000
# Page table entry flags.
.set PTE_PRESENT, 1 << 0
.set PTE_WRITABLE, 1 << 1
.set PTE_HUGE, 1 << 7

.set CR0_WP, 1 << 16
.set CR0_PG, 1 << 31
.set CR4_PAE, 1 << 5
.set MSR_EFER, 0xC0000080
.set EFER_LME, 1 << 8
.set EFER_NXE, 1 << 11
.set CPUID_EXT_LM, 1 << 29
.set CPUID_EXT_NX, 1 << 20
.set EFLAGS_ID, 1 << 21

# Selectors in boot_gdt.
.set BOOT_CODE_SELECTOR, 0x08
.set BOOT_DATA_SELECTOR, 0x10

.set COM1, 0x3F8
.set VGA_TEXT, 0xB8000

# ---------------------------------------------------------------------------
# Multiboot2 header. x86_64.ld puts it at the start of the image, within the
# first 32KiB as the specification requires.

.section .multiboot2_header, "a"
.balign 8
multiboot2_header:
    .long MULTIBOOT2_HEADER_MAGIC
    .long MULTIBOOT2_ARCH_I386
    .long multiboot2_header_end - multiboot2_header
    .long 0x100000000 - (MULTIBOOT2_HEADER_MAGIC + MULTIBOOT2_ARCH_I386 + (multiboot2_header_end - multiboot2_header))
    # End tag.
    .balign 8
    .short 0
    .short 0
    .long 8
multiboot2_header_end:

# ---------------------------------------------------------------------------
# 32-bit entry.

.section .boot.text, "ax"
.code32
.global _start
_start:
    cli
    cld
    mov $boot_stack32_top, %esp
    # cpuid clobbers eax, ebx, ecx and edx; keep the bootloader's magic and
    # boot information pointer in ebp and esi.
    mov %eax, %ebp
    mov %ebx, %esi

    mov $msg_no_multiboot2, %ebx
    cmp $MULTIBOOT2_BOOTLOADER_MAGIC, %ebp
    jne boot_error

    # The cpuid instruction is available iff the ID flag can be changed.
    mov $msg_no_long_mode, %ebx
    pushfl
    pop %eax
    mov %eax, %ecx
    xor $EFLAGS_ID, %eax
    push %eax
    popfl
    pushfl
    pop %eax
    push %ecx
    popfl
    cmp %eax, %ecx
    je boot_error

    # Long mode (and NX, which the kernel's page tables are going to rely
    # on) are reported in the extended function 0x80000001. cpuid overwrites
    # ebx, so the message is only loaded after it.
    mov $0x80000000, %eax
    cpuid
    mov $msg_no_long_mode, %ebx
    cmp $0x80000001, %eax
    jb boot_error
    mov $0x80000001, %eax
    cpuid
    mov $msg_no_long_mode, %ebx
    test $CPUID_EXT_LM, %edx
    jz boot_error
    mov $msg_no_nx, %ebx
    test $CPUID_EXT_NX, %edx
    jz boot_error

    # Clear the page tables. GRUB zeroes .boot.bss when it loads the kernel,
    # but don't depend on it.
    mov $boot_page_tables, %edi
    xor %eax, %eax
    mov $((boot_page_tables_end - boot_page_tables) / 4), %ecx
    rep stosl

    # PML4[0] -> boot_pdpt_low (0..512GiB), PML4[511] -> boot_pdpt_high
    # (-512GiB..0).
    mov $(boot_pdpt_low + PTE_PRESENT + PTE_WRITABLE), %eax
    mov %eax, boot_pml4
    mov $(boot_pdpt_high + PTE_PRESENT + PTE_WRITABLE), %eax
    mov %eax, boot_pml4 + 511 * 8
    # PDPT_low[0] (0..1GiB) and PDPT_high[510] (-2GiB..-1GiB) -> boot_pd.
    mov $(boot_pd + PTE_PRESENT + PTE_WRITABLE), %eax
    mov %eax, boot_pdpt_low
    mov %eax, boot_pdpt_high + 510 * 8
    # boot_pd maps physical 0..1GiB with 512 2MiB pages.
    mov $boot_pd, %edi
    mov $(PTE_PRESENT + PTE_WRITABLE + PTE_HUGE), %eax
    mov $512, %ecx
1:
    mov %eax, (%edi)
    add $HUGE_PAGE_SIZE, %eax
    add $8, %edi
    loop 1b

    mov $boot_pml4, %eax
    mov %eax, %cr3
    mov %cr4, %eax
    or $CR4_PAE, %eax
    mov %eax, %cr4
    mov $MSR_EFER, %ecx
    rdmsr
    or $(EFER_LME | EFER_NXE), %eax
    wrmsr
    # Enabling paging with EFER.LME set activates long mode (in compatibility
    # mode until a 64-bit code segment is loaded).
    mov %cr0, %eax
    or $(CR0_PG | CR0_WP), %eax
    mov %eax, %cr0

    lgdt boot_gdt_pointer
    ljmp $BOOT_CODE_SELECTOR, $long_mode_entry

# Prints the NUL-terminated message at ebx on the VGA text screen and COM1, and
# halts. GRUB leaves the screen in 80x25 text mode, and COM1 works without
# being configured in QEMU.
boot_error:
    mov $msg_boot_error, %esi
    mov $VGA_TEXT, %edi
    call boot_print
    mov %ebx, %esi
    call boot_print
2:
    cli
    hlt
    jmp 2b

# Prints the NUL-terminated message at esi, writing VGA cells from edi on.
boot_print:
    lodsb
    test %al, %al
    jz 4f
    mov %al, %cl
    mov $0x4F, %ah # white on red
    stosw
    mov $(COM1 + 5), %dx # line status register
3:
    in %dx, %al
    test $0x20, %al # transmitter holding register empty
    jz 3b
    mov $COM1, %dx
    mov %cl, %al
    out %al, %dx
    jmp boot_print
4:
    ret

# ---------------------------------------------------------------------------
# 64-bit code, still running at its physical address.

.code64
long_mode_entry:
    mov $BOOT_DATA_SELECTOR, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %ss
    xor %eax, %eax
    mov %ax, %fs
    mov %ax, %gs
    movabs $higher_half_entry, %rax
    jmp *%rax

# ---------------------------------------------------------------------------
# Read-only data used before the jump to the higher half.

.section .boot.rodata, "a"
.balign 8
# Flat 64-bit kernel code and data segments. A real GDT (with user segments and
# a TSS) is set up by the kernel later.
boot_gdt:
    .quad 0
    .quad 0x00AF9A000000FFFF # 0x08: code, DPL 0, long mode
    .quad 0x00CF92000000FFFF # 0x10: data, DPL 0
boot_gdt_end:

boot_gdt_pointer:
    .short boot_gdt_end - boot_gdt - 1
    # lgdt in 32-bit mode only reads the low 4 bytes.
    .quad boot_gdt

msg_boot_error:
    .asciz "KidneyOS x86_64 boot error: "
msg_no_multiboot2:
    .asciz "not loaded by a multiboot2 bootloader"
msg_no_long_mode:
    .asciz "this CPU doesn't support long mode (64-bit)"
msg_no_nx:
    .asciz "this CPU doesn't support the NX bit"

# ---------------------------------------------------------------------------
# Memory used before the jump to the higher half. Part of the loaded image (as
# NOBITS), so the bootloader doesn't put its boot information there.

.section .boot.bss, "aw", @nobits
.balign PAGE_SIZE
boot_page_tables:
boot_pml4:
    .skip PAGE_SIZE
boot_pdpt_low:
    .skip PAGE_SIZE
boot_pdpt_high:
    .skip PAGE_SIZE
boot_pd:
    .skip PAGE_SIZE
boot_page_tables_end:

.balign 16
    .skip 4096
boot_stack32_top:

# ---------------------------------------------------------------------------
# 64-bit code in the higher half.

.section .text.boot, "ax"
higher_half_entry:
    mov $kernel_stack_top, %rsp
    # The upper halves of the registers are undefined after the switch to long
    # mode; 32-bit moves zero them.
    mov %ebp, %edi # magic
    mov %esi, %esi # multiboot2 information (physical address)
    xor %ebp, %ebp # end of the frame pointer chain
    call kmain
5:
    cli
    hlt
    jmp 5b

.section .bss.kernel_stack, "aw", @nobits
.balign 16
kernel_stack_bottom:
    .skip KERNEL_STACK_SIZE
kernel_stack_top:
