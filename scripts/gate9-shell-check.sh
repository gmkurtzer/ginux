#!/bin/sh
set -eu

printf '%s\n' 'GINUX_AUTHENTICATED'
id
cat "/proc/$$/stat"

test ! -e /var/lib/ginux
test ! -e /proc/self/fd/240
printf '%s\n' 'GINUX_LOCKDOWN_HOST_PATHS: BLOCKED'

test ! -e /dev/kvm
test ! -e /dev/sda
printf '%s\n' 'GINUX_LOCKDOWN_DEVICES: BLOCKED'

mkdir -p /tmp/ginux-lockdown-mount
if mount -t tmpfs tmpfs /tmp/ginux-lockdown-mount 2>/dev/null; then
    umount /tmp/ginux-lockdown-mount || true
    exit 90
fi
rmdir /tmp/ginux-lockdown-mount
printf '%s\n' 'GINUX_LOCKDOWN_MOUNT: BLOCKED'

if unshare -m true 2>/dev/null; then
    exit 91
fi
printf '%s\n' 'GINUX_LOCKDOWN_NAMESPACE: BLOCKED'
printf '%s\n' 'GINUX_SHELL_CHECK_DONE'
