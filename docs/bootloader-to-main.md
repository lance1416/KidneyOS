# From Bootloader to `main`

It's not immediately obvious when looking at the repository where the execution of code "starts". This section provides an overview of how KidneyOS gets from the bootloader (the first code the hardware runs) to the kernel's main loop: GRUB, a 32-bit boot stub, long mode, `kmain`, and then the architecture-independent `start_kernel`.

## GRUB

KidneyOS uses GRUB 2 as its bootloader, booting from the BIOS (GRUB's `i386-pc` platform). GRUB locates the KidneyOS kernel within the ISO (`build-support/grub.cfg`), loads it into memory, and jumps to its entry point. The kernel is an ordinary 64-bit ELF file with a [Multiboot2](https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html) header, the standard GRUB 2 implements. Multiboot2 can't start a kernel in 64-bit mode, though: the [machine state section](https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html#Machine-state) of the specification describes what GRUB leaves behind, which is 32-bit protected mode with paging off. The magic value `0x36D76289` in `eax` shows that a Multiboot2 bootloader started the kernel, and `ebx` holds the physical address of the boot information, which includes the map of physical memory.

GRUB loads each segment of the kernel at its physical (load) address, and the code it jumps to must be at its physical address too. The linker script, `build-support/x86_64.ld`, therefore links the Multiboot2 header and the boot stub (the `.boot.*` sections) at their physical addresses, starting at 1MiB, and the rest of the kernel at `KERNEL_BASE` (`0xFFFFFFFF80000000`, the top 2GiB of the address space) plus its physical address.

## The boot stub

The entry point, `_start` (the `ENTRY` of the linker script), is in `kernel/src/arch/x86_64/boot.s`, which is assembled into the kernel with `global_asm!`. It runs in 32-bit protected mode and:

1. checks the Multiboot2 magic value, and that the CPU supports long mode and the NX bit (printing an error on the screen and the serial port, and halting, if not);
2. builds temporary page tables that map physical `0..1GiB` three times, with 2MiB pages: at virtual address 0 (an identity map, for the stub itself), at `PHYS_MAP_OFFSET` (`0xFFFF800000000000`, where the kernel reaches physical memory), and at `KERNEL_BASE` (where the kernel is linked);
3. enables PAE (`CR4`), long mode and NX (the `EFER` MSR) and paging (`CR0`), which puts the CPU in long mode's 32-bit compatibility mode;
4. loads a minimal GDT with a 64-bit code segment and far-jumps to it, which switches to 64-bit mode;
5. jumps to the kernel proper in the higher half, switches to the kernel's stack, and calls `kmain` with the Multiboot2 magic value and the address of the boot information.

## `kmain`

`kmain` (`kernel/src/arch/x86_64/kmain.rs`) is the Rust entry point. It sets up what the boot stub left temporary or out:

- the serial port and the screen, for `println!`;
- the kernel's own GDT and TSS (with separate stacks for double faults and NMIs) and IDT (`gdt.rs` and `idt.rs`);
- SSE, so that user programs can use it (the kernel itself is built without floating point);
- physical memory: the frame allocator is given the available regions of the Multiboot2 memory map, except the kernel image and the boot information;
- the kernel's page tables (`paging.rs`): the direct map of all physical memory at `PHYS_MAP_OFFSET`, and the kernel image at `KERNEL_BASE`, with its code executable and everything else not. These replace the boot stub's page tables, so the identity map is gone from then on;
- the kernel heap, which gets the largest region of physical memory;
- the PIC and the timer (PIT).

It then calls `start_kernel`.

## `start_kernel`

`start_kernel` (`kernel/src/main.rs`) is the architecture-independent part of booting: it sets up the threading system and the root file system (a TempFS), starts the IDE driver's thread and a process for each init program (pipes, by default; see `KIDNEYOS_INIT` in the building guide), enables interrupts, and turns into the kernel shell (rush), showing the `kidney:/$` prompt.
