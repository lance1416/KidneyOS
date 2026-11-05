# macOS (Apple Silicon)

> 💡 The easiest way to build KidneyOS on macOS is the [Docker](./docker.md) method. The builder image is built for `arm64` as well as `amd64`, so on Apple Silicon it runs natively, without emulation, once it has been published (until then, `scripts/build-container.bash --arm` builds it locally; see the [Docker](./docker.md) instructions). Follow these instructions instead if you'd rather have a native toolchain on your Mac.

These instructions assume you have [Homebrew](https://brew.sh) installed. You should have already installed the Rust toolchain with rustup as described in the [previous section](./from-source.md#install-rust-toolchain).

## Install a Cross-Compiler

KidneyOS's user programs are built as x86-64 Linux programs, so you need a [cross compiler](https://en.wikipedia.org/wiki/Cross_compiler) that produces them (the kernel itself only needs the Rust toolchain). On hosts other than x86-64 Linux, the KidneyOS build expects tools prefixed with `x86_64-unknown-linux-gnu-` (e.g. `x86_64-unknown-linux-gnu-gcc`, `x86_64-unknown-linux-gnu-as` and `x86_64-unknown-linux-gnu-ld`). We suggest the prebuilt toolchain from [messense/homebrew-macos-cross-toolchains](https://github.com/messense/homebrew-macos-cross-toolchains):

```sh
brew tap messense/macos-cross-toolchains
brew install x86_64-unknown-linux-gnu
```

Cargo also needs to know to link the Rust user programs with it, so add the following to your shell's profile (or `~/.cargo/config.toml`'s `[target.x86_64-unknown-linux-gnu]` section, as `linker = "x86_64-unknown-linux-gnu-gcc"`):

```sh
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-unknown-linux-gnu-gcc
```

## Install GRUB and Other Tools

Homebrew provides a prebuilt GRUB, so you don't need to build GRUB from source. Install it along with the other tools needed to build and run KidneyOS:

```sh
brew install i686-elf-grub xorriso mtools qemu
```

It is the `i686-elf-grub` package even though KidneyOS is a 64-bit kernel: KidneyOS boots from the BIOS, which GRUB only supports on its 32-bit `i386-pc` platform (GRUB starts the kernel in 32-bit mode, and the kernel switches to 64-bit mode itself, see [From Bootloader to `main`](../bootloader-to-main.md)). Homebrew's `x86_64-elf-grub` is for UEFI instead.

Homebrew installs GRUB's tools with an `i686-elf-` prefix, so tell the KidneyOS `Makefile` where to find `grub-mkrescue` by creating a file named `local.mk` in the root of the repository (it is ignored by Git) containing:

```make
GRUB_MKRESCUE := i686-elf-grub-mkrescue
```

[Bochs](../useful-tools.md#bochs) is optional, and can be installed with `brew install bochs` (it must support x86-64, see its section).

## Build and Run

You should now be able to run `make` to build the ISO, and `make run-qemu` to run it.

> ⚠️ `make test` doesn't work natively on macOS, since the tests are compiled for x86-64 Linux, which macOS can't run (and QEMU's Linux user-mode emulation, `qemu-x86_64`, isn't available on macOS). Use the [Docker](./docker.md) container if you need to run the tests.
