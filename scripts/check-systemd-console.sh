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

allowlisted_bpf='bpf-restrict-fs: BPF LSM hook not enabled in the kernel, BPF LSM not supported.'
allowlisted_netlink='Failed to open netlink, ignoring: Address family not supported by protocol'
allowlisted_monitor='Failed to allocate device monitor: Address family not supported by protocol'
allowlisted_counts=$(awk \
    -v bpf="$allowlisted_bpf" \
    -v netlink="$allowlisted_netlink" \
    -v monitor="$allowlisted_monitor" '
    { sub(/\r$/, "") }
    $0 == bpf { bpf_count++ }
    $0 == netlink { netlink_count++ }
    $0 == monitor { monitor_count++ }
    END { print bpf_count + 0, netlink_count + 0, monitor_count + 0 }
' "$log")
set -- $allowlisted_counts
test "$1" -le 1 && test "$2" -le 1 && test "$3" -le 1 || {
    printf '%s\n' 'systemd console repeated an allowlisted host-feature diagnostic' >&2
    exit 1
}

unexpected=$(awk \
    -v bpf="$allowlisted_bpf" \
    -v netlink="$allowlisted_netlink" \
    -v monitor="$allowlisted_monitor" \
    -v mode="$mode" '
    { sub(/\r$/, "") }
    /^GINUX_INSTANCE_START:/ { console = 1; next }
    /GINUX_CONFIG_EFFECTIVE_END/ { console = 1; next }
    !console { next }
    /^GINUX_NATIVE_AUDIT:/ { next }
    /^emulate surface=unsupported-unix-address length=[0-9]+ result=-1$/ { next }
    $0 == bpf || $0 == netlink || $0 == monitor { next }
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

printf 'systemd console audit: PASS (unexpected diagnostics=0, allowlisted host features=%s, mode=%s)\n' \
    "$allowlisted_counts" "${mode:-normal}"
