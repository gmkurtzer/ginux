#!/bin/sh
set -eu

missing=0
for command in cargo rustc rustup qemu-system-x86_64 cpio gzip shasum perl; do
    if command -v "$command" >/dev/null 2>&1; then
        printf 'doctor: %-24s ok\n' "$command"
    else
        printf 'doctor: %-24s MISSING\n' "$command" >&2
        missing=1
    fi
done

if rustup target list --installed | grep -qx 'x86_64-unknown-linux-gnu'; then
    printf '%s\n' 'doctor: Rust x86_64 target       ok'
else
    printf '%s\n' 'doctor: Rust x86_64 target       MISSING' >&2
    missing=1
fi

case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) printf '%s\n' 'doctor: reference host          macOS arm64' ;;
    *) printf 'doctor: reference host          unsupported (%s-%s)\n' "$(uname -s)" "$(uname -m)" >&2; missing=1 ;;
esac

kernel=build/cache/vmlinuz-virt-3.24.3
if [ -f "$kernel" ]; then
    printf '%s\n' 'doctor: cached test kernel     ok'
else
    printf '%s\n' 'doctor: cached test kernel     absent (run make fetch-test-assets)'
fi

[ "$missing" -eq 0 ]
