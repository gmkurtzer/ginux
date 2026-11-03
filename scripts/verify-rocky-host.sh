#!/bin/sh
set -eu

. ./config/rocky-host-image.env

image=build/cache/rocky/$ROCKY_IMAGE_NAME
published=$(awk '/^SHA256 / {print $NF}' config/rocky-host-image.CHECKSUM)
if [ "$published" != "$ROCKY_IMAGE_SHA256" ]; then
    printf '%s\n' 'tracked Rocky published checksum disagrees with image manifest' >&2
    exit 1
fi
if [ ! -f "$image" ]; then
    printf '%s\n' 'missing Rocky host image; run make host-image-fetch' >&2
    exit 2
fi

size=$(wc -c < "$image" | tr -d ' ')
actual=$(shasum -a 256 "$image" | awk '{print $1}')
if [ "$size" != "$ROCKY_IMAGE_SIZE" ]; then
    printf 'Rocky host image size mismatch: expected %s, got %s\n' "$ROCKY_IMAGE_SIZE" "$size" >&2
    exit 1
fi
if [ "$actual" != "$ROCKY_IMAGE_SHA256" ]; then
    printf 'Rocky host image checksum mismatch: expected %s, got %s\n' "$ROCKY_IMAGE_SHA256" "$actual" >&2
    exit 1
fi
printf 'Rocky host image verified: %s\n' "$image"
