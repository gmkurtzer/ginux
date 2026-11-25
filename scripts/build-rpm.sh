#!/bin/sh
set -eu

root=$(git rev-parse --show-toplevel)
topdir=$root/build/rpm
test -r "$topdir/SPECS/ginux.spec" || {
    printf '%s\n' 'run make rpm-source first' >&2
    exit 1
}
if test "$(uname -s)" = Linux && test -r /etc/rocky-release; then
    command -v rpmbuild >/dev/null 2>&1 || {
        printf '%s\n' 'rpmbuild is required' >&2
        exit 69
    }
    rpmbuild --define "_topdir $topdir" --ba "$topdir/SPECS/ginux.spec"
else
    rpm_ssh_port=${GINUX_RPM_SSH_PORT:-22426}
    ROCKY_NETWORK_RESTRICT=off \
        ROCKY_SSH_PORT="$rpm_ssh_port" \
        ROCKY_PID_FILE=build/rocky-rpm-build.pid \
        ROCKY_SERIAL_LOG=build/rocky-rpm-build-serial.log \
        ROCKY_OVERLAY_DISK=build/rocky-rpm-build-overlay.qcow2 \
        ROCKY_VCPUS=4 ROCKY_MEMORY_MIB=4096 \
        ./scripts/with-rocky-host.sh ./scripts/build-rpm-rocky.sh
fi
