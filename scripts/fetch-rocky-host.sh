#!/bin/sh
set -eu

. ./config/rocky-host-image.env

destination=build/cache/rocky/$ROCKY_IMAGE_NAME
temporary=${destination}.download
mkdir -p build/cache/rocky

if [ ! -f "$destination" ]; then
    curl -fL --retry 3 -o "$temporary" "$ROCKY_IMAGE_URL"
    mv "$temporary" "$destination"
fi

./scripts/verify-rocky-host.sh
