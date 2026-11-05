#!/bin/sh
set -eu

. ./config/rocky-host-image.env

destination=build/cache/rocky/$ROCKY_IMAGE_NAME
temporary=${destination}.download
mkdir -p build/cache/rocky
trap 'rm -f "$temporary"' 0 HUP INT TERM

published=$(awk '/^SHA256 / {print $NF}' config/rocky-host-image.CHECKSUM)
if [ "$published" != "$ROCKY_IMAGE_SHA256" ]; then
    printf '%s\n' 'tracked Rocky published checksum disagrees with image manifest' >&2
    exit 1
fi

if [ -f "$destination" ]; then
    if ROCKY_IMAGE_PATH=$destination ./scripts/verify-rocky-host.sh; then
        exit 0
    fi
    printf '%s\n' 'cached Rocky host image failed verification; fetching it again (the cached file is kept if that fails)' >&2
fi

rm -f "$temporary"
curl -fL --retry 3 -o "$temporary" "$ROCKY_IMAGE_URL"
if ! ROCKY_IMAGE_PATH=$temporary ./scripts/verify-rocky-host.sh >/dev/null; then
    printf '%s\n' 'downloaded Rocky host image failed verification; cache left unchanged' >&2
    exit 1
fi
mv "$temporary" "$destination"

ROCKY_IMAGE_PATH=$destination ./scripts/verify-rocky-host.sh
