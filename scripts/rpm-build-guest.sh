#!/bin/sh
set -eu

sudo dnf -y install epel-release
sudo dnf -y install rpm-build rust cargo clang systemd-rpm-macros rust-std-static rpmlint
rpmbuild --define '_topdir /var/tmp/ginux-rpmbuild' \
    --ba /var/tmp/ginux-rpmbuild/SPECS/ginux.spec
rpmlint -r /var/tmp/ginux-rpmbuild/SOURCES/ginux.rpmlintrc \
    /var/tmp/ginux-rpmbuild/SRPMS/ginux-*.src.rpm \
    /var/tmp/ginux-rpmbuild/RPMS/x86_64/ginux-*.x86_64.rpm
