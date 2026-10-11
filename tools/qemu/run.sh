#!/usr/bin/env bash

set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

qemu="$(find_qemu)"
ovmf_code="$(find_ovmf_code)"
ovmf_vars_template="$(find_ovmf_vars)"
accelerator="${QEMU_ACCEL:-$(default_accelerator)}"
esp_dir="${GINUX_ESP_DIR:-$repo_root/build/esp}"
runtime_dir="$repo_root/build/qemu"
ovmf_vars="$runtime_dir/ovmf-vars.fd"

if [[ ! -f "$esp_dir/EFI/BOOT/BOOTX64.EFI" ]]; then
    echo "Missing $esp_dir/EFI/BOOT/BOOTX64.EFI; run 'make build' first." >&2
    exit 1
fi

mkdir -p "$runtime_dir"
cp "$ovmf_vars_template" "$ovmf_vars"

echo "Booting x86-64 Ginux with QEMU/$accelerator"

exec "$qemu" \
    -machine "q35,accel=$accelerator" \
    -cpu qemu64 \
    -smp 1 \
    -m 256M \
    -display none \
    -monitor none \
    -serial stdio \
    -no-reboot \
    -net none \
    -drive "if=pflash,format=raw,readonly=on,file=$ovmf_code" \
    -drive "if=pflash,format=raw,file=$ovmf_vars" \
    -drive "format=raw,file=fat:rw:$esp_dir"
