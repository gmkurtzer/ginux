#!/bin/sh
set -eu

test "$#" -ge 1 && test "$#" -le 2 || {
    printf '%s\n' 'usage: check-systemd-console.sh LOG [dnf-failure]' >&2
    exit 64
}

log=$1
mode=${2-}
test -r "$log" || {
    printf 'systemd console log is not readable: %s\n' "$log" >&2
    exit 66
}

allowlisted='bpf-restrict-fs: BPF LSM hook not enabled in the kernel, BPF LSM not supported.'
allowlisted_count=$(awk -v allowlisted="$allowlisted" '
    { sub(/\r$/, "") }
    $0 == allowlisted { count++ }
    END { print count + 0 }
' "$log")
test "$allowlisted_count" -le 1 || {
    printf '%s\n' 'systemd console repeated the allowlisted BPF LSM diagnostic' >&2
    exit 1
}

unexpected=$(awk -v allowlisted="$allowlisted" -v mode="$mode" '
    { sub(/\r$/, "") }
    /^GINUX_INSTANCE_START:/ { console = 1; next }
    /GINUX_CONFIG_EFFECTIVE_END/ { console = 1; next }
    !console { next }
    $0 == allowlisted { next }
    mode == "failure" && /127\.0\.0\.1:1\/ginux-missing/ { next }
    mode == "failure" && /repo '\''ginux-failure'\''/ { next }
    /\[FAILED\]/ ||
    /Failed to/ ||
    /failed with result/ ||
    /Dependency failed/ ||
    /Operation not permitted/ ||
    /[Uu]nsupported/ ||
    /not supported/ ||
    /timed out/ ||
    /[Ww]arning/ { print NR ":" $0 }
' "$log")

if test -n "$unexpected"; then
    printf '%s\n' 'unexpected systemd console diagnostics:' >&2
    printf '%s\n' "$unexpected" >&2
    exit 1
fi

printf 'systemd console audit: PASS (unexpected diagnostics=0, allowlisted BPF LSM=%s, mode=%s)\n' \
    "$allowlisted_count" "${mode:-normal}"
