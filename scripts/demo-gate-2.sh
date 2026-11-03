#!/bin/sh
set -eu

. ./scripts/rocky-domain-common.sh

printf '%s\n' 'Gate 2 scope: turn the pinned Rocky QCOW2 into a reproducible userspace archive.'
printf '%s\n' 'This gate tests image preparation only; it does not run Rocky under Ginus.'
printf '%s\n' 'The extraction helper must have no external network and mount the source read-only.'
printf '%s\n' ''

./scripts/test-rocky-domain.sh

printf '%s\n' ''
printf '%s\n' 'Prepared userspace identity:'
tar -xOf "$ROCKY_DOMAIN_ROOTFS_PATH" ./usr/lib/os-release |
    sed -n '/^NAME=/p; /^VERSION=/p; /^ID=/p; /^VERSION_ID=/p'
printf '%s\n' ''
printf '%s\n' 'Gate 2 verified:'
printf '%s\n' '  - official pinned QCOW2 accepted; corrupt and missing sources rejected'
printf '%s\n' '  - source XFS mounted read-only by a network-isolated extraction helper'
printf '%s\n' '  - Rocky identity, machine-id, x86-64 systemd, and loader are present'
printf '%s\n' '  - two independent preparations produced the same content manifest'
printf '%s\n' '  - normal cleanup retained cache artifacts and Git tracks none of them'
printf '  - rootfs: %s\n' "$ROCKY_DOMAIN_ROOTFS_PATH"
printf '  - manifest sha256: %s\n' "$(shasum -a 256 "$ROCKY_DOMAIN_MANIFEST_PATH" | awk '{print $1}')"
printf '%s\n' ''
printf '%s\n' 'Not tested in Gate 2: Ginus execution, Ginus VFS behavior, or systemd boot under Ginus.'
printf '%s\n' 'GATE2: PASS (image preparation pipeline only)'
