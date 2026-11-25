#!/bin/sh
set -eu

# Automated checks never share the operator's persistent interactive host.
export ROCKY_SSH_PORT="${ROCKY_SSH_PORT:-22408}"
export ROCKY_PID_FILE="${ROCKY_PID_FILE:-build/rocky-test-suite-host.pid}"
export ROCKY_SERIAL_LOG="${ROCKY_SERIAL_LOG:-build/rocky-test-suite-host-serial.log}"
export ROCKY_OVERLAY_DISK="${ROCKY_OVERLAY_DISK:-build/rocky-test-suite-host-overlay.qcow2}"

cleanup() {
    ./scripts/stop-rocky-host.sh
}
trap cleanup 0 HUP INT TERM

./scripts/start-rocky-host.sh
"$@"
