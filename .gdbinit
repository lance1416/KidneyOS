set disassembly-flavor intel
# For a split source/assembly view, uncomment the next line (or run it in GDB).
# layout split

# QEMU's gdbstub (qemu-system-x86_64) always describes the CPU as x86-64, also
# while GRUB and the boot stub run in 32-bit mode, so GDB has to use that
# architecture from the start (or it rejects QEMU's register packets).
set architecture i386:x86-64
symbol-file build/kernel.sym
target remote localhost:1234

# Hardware breakpoints, since the kernel isn't in memory yet when GDB connects:
# GRUB loads it, overwriting any software breakpoint.
#
# To debug the 32-bit boot stub (kernel/src/arch/x86_64/boot.s) instead, use
# `hbreak _start`, and `set architecture i386` to disassemble and step through
# its 32-bit code; once it has jumped to `long_mode_entry`, switch back with
# `set architecture i386:x86-64`.
hbreak kmain
continue
