#!/bin/sh
set -eu

test_root=$(mktemp -d "${TMPDIR:-/tmp}/ginux-host-lifecycle.XXXXXX")
cleanup() {
    rm -rf "$test_root"
}
trap cleanup 0 HUP INT TERM

overlay=$test_root/host-overlay.qcow2
pid_file=$test_root/host.pid
serial_log=$test_root/host.log
: >"$overlay"
: >"$serial_log"

ROCKY_OVERLAY_DISK=$overlay \
ROCKY_PID_FILE=$pid_file \
ROCKY_SERIAL_LOG=$serial_log \
    ./scripts/reset-rocky-host.sh >/dev/null

test ! -e "$overlay"
test ! -e "$pid_file"
test ! -e "$serial_log"

if grep -Fq 'build/rocky-host-overlay.qcow2' scripts/clean-build.sh; then
    printf '%s\n' 'ordinary cleanup still removes the persistent host overlay' >&2
    exit 1
fi

grep -Eq '^run-host:[[:space:]]+rpm$' Makefile
grep -Eq '^host-run:[[:space:]]+run-host$' Makefile
grep -Fq './scripts/run-host.sh' Makefile
grep -Fq './scripts/install-ginux-host.sh' scripts/run-host.sh
grep -Fq 'sudo dnf -y install /var/tmp/ginux.rpm' scripts/install-ginux-host.sh
grep -Fq 'sudo rpm -V ginux' scripts/install-ginux-host.sh

printf '%s\n' 'HOST_LIFECYCLE: PASS (RPM-backed run-host; ordinary cleanup preserves; explicit reset removes)'
