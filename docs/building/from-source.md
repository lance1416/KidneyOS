# From Source

This section describes how to build KidneyOS by installing dependencies using a package manager and building it from source.

> ❗ This method is only supported on `x86_64-linux` and `aarch64-linux` hosts, or in WSL. For additional instructions on building for macOS (ARM), see [here](./macos.md).

> ⚠️ The instructions that follow are designed to be as platform-agnostic as possible. However, the names of packages are not necessarily the same across different package managers. When mentioning package names, we will provide links to [Repology](https://repology.org/), which you should consult to determine the specific name used by your package manager.

<!-- [> clone](clone.md) -->

<!-- BEGIN mdsh -->
## Clone Repository

Clone the repository and `cd` into the resulting directory. (Depending on how your instructor wants you to submit your work, they may have given you an alternate repository URL. If so, use that URL instead of the one below.)

```sh
git clone https://github.com/lance1416/KidneyOS
cd KidneyOS
```

<!-- TODO: Provide instructions for checking out the appropriate branch for once we have stable, tagged versions. -->
<!-- END mdsh -->

## Install Rust Toolchain

Next, we will install the specific version of the Rust toolchain used by KidneyOS. We will do this using rustup. You can install it by running the command on the [rustup website](https://rustup.rs/), or by installing the [`rustup`](https://repology.org/project/rustup/versions) package with your package manager.

The `rust-toolchain.toml` file at the root of the repository tells rustup which toolchain to use, so you don't need to select one yourself (and shouldn't run `rustup override`, which would take precedence over that file). However, recent versions of rustup no longer install toolchains automatically, so the first time you use the repository, run the following from its root:

```sh
rustup toolchain install
```

Running `cargo --version` should now print:

```
cargo 1.101.0-nightly (31d3246e0 2026-10-27)
```

## Install x86-64 Linux Build Tools

The user programs are built as x86-64 Linux programs, with `gcc`, `as` and `ld` (GNU binutils) for x86-64. On an `x86_64-linux` host, those are the native ones, which your distribution's `gcc` and `binutils` packages provide. On other hosts, you need a cross toolchain whose tools are prefixed with `x86_64-unknown-linux-gnu-` (e.g. `x86_64-unknown-linux-gnu-gcc`), and to point Cargo at it with `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-unknown-linux-gnu-gcc`; set `CROSS_PREFIX` in a `local.mk` file in the root of the repository if your toolchain's prefix is different. Some package managers may have a package for these. If yours doesn't, you may have to build them from source. <!-- TODO: Provide more detailed instructions on how to do this, or a link to somewhere which does? -->

## Install Other Tools

We'll also need to install the following packages:

- [`grub`](https://repology.org/project/grub/versions)
  - Make sure it's Grub 2.
  - KidneyOS boots from the BIOS, which GRUB supports with its 32-bit `i386-pc` platform (it then switches to 64-bit mode itself), so you need GRUB's `i386-pc` modules to build the ISO. On `x86_64-linux`, the usual GRUB package has them. If you're on `aarch64-linux`, this will be highly distro-specific. You may have to build Grub from source.
- [`qemu`](https://repology.org/project/qemu/versions)
- [`xorriso`](https://repology.org/project/xorriso/versions)
- [`bochs`](https://repology.org/project/bochs/versions)
  - Optional, but recommended as it is useful for debugging. It must be built with x86-64 support. See the corresponding ["Useful Tools" section](../useful-tools.md#bochs) for more information.
