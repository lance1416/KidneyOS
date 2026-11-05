/*
 * Linker script for the x86_64 kernel (`make ARCH=x86_64`).
 *
 * GRUB loads every segment at its physical (load) address and jumps to the
 * entry point in 32-bit protected mode with paging off, so the multiboot2
 * header and the boot stub (kernel/src/arch/x86_64/boot.s, sections .boot.*)
 * are linked at their physical addresses, below 4GiB. The rest of the kernel
 * is linked in the top 2GiB of the address space (as required by the kernel
 * code model) at `KERNEL_BASE + physical address`, where the boot stub maps
 * it.
 */
ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

/* Physical address the image is loaded at. */
KERNEL_PHYS = 0x100000;
/* Must match KERNEL_BASE in shared/src/mem/mod.rs. */
KERNEL_BASE = 0xFFFFFFFF80000000;

SECTIONS {
    . = KERNEL_PHYS;

    . += SIZEOF_HEADERS;

    .multiboot2_header : {
        KEEP(*(.multiboot2_header))
    }

    ASSERT(. < KERNEL_PHYS + 32768, "multiboot2 header must be within the first 32768 bytes.")

    .boot.text ALIGN(4K) : {
        boot_start = .;
        *(.boot.text)
    }

    .boot.rodata : {
        *(.boot.rodata)
    }

    /* NOBITS, but deliberately part of a loaded segment: the bootloader
       zeroes it and doesn't put its own data there. */
    .boot.bss ALIGN(4K) : {
        *(.boot.bss)
        . = ALIGN(4K);
    }

    boot_end = .;
}

SECTIONS {
    . += KERNEL_BASE;

    .text ALIGN(4K) : AT(ADDR(.text) - KERNEL_BASE) {
        kernel_start = .;
        *(.text .text.*)
    }

    .rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_BASE) {
        *(.rodata .rodata.*)
    }

    .data ALIGN(4K) : AT(ADDR(.data) - KERNEL_BASE) {
        kernel_data_start = .;
        *(.data .data.*)
        /* The prebuilt core is position independent; GOT entries the linker
           can't relax away end up here. */
        *(.got .got.*)
    }

    .bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_BASE) {
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4K);
    }

    kernel_end = .;

    /DISCARD/ : {
        *(.comment*)
        *(.eh_frame*)
        *(.gcc_except_table*)
        *(.note*)
        *(.rela.eh_frame*)
    }
}

/* Physical addresses of the kernel image (excluding the boot stub before
   it), for the kernel's frame allocator. */
kernel_start_phys = kernel_start - KERNEL_BASE;
kernel_data_start_phys = kernel_data_start - KERNEL_BASE;
kernel_end_phys = kernel_end - KERNEL_BASE;

ASSERT(boot_end <= kernel_start_phys, "the boot stub overlaps the kernel image")
ASSERT(kernel_end_phys <= 0x40000000, "the kernel image must fit in the 1GiB mapped by the boot stub")
