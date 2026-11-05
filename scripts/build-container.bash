#!/usr/bin/env bash

set -euo pipefail

usage() {
  echo "Usage: $0 [-a | --arm]" >&2
  exit 2
}

case "$#" in
  0)
    system=x86_64-linux
    arch=amd64
    ;;
  1)
    case "$1" in
      -a | --arm)
        system=aarch64-linux
        arch=arm64
        ;;
      *)
        usage
        ;;
    esac
    ;;
  *)
    usage
    ;;
esac

image=ghcr.io/kidneyos/kidneyos-builder
project_root=$(dirname "$(dirname "$(realpath "${BASH_SOURCE[0]}")")")
cd "$project_root"

attr="./nix#packages.$system.kidneyos-builder"
out=build/kidneyos-builder-$arch.tar.gz
mkdir -p build

if command -v nix >/dev/null; then
  # Build the image with the host's Nix. This requires a host capable of
  # building derivations for the selected system (natively, or via a remote
  # builder or binfmt emulation).
  nix --extra-experimental-features 'nix-command flakes' \
    build --out-link "$out" "$attr"
else
  # No Nix on the host (e.g. macOS): build inside a Linux container instead.
  # Docker Desktop on Apple Silicon can run amd64 containers via emulation, so
  # both architectures can be built there (amd64 is much slower). The Nix store
  # is kept in a named volume so rebuilds are incremental.
  echo "nix not found; building inside a nixos/nix container" >&2
  docker run --rm --platform "linux/$arch" \
    -v "kidneyos-nix-store-$arch:/nix" \
    -v "$project_root:/KidneyOS" \
    -w /KidneyOS \
    nixos/nix:latest \
    sh -euc '
      git config --global --add safe.directory /KidneyOS
      nix --extra-experimental-features "nix-command flakes" \
        --option filter-syscalls false \
        build --no-link --print-out-paths "$1" > /tmp/out
      cp "$(cat /tmp/out)" "$2"
    ' sh "$attr" "$out"
fi

# Import the result of the build into the Docker daemon and give it an
# architecture-specific tag, used by scripts/push-container.bash.
docker load -i "$out"
docker tag "$image:latest" "$image:latest-$arch"
