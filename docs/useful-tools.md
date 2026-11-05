# Useful Tools

This section contains info about a few tools that might be helpful as you work on KidneyOS.

## GDB

GDB is a debugger which supports a variety of programming languages. You can debug an instance of KidneyOS running inside QEMU by first starting the virtual machine with:

```sh
make run-qemu-gdb
```

The QEMU process will wait for GDB to attach before starting the OS.

The root of the KidneyOS repository contains a `.gdbinit` file, which makes a variety of useful configuration tweaks. Before GDB will use this file though, you'll likely need to add the following to your user-level gdbinit file (which is located at `~/.config/gdb/gdbinit` on Linux and MacOS):

```gdbinit
set auto-load safe-path KidneyOS # Replace "KidneyOS" with the path to your repository.
```

Then, run the following from the root of the KidneyOS repository. (If you built KidneyOS with Docker and are running QEMU on your host, and `rust-gdb` is not installed on your host system, you can also use plain `gdb`, though it won't have the Rust-specific pretty printers added by `rust-gdb`, which is a wrapper script for `gdb`.)

```sh
rust-gdb
```

This will start GDB. After you hit the enter key once, GDB should attach to QEMU and resume execution of the OS until it reaches the kernel's `kmain` function (in `kernel/src/arch/x86_64/kmain.rs`). From this point onwards, you should be able to use GDB as usual. Use hardware breakpoints (`hbreak`) for anything before that, since GRUB only loads the kernel into memory after GDB has connected. The `.gdbinit` file also explains how to step through the 32-bit boot stub (`_start`), which runs before the CPU is in 64-bit mode.

## Bochs

[Bochs](https://bochs.sourceforge.io) is a x86 emulator. (Note that it is a GUI program, meaning if you are building KidneyOS using Docker, you'll have to install it separately on your host system.) It has some, though not all, of the debugging features that GDB does, but it also has additional features which are more specific to OS development that GDB doesn't have. Some examples include:

- It has an indicator which displays the current CPU mode.
- You can dump sections of memory based on either physical or linear addresses.
- It has windows for viewing the contents of the global and interrupt descriptor tables, and the memory page tables.

KidneyOS is a 64-bit kernel, so Bochs must be built with x86-64 support (`./configure --enable-x86-64`); `bochsrc.txt` selects a CPU model with the features KidneyOS needs. You can run KidneyOS with Bochs by running:

```sh
make run-bochs
```

The `bochs_break!` macro (which is defined in `shared/src/macros.rs`) may be of use when running KidneyOS with Bochs.

## Code Coverage

If you're writing tests and want to see which lines are being executed by the tests, the Makefile provides targets that help with this. To produce an HTML coverage report, run the following:

```sh
make test report-coverage
```

This will run the tests and produce a report at `build/coverage/index.html`.
