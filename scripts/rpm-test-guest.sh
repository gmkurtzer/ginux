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
sudo test -x /var/lib/ginux/instances/rpm-alpine/root/bin/busybox
test -n "$(sudo find /var/lib/ginux/cache/oci/blobs/sha256 -type f -print -quit)"
test -z "$(sudo find /var/lib/ginux -name rootfs.tar -print -quit)"
ginux start rpm-alpine
for _ in $(seq 1 20); do
    ginux status rpm-alpine | grep -Fq 'state: running' && break
    sleep 1
done
ginux status rpm-alpine | grep -Fq 'state: running'
ginux stop rpm-alpine
ginux status rpm-alpine | grep -Fq 'state: stopped'
sudo touch /var/lib/ginux/instances/rpm-alpine/rpm-state-marker

# Exercise the packaged managed-systemd path, not only the small Alpine
# lifecycle. This catches packaging/runtime integration failures in netlink,
# namespace fallback, and the installed private runtime path.
ginux import rpm-rocky /etc/ginux/examples/rocky10-systemd.toml
ginux start rpm-rocky
rpm_rocky_ready=false
for _ in $(seq 1 120); do
    if sudo grep -Fq 'ginux login:' \
        /var/lib/ginux/instances/rpm-rocky/console.log; then
        rpm_rocky_ready=true
        break
    fi
    ginux status rpm-rocky | grep -Fq 'state: running' || {
        sudo tail -n 80 /var/lib/ginux/instances/rpm-rocky/console.log >&2 || true
        exit 1
    }
    sleep 1
done
test "$rpm_rocky_ready" = true
ginux stop rpm-rocky
ginux status rpm-rocky | grep -Fq 'state: stopped'

# Cached import must work with all guest networking disabled.
sudo unshare -n ginux import rpm-cache /etc/ginux/examples/alpine3.22-native.toml
sudo test -x /var/lib/ginux/instances/rpm-cache/root/bin/busybox
sudo unshare -n ginux start rpm-cache
for _ in $(seq 1 20); do
    ginux status rpm-cache | grep -Fq 'state: running' && break
    sleep 1
done
ginux status rpm-cache | grep -Fq 'state: running'
ginux stop rpm-cache

sudo dnf -y reinstall /var/tmp/ginux.rpm
sudo rpm -V ginux
sudo test -f /var/lib/ginux/instances/rpm-alpine/rpm-state-marker
ginux list | grep -Fq rpm-alpine
ginux list | grep -Fq rpm-rocky

sudo rpm -e ginux
test ! -e /usr/bin/ginux
sudo test -f /var/lib/ginux/instances/rpm-alpine/rpm-state-marker
test -n "$(sudo find /var/lib/ginux/cache/oci/blobs/sha256 -type f -print -quit)"

sudo dnf -y install /var/tmp/ginux.rpm
sudo rpm -V ginux
ginux list | grep -Fq rpm-alpine
ginux list | grep -Fq rpm-rocky
printf '%s\n' 'rpm-test-guest: PASS'
