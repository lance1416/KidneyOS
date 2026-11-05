#!/usr/bin/env bash

set -euo pipefail

usage() {
  echo "Usage: $0 [-p | --podman]" >&2
  exit 2
}

case "$#" in
  0)
    extra_flags=
    ;;
  1)
    case "$1" in
      -p | --podman)
        extra_flags=--userns=keep-id
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

# shellcheck source=scripts/container-image.bash
source "$(dirname "$(realpath "${BASH_SOURCE[0]}")")/container-image.bash"

# Docker picks the image matching the host's architecture (arm64 on Apple
# Silicon). Set KIDNEYOS_PLATFORM (e.g. linux/amd64) to force a specific one.
platform_flags=()
if [ -n "${KIDNEYOS_PLATFORM:-}" ]; then
  platform_flags=(--platform "$KIDNEYOS_PLATFORM")
fi

# The ${a[@]+...} form avoids an "unbound variable" error for an empty array
# under `set -u` in bash 3.2, which is what macOS ships as /bin/bash.
# shellcheck disable=SC2086 # extra_flags is intentionally word-split.
docker run $extra_flags ${platform_flags[@]+"${platform_flags[@]}"} --rm -it \
  -v "$project_root:/KidneyOS" \
  -w /KidneyOS \
  "$image:latest"
