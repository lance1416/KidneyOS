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

The builder image, `ghcr.io/lance1416/kidneyos-builder`, is built for both `amd64` and `arm64`, so on Apple Silicon Macs and ARM Linux machines Docker runs the `arm64` variant natively, without emulation. (The older `ghcr.io/kidneyos/kidneyos-builder` image is `amd64`-only and its toolchain is too old to build the current code, so don't use it.)

> ⚠️ The image is published to the GitHub container registry by CI (see [Maintainers](#maintainers)). If `docker pull ghcr.io/lance1416/kidneyos-builder` fails with `denied` or `manifest unknown`, it hasn't been published yet (or isn't public yet). In that case build it locally first with `scripts/build-container.bash` (add `--arm` on Apple Silicon or other ARM hosts); this only needs Docker and takes a while the first time.

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

This will pull the image from the GitHub container registry (unless you've built it locally) and start a shell in it with the repository mounted at `/KidneyOS`, where you can run `make`. You will need to run the container with the script above each time you want to build KidneyOS.

Docker uses the image matching your machine's architecture. To force a specific one, set `KIDNEYOS_PLATFORM`, e.g. `KIDNEYOS_PLATFORM=linux/amd64 scripts/run-container.bash` (or `$env:KIDNEYOS_PLATFORM = "linux/amd64"` in PowerShell).

## Install Host Tools

Since we can't run graphical tools inside the Docker container, we'll have to install the following two packages on the host (on macOS, run `brew install qemu bochs`):

- [`qemu`](https://repology.org/project/qemu/versions)
- [`bochs`](https://repology.org/project/bochs/versions) (Optional, but recommended as it is useful for debugging. See the corresponding ["Useful Tools" section](../useful-tools.md#bochs) for more information.)

## Maintainers

The image is built from the `kidneyos-builder` package in `nix/flake.nix`, which wraps the same `build` devshell that CI uses. Its name is defined once, as `builderImage` in `nix/flake.nix`; the scripts read it from there.

The [`container.yaml`](https://github.com/lance1416/KidneyOS/blob/main/.github/workflows/container.yaml) workflow publishes it: whenever `nix/**` or `rust-toolchain.toml` changes on `main` (or when run by hand from the Actions tab), it builds the image natively on `amd64` and `arm64` runners, pushes the `latest-amd64` and `latest-arm64` tags, and points the multi-architecture `latest` tag at them. After the first publish, make the package public under the package's settings on GitHub so that `docker pull` works without logging in.

To build and push by hand instead:

```sh
scripts/build-container.bash        # amd64
scripts/build-container.bash --arm  # arm64
scripts/push-container.bash
scripts/push-container.bash --arm
```

`build-container.bash` uses the host's Nix if it is installed, and otherwise builds inside a `nixos/nix` container, so it also works on macOS with only Docker installed. On an Apple Silicon Mac both architectures can be built this way (the `amd64` image is built under emulation, which is much slower). `push-container.bash` pushes the per-architecture `latest-amd64`/`latest-arm64` tag and then points the multi-architecture `latest` tag at every per-architecture tag that has been pushed. You need to be logged in to `ghcr.io` (`docker login ghcr.io`) with a token that has the `write:packages` scope.
