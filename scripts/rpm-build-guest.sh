#!/bin/sh
set -eu

sudo dnf -y install rpm-build rust cargo clang systemd-rpm-macros rust-std-static
rpmbuild --define '_topdir /var/tmp/ginux-rpmbuild' \
    --ba /var/tmp/ginux-rpmbuild/SPECS/ginux.spec
