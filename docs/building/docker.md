# Docker

This section describes how to build KidneyOS using Docker.

<!-- [> clone](clone.md) -->

<!-- BEGIN mdsh -->
## Clone Repository

Clone the repository and `cd` into the resulting directory. (Depending on how your instructor wants you to submit your work, they may have given you an alternate repository URL. If so, use that URL instead of the one below.)

```sh
git clone https://github.com/KidneyOS/KidneyOS
cd KidneyOS
```

<!-- TODO: Provide instructions for checking out the appropriate branch for once we have stable, tagged versions. -->
<!-- END mdsh -->

## Install Docker

If you already have Docker installed, you can skip this step. If you're on Linux, install the [`docker`](https://repology.org/project/docker/versions) package using your package manager (or [`podman`](https://repology.org/project/podman/versions), see below). Otherwise, install Docker Desktop, which can be downloaded [for macOS from here](https://docs.docker.com/desktop/setup/install/mac-install/) or [for Windows from here](https://docs.docker.com/desktop/setup/install/windows-install/).

The image is published for both `amd64` and `arm64`, so on Apple Silicon Macs and ARM Linux machines Docker runs it natively, without emulation.

## Run Container

Then execute the appropriate script. For Linux or MacOS, use the following:

```sh
scripts/run-container.bash
```

...or for Windows:

```powershell
scripts/run-container.ps1
```

<!-- TODO: support some way of handling different container versions for once there are multiple releases. -->

If you use Podman instead of Docker, pass `--podman` to the script, which makes files created in the container owned by your user.

This will pull the container from the GitHub container registry and start it. You will need to run the container with the script above each time you want to build KidneyOS.

## Install Host Tools

Since we can't run graphical tools inside the Docker container, we'll have to install the following two packages on the host (on macOS, run `brew install qemu bochs`):

- [`qemu`](https://repology.org/project/qemu/versions)
- [`bochs`](https://repology.org/project/bochs/versions) (Optional, but recommended as it is useful for debugging. See the corresponding ["Useful Tools" section](../useful-tools.md#bochs) for more information.)

## Maintainers

The image is built from the `kidneyos-builder` package in `nix/flake.nix`, which wraps the same `build` devshell that CI uses. After changes to the devshell are merged, rebuild and push the image for each architecture:

```sh
scripts/build-container.bash        # amd64
scripts/build-container.bash --arm  # arm64
scripts/push-container.bash
scripts/push-container.bash --arm
```

`build-container.bash` uses the host's Nix if it is installed, and otherwise builds inside a `nixos/nix` container, so it also works on macOS with only Docker installed. On an Apple Silicon Mac both architectures can be built this way (the `amd64` image is built under emulation, which is much slower). `push-container.bash` pushes the per-architecture `latest-amd64`/`latest-arm64` tags and then points the multi-architecture `latest` tag at them. You need to be logged in to `ghcr.io` with permission to push to the KidneyOS organization's packages.
