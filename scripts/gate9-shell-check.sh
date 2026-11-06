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

set -- /proc/[0-9]*
printf 'GINUX_MEASURE: idle-managed-proc-entries=%s\n' "$#"
if test -e /run/ginux-skip-network; then
    printf '%s\n' 'GINUX_NETWORK_DNS_HTTPS: SKIPPED'
    printf '%s\n' 'GINUX_NETWORK_BIND_ACCEPT_CONFLICT: SKIPPED'
else
    if test -e /run/ginux-skip-external-network; then
        printf '%s\n' 'GINUX_NETWORK_DNS_HTTPS: SKIPPED'
    else
        getent ahostsv4 dl.rockylinux.org >/tmp/ginux-rocky-addresses
        test -s /tmp/ginux-rocky-addresses
        curl --fail --silent --show-error --head --max-time 30 \
            https://dl.rockylinux.org/pub/rocky/ >/tmp/ginux-rocky-headers
        test -s /tmp/ginux-rocky-headers
        printf '%s\n' 'GINUX_NETWORK_DNS_HTTPS: PASS'
    fi

    /usr/local/sbin/ginux-gate9-network-check
fi
if test -e /run/ginux-fail-dnf; then
    printf '%s\n' 'GINUX_DNF_UPDATE: FAILURE-BEGIN'
    rpm -qa | LC_ALL=C sort >/tmp/ginux-rpmdb-before-failure
    if dnf -y --disablerepo='*' \
        --repofrompath=ginux-failure,http://127.0.0.1:1/ginux-missing \
        --setopt=timeout=1 --setopt=retries=0 makecache
    then
        exit 92
    fi
    rpm -qa | LC_ALL=C sort >/tmp/ginux-rpmdb-after-failure
    test -s /tmp/ginux-rpmdb-after-failure
    cmp -s /tmp/ginux-rpmdb-before-failure /tmp/ginux-rpmdb-after-failure
    printf '%s\n' 'GINUX_DNF_UPDATE: FAILURE-PASS'
elif test -e /run/ginux-skip-dnf; then
    printf '%s\n' 'GINUX_DNF_UPDATE: SKIPPED'
else
    printf '%s\n' 'GINUX_DNF_UPDATE: BEGIN'
    dnf -v repolist --enabled
    dnf -v -y update
    printf '%s\n' 'GINUX_DNF_UPDATE: PASS'
fi
printf '%s\n' 'GINUX_SHELL_CHECK_DONE'
