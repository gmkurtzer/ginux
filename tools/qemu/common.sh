#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

find_first_file() {
    local candidate
    for candidate in "$@"; do
        if [[ -f "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

find_qemu() {
    if [[ -n "${QEMU_SYSTEM_X86_64:-}" ]]; then
        printf '%s\n' "$QEMU_SYSTEM_X86_64"
    else
        command -v qemu-system-x86_64
    fi
}

find_ovmf_code() {
    if [[ -n "${OVMF_CODE:-}" ]]; then
        printf '%s\n' "$OVMF_CODE"
        return
    fi

    find_first_file \
        /opt/homebrew/opt/qemu/share/qemu/edk2-x86_64-code.fd \
        /usr/local/share/qemu/edk2-x86_64-code.fd \
        /usr/share/OVMF/OVMF_CODE.fd \
        /usr/share/edk2/x64/OVMF_CODE.fd
}

find_ovmf_vars() {
    if [[ -n "${OVMF_VARS:-}" ]]; then
        printf '%s\n' "$OVMF_VARS"
        return
    fi

    find_first_file \
        /opt/homebrew/opt/qemu/share/qemu/edk2-i386-vars.fd \
        /usr/local/share/qemu/edk2-i386-vars.fd \
        /usr/share/OVMF/OVMF_VARS.fd \
        /usr/share/edk2/x64/OVMF_VARS.fd
}

default_accelerator() {
    if [[ "$(uname -s)" == "Linux" && "$(uname -m)" == "x86_64" && -r /dev/kvm && -w /dev/kvm ]]; then
        printf 'kvm\n'
    else
        printf 'tcg\n'
    fi
}
