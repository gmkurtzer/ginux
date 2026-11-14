#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

rpm_file=${GINUX_RPM_FILE:-}
if test -z "$rpm_file"; then
    if test -d build/rpm/RPMS; then
        rpm_file=$(find build/rpm/RPMS -type f -name 'ginux-*.x86_64.rpm' -print | LC_ALL=C sort | tail -n 1)
    fi
fi
test -n "$rpm_file" && test -r "$rpm_file" || {
    printf '%s\n' 'Ginux RPM is missing; run make rpm first' >&2
    exit 66
}

printf '%s\n' 'Ginux host install: checking Rocky host'
./scripts/check-rocky-host.sh
printf '%s\n' "Ginux host install: installing $rpm_file"
rocky_scp "$rpm_file" "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux.rpm"
rocky_ssh 'sudo dnf -y install /var/tmp/ginux.rpm >/dev/null; rm -f /var/tmp/ginux.rpm; command -v ginux >/dev/null; sudo rpm -V ginux'

printf '%s\n' 'GINUX_HOST_INSTALL: PASS (installed and verified RPM; host state preserved)'
