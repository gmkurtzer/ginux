#!/bin/sh
set -eu

sudo dnf -y install /var/tmp/ginux.rpm
sudo rpm -V ginux
ginux --version
ginux --help >/dev/null
ginux create --help >/dev/null
ginux validate /etc/ginux/examples/alpine3.22-native.toml
ginux import rpm-alpine /etc/ginux/examples/alpine3.22-native.toml
ginux status rpm-alpine | grep -Fq 'state: stopped'
test -x /var/lib/ginux/instances/rpm-alpine/root/sbin/init
test -n "$(find /var/lib/ginux/cache/oci/blobs/sha256 -type f -print -quit)"
test -z "$(find /var/lib/ginux -name rootfs.tar -print -quit)"
sudo touch /var/lib/ginux/instances/rpm-alpine/rpm-state-marker

# Cached import must work with all guest networking disabled.
sudo unshare -n ginux import rpm-cache /etc/ginux/examples/alpine3.22-native.toml
test -x /var/lib/ginux/instances/rpm-cache/root/sbin/init

sudo dnf -y reinstall /var/tmp/ginux.rpm
sudo rpm -V ginux
test -f /var/lib/ginux/instances/rpm-alpine/rpm-state-marker
ginux list | grep -Fq rpm-alpine

sudo rpm -e ginux
test ! -e /usr/bin/ginux
test -f /var/lib/ginux/instances/rpm-alpine/rpm-state-marker
test -n "$(find /var/lib/ginux/cache/oci/blobs/sha256 -type f -print -quit)"

sudo dnf -y install /var/tmp/ginux.rpm
sudo rpm -V ginux
ginux list | grep -Fq rpm-alpine
printf '%s\n' 'rpm-test-guest: PASS'
