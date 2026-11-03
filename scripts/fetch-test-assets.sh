#!/bin/sh
set -eu

url=https://dl-cdn.alpinelinux.org/alpine/v3.24/releases/x86_64/netboot-3.24.3/vmlinuz-virt
expected=17fbd3efd16f2e7cb1b945f06423b37399ba7d5ac8cdd3665e7f13f363500e85
destination=build/cache/vmlinuz-virt-3.24.3
temporary=${destination}.download

mkdir -p build/cache
if [ ! -f "$destination" ]; then
    curl -fL --retry 3 -o "$temporary" "$url"
    mv "$temporary" "$destination"
fi

actual=$(shasum -a 256 "$destination" | awk '{print $1}')
if [ "$actual" != "$expected" ]; then
    printf 'test kernel checksum mismatch: expected %s, got %s\n' "$expected" "$actual" >&2
    exit 1
fi
printf 'test kernel verified: %s\n' "$destination"
