#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
rpm_file=$1

rocky_scp "$rpm_file" "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux.rpm"
rocky_scp scripts/rpm-test-guest.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/rpm-test-guest.sh"
rocky_ssh 'chmod 0755 /var/tmp/rpm-test-guest.sh && /var/tmp/rpm-test-guest.sh'
printf '%s\n' 'Rocky Linux RPM lifecycle: PASS'
