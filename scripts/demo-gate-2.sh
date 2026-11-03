#!/bin/sh
set -eu

. ./scripts/rocky-domain-common.sh

printf '%s\n' 'Gate 2 demonstration: prepare the pinned Rocky userspace offline.'
printf '%s\n' 'Expected: source verification, Rocky identity, repeatable manifest,'
printf '%s\n' '          controlled corruption rejection, and retained cache artifacts.'

./scripts/verify-rocky-host.sh
./scripts/test-rocky-domain.sh

tar -xOf "$ROCKY_DOMAIN_ROOTFS_PATH" ./usr/lib/os-release |
    sed -n '/^NAME=/p; /^VERSION=/p; /^ID=/p; /^VERSION_ID=/p'
printf 'SOURCE: %s\n' "$ROCKY_IMAGE_NAME"
printf 'ROOTFS: %s\n' "$ROCKY_DOMAIN_ROOTFS_PATH"
printf 'MANIFEST_SHA256: %s\n' "$(shasum -a 256 "$ROCKY_DOMAIN_MANIFEST_PATH" | awk '{print $1}')"
printf '%s\n' 'CORRUPT_SOURCE: rejected'
printf '%s\n' 'NETWORK: disabled for extraction helper'
printf '%s\n' 'GATE2: PASS'
