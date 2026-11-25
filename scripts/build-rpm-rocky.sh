#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

rocky_ssh 'rm -rf /var/tmp/ginux-rpmbuild && mkdir -p /var/tmp/ginux-rpmbuild/SOURCES /var/tmp/ginux-rpmbuild/SPECS'
rocky_scp build/rpm/SOURCES/* \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-rpmbuild/SOURCES/"
rocky_scp build/rpm/SPECS/ginux.spec \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-rpmbuild/SPECS/"
rocky_scp scripts/rpm-build-guest.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/rpm-build-guest.sh"
rocky_ssh 'chmod 0755 /var/tmp/rpm-build-guest.sh && /var/tmp/rpm-build-guest.sh'
mkdir -p build/rpm/RPMS/x86_64 build/rpm/SRPMS
rocky_scp "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-rpmbuild/RPMS/x86_64/gin*" \
    build/rpm/RPMS/x86_64/
rocky_scp "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-rpmbuild/SRPMS/gin*" \
    build/rpm/SRPMS/
printf '%s\n' 'Rocky Linux RPM build: PASS'
