#!/bin/sh
set -eu

missing=0
for command in cargo rustc rustup rg qemu-system-x86_64 qemu-img ssh scp ssh-keygen hdiutil shasum docker curl jq gzip; do
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

. ./config/rocky-instance-image.env
if docker info >/dev/null 2>&1; then
    printf '%s\n' 'doctor: Docker engine            running'
    if docker image inspect "$ROCKY_INSTANCE_HELPER_IMAGE" >/dev/null 2>&1; then
        printf '%s\n' 'doctor: pinned image helper      present'
    else
        printf '%s\n' 'doctor: pinned image helper      absent (run make image-helper-fetch)'
    fi
else
    printf '%s\n' 'doctor: Docker engine            NOT RUNNING' >&2
    missing=1
fi

case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) printf '%s\n' 'doctor: reference host          macOS arm64' ;;
    *) printf 'doctor: reference host          unsupported (%s-%s)\n' "$(uname -s)" "$(uname -m)" >&2; missing=1 ;;
esac

image=build/cache/rocky/Rocky-10-GenericCloud-Base-10.2-20260525.0.x86_64.qcow2
if [ -f "$image" ]; then
    printf '%s\n' 'doctor: cached Rocky host      present'
else
    printf '%s\n' 'doctor: cached Rocky host      absent (run make host-image-fetch)'
fi

[ "$missing" -eq 0 ]
