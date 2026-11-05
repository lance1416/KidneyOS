# shellcheck shell=bash
# Sourced by the other scripts/*-container.bash scripts. Sets `project_root`
# and `image` (the builder image name, defined once in nix/flake.nix).

project_root=$(dirname "$(dirname "$(realpath "${BASH_SOURCE[0]}")")")
image=$(sed -n 's/^ *builderImage = "\([^"]*\)";.*$/\1/p' "$project_root/nix/flake.nix")
if [ -z "$image" ]; then
  echo "error: couldn't find builderImage in $project_root/nix/flake.nix" >&2
  exit 1
fi
