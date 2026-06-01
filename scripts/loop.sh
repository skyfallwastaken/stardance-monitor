#!/bin/bash
set -euo pipefail

PROG=/app/stardance_monitor

if [ ! -x "$PROG" ]; then
  echo "Executable $PROG not found or not executable" >&2
  exit 1
fi

trap 'echo "Exiting"; exit 0' INT TERM

while true; do
  echo "Running $PROG at $(date --iso-8601=seconds)"
  if ! $PROG; then
    echo "$PROG exited with non-zero status at $(date --iso-8601=seconds)" >&2
  fi

  sleep 60 &
  wait
done
