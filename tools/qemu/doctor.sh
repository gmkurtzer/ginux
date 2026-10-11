#!/usr/bin/env bash

set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

failed=0

check_command() {
    local command_name="$1"
    if command -v "$command_name" >/dev/null 2>&1; then
        printf 'ok   %-24s %s\n' "$command_name" "$(command -v "$command_name")"
    else
        printf 'MISS %-24s required\n' "$command_name"
        failed=1
    fi
}

check_command cargo
check_command rustc
check_command "${CLANG:-clang}"
check_command qemu-system-x86_64

if ovmf_code="$(find_ovmf_code 2>/dev/null)"; then
    printf 'ok   %-24s %s\n' 'OVMF code' "$ovmf_code"
else
    printf 'MISS %-24s required (set OVMF_CODE)\n' 'OVMF code'
    failed=1
fi

if ovmf_vars="$(find_ovmf_vars 2>/dev/null)"; then
    printf 'ok   %-24s %s\n' 'OVMF variables' "$ovmf_vars"
else
    printf 'MISS %-24s required (set OVMF_VARS)\n' 'OVMF variables'
    failed=1
fi

printf 'info %-24s %s/%s via %s\n' 'host' "$(uname -s)" "$(uname -m)" "${QEMU_ACCEL:-$(default_accelerator)}"

if [[ "$failed" -ne 0 ]]; then
    exit 1
fi
