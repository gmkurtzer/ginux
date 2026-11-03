#!/bin/sh
set -eu

kernel=build/cache/vmlinuz-virt-3.24.3
initramfs=build/ginus-initramfs.cpio.gz

if [ ! -f "$kernel" ]; then
    printf '%s\n' 'missing cached kernel; run make fetch-test-assets once' >&2
    exit 2
fi
if [ ! -f "$initramfs" ]; then
    printf '%s\n' 'missing initramfs; run make build' >&2
    exit 2
fi

exec perl -e 'alarm shift; exec @ARGV' 45 \
    qemu-system-x86_64 \
    -machine pc,accel=tcg \
    -cpu max \
    -m 256M \
    -nodefaults \
    -display none \
    -serial stdio \
    -no-reboot \
    -kernel "$kernel" \
    -initrd "$initramfs" \
    -append 'console=ttyS0 rdinit=/sbin/ginus panic=-1 quiet'
