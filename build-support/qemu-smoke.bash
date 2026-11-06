#!/usr/bin/env bash

# Boots KidneyOS in QEMU without a display, with the serial port written to a
# log file, and succeeds once every EXPECTED string (e.g. the shell prompt)
# shows up in it. An EXPECTED string of the form "N*string" must show up in at
# least N lines. It fails as soon as the log matches the extended regular
# expression SMOKE_FATAL, by default a panic (whose message contains "panicked
# at"), a failure in the x86_64 boot stub or a user program being killed (for a
# fault), none of which get any further.
#
# Usage: build-support/qemu-smoke.bash QEMU TIMEOUT_SECONDS LOG_FILE EXPECTED... -- QEMU_ARGS...
# Run through `make smoke-test`, which supplies the arguments.

set -euo pipefail

usage() {
  echo "Usage: $0 QEMU TIMEOUT_SECONDS LOG_FILE EXPECTED... -- QEMU_ARGS..." >&2
  exit 2
}

[[ $# -ge 5 ]] || usage
qemu=$1
timeout_seconds=$2
log=$3
shift 3
expected=()
while [[ $# -gt 0 && $1 != -- ]]; do
  expected+=("$1")
  shift
done
[[ $# -gt 0 && ${#expected[@]} -gt 0 ]] || usage
shift # --
fatal=${SMOKE_FATAL:-panicked at|boot error|Process [0-9]+ killed}

mkdir -p "$(dirname "$log")"
: >"$log"

"$qemu" -display none -serial "file:$log" "$@" &
qemu_pid=$!
trap 'kill "$qemu_pid" 2>/dev/null || true' EXIT

# Prints the expected strings that aren't in the log yet; succeeds if any.
missing() {
  local found=1 string count
  for string in "${expected[@]}"; do
    count=1
    if [[ $string =~ ^([0-9]+)\*(.*)$ ]]; then
      count=${BASH_REMATCH[1]}
      string=${BASH_REMATCH[2]}
    fi
    if (($(grep -cF -- "$string" "$log") < count)); then
      echo "'$string' (x$count)"
      found=0
    fi
  done
  ((found == 0))
}

for ((i = 0; i < timeout_seconds; i++)); do
  if grep -qE -- "$fatal" "$log"; then
    echo "The log matches '$fatal':" >&2
    grep -aE -- "$fatal" "$log" >&2
    break
  fi
  if ! missing >/dev/null; then
    echo "Smoke test passed: saw all ${#expected[@]} expected strings after ~${i}s."
    exit 0
  fi
  if ! kill -0 "$qemu_pid" 2>/dev/null; then
    echo "QEMU exited early." >&2
    break
  fi
  sleep 1
done

echo "----- serial output ($log) -----" >&2
cat "$log" >&2
echo "--------------------------------" >&2
echo "Smoke test failed within ${timeout_seconds}s; missing: $(missing | tr '\n' ' ')" >&2
exit 1
