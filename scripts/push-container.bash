#!/usr/bin/env bash

set -euo pipefail

usage() {
  echo "Usage: $0 [-a | --arm]" >&2
  exit 2
}

case "$#" in
  0)
    arch=amd64
    ;;
  1)
    case "$1" in
      -a | --arm)
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

# You need to be logged in to the GitHub container registry and have sufficient
# permissions within the KidneyOS organization for this to work.
docker push "$image:latest-$arch"

# Point the multi-architecture `latest` tag at every per-architecture image that
# has been pushed so far, so that `docker pull` picks the native one.
sources=()
for a in amd64 arm64; do
  if docker manifest inspect "$image:latest-$a" >/dev/null 2>&1; then
    sources+=("$image:latest-$a")
  fi
done
docker buildx imagetools create --tag "$image:latest" "${sources[@]}"
