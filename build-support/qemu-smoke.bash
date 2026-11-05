#!/usr/bin/env bash

# Boots KidneyOS in QEMU without a display, with the serial port written to a
# log file, and succeeds once EXPECTED (the shell prompt, or for x86_64 so far
# its greeting) shows up in it.
#
# Usage: build-support/qemu-smoke.bash QEMU TIMEOUT_SECONDS LOG_FILE EXPECTED QEMU_ARGS...
# Run through `make smoke-test`, which supplies the arguments.

set -euo pipefail

if [[ $# -lt 4 ]]; then
  echo "Usage: $0 QEMU TIMEOUT_SECONDS LOG_FILE EXPECTED QEMU_ARGS..." >&2
  exit 2
fi

qemu=$1
timeout_seconds=$2
log=$3
expected=$4
shift 4

mkdir -p "$(dirname "$log")"
: >"$log"

"$qemu" -display none -serial "file:$log" "$@" &
qemu_pid=$!
trap 'kill "$qemu_pid" 2>/dev/null || true' EXIT

for ((i = 0; i < timeout_seconds; i++)); do
  # A panic (whose message contains "panicked at") or a failure in the
  # x86_64 boot stub won't get any further, so don't wait for the timeout.
  if grep -qE 'panicked at|boot error' "$log"; then
    echo "KidneyOS panicked or failed to boot." >&2
    break
  fi
  if grep -qF "$expected" "$log"; then
    echo "Smoke test passed: reached '$expected' after ~${i}s."
    exit 0
  fi
  if ! kill -0 "$qemu_pid" 2>/dev/null; then
    echo "QEMU exited before '$expected' appeared." >&2
    break
  fi
  sleep 1
done

echo "----- serial output ($log) -----" >&2
cat "$log" >&2
echo "--------------------------------" >&2
echo "Smoke test failed: no '$expected' within ${timeout_seconds}s." >&2
exit 1
