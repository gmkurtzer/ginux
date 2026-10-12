#!/usr/bin/env bash

set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

qemu="$(find_qemu)"
ovmf_code="$(find_ovmf_code)"
ovmf_vars_template="$(find_ovmf_vars)"
accelerator="${QEMU_ACCEL:-$(default_accelerator)}"
cpu_model="${QEMU_CPU:-qemu64,+smep,+smap,+umip}"
esp_dir="${GINUX_ESP_DIR:-$repo_root/build/esp}"
runtime_dir="$repo_root/build/qemu"
ovmf_vars="$runtime_dir/ovmf-vars.fd"
volume_image="${GINUX_VOLUME_IMAGE:-$runtime_dir/volume.raw}"

if [[ ! -f "$esp_dir/EFI/BOOT/BOOTX64.EFI" ]]; then
    echo "Missing $esp_dir/EFI/BOOT/BOOTX64.EFI; run 'make build' first." >&2
    exit 1
fi

mkdir -p "$runtime_dir"
cp "$ovmf_vars_template" "$ovmf_vars"
if [[ ! -f "$volume_image" ]]; then
    dd if=/dev/zero of="$volume_image" bs=1048576 count=1 2>/dev/null
fi

echo "Booting x86-64 Ginux with QEMU/$accelerator using CPU $cpu_model"

exec "$qemu" \
    -machine "q35,accel=$accelerator" \
    -cpu "$cpu_model" \
    -smp 1 \
    -m 256M \
    -display none \
    -monitor none \
    -serial stdio \
    -no-reboot \
    -net none \
    -device "intel-iommu,intremap=off" \
    -device "virtio-blk-pci,drive=ginux-volume,disable-legacy=on,serial=ginux-volume" \
    -drive "if=none,id=ginux-volume,format=raw,file=$volume_image" \
    -drive "if=pflash,format=raw,readonly=on,file=$ovmf_code" \
    -drive "if=pflash,format=raw,file=$ovmf_vars" \
    -drive "format=raw,file=fat:rw:$esp_dir"
