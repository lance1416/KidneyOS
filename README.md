# KidneyOS

[![CI](https://github.com/lance1416/KidneyOS/actions/workflows/ci.yaml/badge.svg)](https://github.com/lance1416/KidneyOS/actions/workflows/ci.yaml)

<!-- BEGIN mdsh -->
KidneyOS is a toy OS written in Rust, designed for teaching operating systems concepts. It is loosely based on [PintOS](https://pintos-os.org/) (the name "KidneyOS" refers to kidney beans, and is a joke about how PintOS can be read as "pintos", another kind of bean). It is still a work in progress, and is not yet ready for use. If you're interested in using KidneyOS for teaching when it's ready, you can "watch" the repository for releases on [GitHub](https://github.com/lance1416/KidneyOS).

This repository is a fork of the original [KidneyOS](https://github.com/KidneyOS/KidneyOS) project, which was developed by students at the University of Toronto under the supervision of [Professor Jack Sun](https://www.cs.toronto.edu/~sunk/). The upstream project has not been updated since January 2025; this fork continues its development independently. Many thanks to the original authors and to Professor Sun for creating KidneyOS. This fork is not maintained by, or affiliated with, the University of Toronto.
<!-- END mdsh -->

## Quickstart

Set up a build environment first (see the [building guide](https://lance1416.github.io/KidneyOS/building.html); the Nix devshell is recommended), then, from the root of the repository:

```sh
make help          # list all available targets
make               # build the ISO at build/i686/kidneyos.iso
make run-qemu-ng   # run it in QEMU without a graphical window
make check         # run the same checks as CI
```

The kernel is being ported to x86_64: `make ARCH=x86_64 run-qemu-ng` builds and boots that port (it only prints a greeting so far).

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to contribute.

## Documentation

Detailed documentation for KidneyOS (including building instructions and assignment descriptions) is available [here](https://lance1416.github.io/KidneyOS/).
