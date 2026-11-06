#!/bin/sh
# Probe one Ginux daemon binary. The optional runner (qemu-aarch64) executes
# a foreign-architecture static binary. Exit status is the probe's status.
set -eu

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
    echo "usage: arch-smoke.sh <ginux> <x86_64|aarch64> [runner]" >&2
    exit 2
fi

bin=$1
arch=$2
runner=${3:-}

run() {
    if [ -n "$runner" ]; then
        "$runner" "$bin" "$@"
    else
        "$bin" "$@"
    fi
}

output=$(run arch)
printf '%s\n' "$output"
printf '%s\n' "$output" | grep -q "GINUX_ARCH: ${arch}"
printf '%s\n' "$output" | grep -q "GINUX_ARCH: fork=ok"

help_err=$(mktemp)
trap 'rm -f "$help_err"' EXIT
set +e
run help >/dev/null 2>"$help_err"
status=$?
set -e
if [ "$status" -ne 2 ]; then
    echo "ginux help exited ${status}, expected 2" >&2
    exit 1
fi
grep -q "ginux: usage:" "$help_err"
