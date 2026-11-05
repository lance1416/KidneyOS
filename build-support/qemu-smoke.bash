#!/usr/bin/env bash

# Boots KidneyOS in QEMU without a display, with the serial port written to a
# log file, and succeeds once the shell prompt shows up in it.
#
# Usage: build-support/qemu-smoke.bash TIMEOUT_SECONDS LOG_FILE QEMU_ARGS...
# Run through `make smoke-test`, which supplies the arguments.

set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 TIMEOUT_SECONDS LOG_FILE QEMU_ARGS..." >&2
  exit 2
fi

timeout_seconds=$1
log=$2
shift 2
prompt='kidney:/$'

mkdir -p "$(dirname "$log")"
: >"$log"

qemu-system-i386 -display none -serial "file:$log" "$@" &
qemu_pid=$!
trap 'kill "$qemu_pid" 2>/dev/null || true' EXIT

for ((i = 0; i < timeout_seconds; i++)); do
  if grep -qF "$prompt" "$log"; then
    echo "Smoke test passed: reached '$prompt' after ~${i}s."
    exit 0
  fi
  if ! kill -0 "$qemu_pid" 2>/dev/null; then
    echo "QEMU exited before the prompt appeared." >&2
    break
  fi
  sleep 1
done

echo "----- serial output ($log) -----" >&2
cat "$log" >&2
echo "--------------------------------" >&2
echo "Smoke test failed: no '$prompt' within ${timeout_seconds}s." >&2
exit 1
