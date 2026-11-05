# Building

In this section, we'll walk you through building a KidneyOS ISO from source on your computer.

## Creating a Build Environment

First, we need to prepare an environment with the necessary dependencies. There are multiple approaches you can take. (Your instructor may have provided guidance on which of these options to choose.) **After choosing _one_ of these methods and completing the steps, return to this page and proceed to the next section.**

- You can use a Nix devshell. This is the recommended approach. This method can only be used directly on `x86_64-linux` and `aarch64-linux` hosts (but you can still use it indirectly on other host systems via [WSL](https://learn.microsoft.com/en-us/windows/wsl) or a virtual machine). Once you've installed the [Nix package manager](https://nixos.org), we'll use it to create the build environment with a single command. Click [here](./building/nix.md) for instructions.
- You can run the build tools inside a Docker container. This method is likely the simplest if you're familiar with Docker, but it comes with some overhead on MacOS and Windows, and can be somewhat awkward as graphical development tools can't be run inside Docker, so you'll have to install them manually on your host system. Click [here](./building/docker.md) for instructions.
- You can install all the dependencies manually by using your package manager and building things from source when necessary. This method is not recommended as it is the most difficult and may result in build failures if you install different versions of tools than those used by KidneyOS maintainers. Click [here](./building/from-source.md) for instructions.

## Building KidneyOS

Once you have a build environment ready, simply run:

```sh
make
```

...from within your build environment, at the root of the KidneyOS repository. This will compile the various pieces of KidneyOS, and produce an ISO at `build/i686/kidneyos.iso`.

## Running KidneyOS

To run the ISO you've just built, run:

```sh
make run-qemu
```

There are two things to note here:

1. If you're building KidneyOS with Docker, this command should be run on the host system, not in the container build environment.
2. If you're building KidneyOS with any of the other methods, you don't have to run `make` first after modifying the source code. `make run-qemu` will also automatically rebuild the ISO if any of the KidneyOS source files have changed.

## Building for x86_64

KidneyOS is being ported from i686 to x86_64. i686 is still the default; to build and run the x86_64 kernel instead, add `ARCH=x86_64` to any of the commands above:

```sh
make ARCH=x86_64 run-qemu-ng
```

Its outputs go to `build/x86_64/` (the i686 ones to `build/i686/`). So far the x86_64 kernel only boots into long mode, prints a greeting and the amount of available memory on the screen and the serial port, and halts. `make ARCH=x86_64 smoke-test` checks that it gets that far.
