#!/bin/sh
set -eu

rpm_file=$(find build/rpm/RPMS -type f -name 'ginux-*.x86_64.rpm' -print | sed -n '1p')
test -n "$rpm_file" || { printf '%s\n' 'Ginux RPM is missing' >&2; exit 1; }

rm -f build/rocky-rpm-test-overlay.qcow2
ROCKY_NETWORK_RESTRICT=off \
    ROCKY_SSH_PORT=22427 \
    ROCKY_PID_FILE=build/rocky-rpm-test.pid \
    ROCKY_SERIAL_LOG=build/rocky-rpm-test-serial.log \
    ROCKY_OVERLAY_DISK=build/rocky-rpm-test-overlay.qcow2 \
    ROCKY_VCPUS=4 ROCKY_MEMORY_MIB=4096 \
    ./scripts/with-rocky-host.sh ./scripts/test-rpm-rocky.sh "$rpm_file"
