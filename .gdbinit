set disassembly-flavor intel
# For a split source/assembly view, uncomment the next line (or run it in GDB).
# layout split
file build/isofiles/boot/kernel.bin
symbol-file build/kernel.sym
target remote localhost:1234
break _start
continue
