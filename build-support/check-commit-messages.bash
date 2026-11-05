#!/usr/bin/env bash

# Checks the message of every commit in a revision range with the same script
# the commit-msg hook uses (.githooks/commit-msg).
#
# Usage: build-support/check-commit-messages.bash RANGE   (e.g. origin/main..HEAD)
# Run through `make check-commits COMMIT_RANGE=...`.

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 RANGE" >&2
  exit 2
fi
range=$1
hook="$(dirname "$0")/../.githooks/commit-msg"

# With set -e, a bad range stops the script here.
commits=$(git rev-list --reverse "$range")

message=$(mktemp)
trap 'rm -f "$message"' EXIT

checked=0
failed=0
while IFS= read -r commit; do
  [[ -n "$commit" ]] || continue
  checked=$((checked + 1))
  git log -1 --format=%B "$commit" >"$message"
  if ! errors=$("$hook" "$message" 2>&1); then
    failed=$((failed + 1))
    git log -1 --format='%h %s' "$commit"
    while IFS= read -r line; do echo "    $line"; done <<<"$errors"
  fi
done <<<"$commits"

if ((failed > 0)); then
  echo "$failed of $checked commit message(s) in $range don't follow CONTRIBUTING.md." >&2
  exit 1
fi
echo "All $checked commit message(s) in $range are OK."
