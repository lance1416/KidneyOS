# macOS (Apple Silicon)

> 💡 The easiest way to build KidneyOS on macOS is the [Docker](./docker.md) method. The builder image is built for `arm64` as well as `amd64`, so on Apple Silicon it runs natively, without emulation, once it has been published (until then, `scripts/build-container.bash --arm` builds it locally; see the [Docker](./docker.md) instructions). Follow these instructions instead if you'd rather have a native toolchain on your Mac.

These instructions assume you have [Homebrew](https://brew.sh) installed. You should have already installed the Rust toolchain with rustup as described in the [previous section](./from-source.md#install-rust-toolchain).

## Install a Cross-Compiler

Since KidneyOS is built for 32-bit x86, you need a [cross compiler](https://en.wikipedia.org/wiki/Cross_compiler) that produces binaries for that architecture. The KidneyOS build expects tools prefixed with `i686-unknown-linux-gnu-` (e.g. `i686-unknown-linux-gnu-gcc`, `i686-unknown-linux-gnu-ld` and `i686-unknown-linux-gnu-objcopy`). We suggest the prebuilt toolchain from [messense/homebrew-macos-cross-toolchains](https://github.com/messense/homebrew-macos-cross-toolchains):

```sh
brew tap messense/macos-cross-toolchains
brew install i686-unknown-linux-gnu
```

## Install GRUB and Other Tools

Homebrew provides a prebuilt GRUB for i686, so you no longer need to build GRUB from source. Install it along with the other tools needed to build and run KidneyOS:

```sh
brew install i686-elf-grub xorriso mtools qemu
```

Homebrew installs GRUB's tools with an `i686-elf-` prefix, so tell the KidneyOS `Makefile` where to find `grub-mkrescue` by creating a file named `local.mk` in the root of the repository (it is ignored by Git) containing:

```make
GRUB_MKRESCUE := i686-elf-grub-mkrescue
```

[Bochs](../useful-tools.md#bochs) is optional, and can be installed with `brew install bochs`.

## Build and Run

You should now be able to run `make` to build the ISO, and `make run-qemu` to run it.

> ⚠️ `make test` doesn't work natively on macOS, since the tests are compiled for i686 Linux and run with QEMU's Linux user-mode emulation (`qemu-i386`), which isn't available on macOS. Use the [Docker](./docker.md) container if you need to run the tests.
