#!/bin/sh
set -eu

root=build/initramfs-root
output=build/ginus-initramfs.cpio.gz

case "$root" in
    build/initramfs-root) rm -rf "$root" ;;
    *) printf '%s\n' 'refusing unsafe initramfs cleanup' >&2; exit 1 ;;
esac

mkdir -p "$root/sbin" "$root/dev" "$root/proc" "$root/sys" "$root/tmp"
cp target/x86_64-unknown-linux-gnu/release/ginus "$root/sbin/ginus"
cp target/x86_64-unknown-linux-gnu/release/managed-payload "$root/managed-payload"
chmod 0755 "$root/sbin/ginus" "$root/managed-payload"

(cd "$root" && find . -print | LC_ALL=C sort | cpio -o -H newc 2>/dev/null) | gzip -n -9 > "$output"
printf 'built %s\n' "$output"
