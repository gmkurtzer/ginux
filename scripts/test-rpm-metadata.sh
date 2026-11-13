#!/bin/sh
set -eu

rpm_file=$(find build/rpm/RPMS -type f -name 'ginux-*.x86_64.rpm' -print | sed -n '1p')
srpm_file=$(find build/rpm/SRPMS -type f -name 'ginux-*.src.rpm' -print | sed -n '1p')
test -n "$rpm_file" && test -n "$srpm_file"
test "$(rpm -qp --qf '%{NAME}\n' "$rpm_file")" = ginux
test "$(rpm -qp --qf '%{ARCH}\n' "$rpm_file")" = x86_64
test "$(rpm -qp --qf '%{LICENSE}\n' "$rpm_file")" = Apache-2.0
files=$(rpm -qpl "$rpm_file")
printf '%s\n' "$files" | grep -Fxq /usr/bin/ginux
printf '%s\n' "$files" | grep -Fxq /usr/libexec/ginux/ginux-runtime
printf '%s\n' "$files" | grep -Fxq /usr/libexec/ginux/ginux-helper
printf '%s\n' "$files" | grep -Fxq /usr/libexec/ginux/ginux-image
printf '%s\n' "$files" | grep -Fxq /usr/lib/tmpfiles.d/ginux.conf
printf '%s\n' "$files" | grep -Fxq /etc/ginux/examples/rocky10-systemd.toml
printf '%s\n' "$files" | grep -Fxq /etc/ginux/examples/rocky10-agent-workspace.toml
printf '%s\n' "$files" | grep -Fxq /etc/ginux/examples/ubuntu24-network-service.toml
printf '%s\n' "$files" | grep -Fxq /etc/ginux/examples/alpine3.22-native.toml
if printf '%s\n' "$files" | grep -Eq '^/usr/local|ensure-oci|rootfs\.tar|rocky-systemd-oci\.env'; then
    printf '%s\n' 'RPM contains a forbidden legacy payload' >&2
    exit 1
fi
test "$(printf '%s\n' "$files" | grep -Ec '^/usr/bin/')" -eq 1
rpm -qp --requires "$rpm_file" | grep -Fxq /bin/sh
printf '%s\n' 'test-rpm-metadata: PASS'
