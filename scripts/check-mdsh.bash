#!/usr/bin/env bash

set -euo pipefail

# Check that every markdown file's mdsh-generated sections are up to date.
# mdsh only accepts a single input file per invocation, and resolves includes
# relative to the current directory, so run it from each file's directory.
# Only tracked files: a plain `find .` would also walk build outputs and other
# checkouts, like the Git worktrees under .claude/worktrees.
cd "$(git rev-parse --show-toplevel)"
status=0
while IFS= read -r -d '' file; do
  # Only print mdsh's output if it fails, since it's noisy otherwise.
  if ! output=$(cd "$(dirname "$file")" && mdsh --frozen -i "$(basename "$file")" 2>&1); then
    echo "$output" >&2
    echo "mdsh: $file is out of date; run 'mdsh -i $file' to update it." >&2
    status=1
  fi
done < <(git ls-files -z -- '*.md')

exit "$status"
