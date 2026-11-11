#!/bin/sh
set -eu

. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
index=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_INDEX
before_rootfs=$(shasum -a 256 "$rootfs" | awk '{print $1}')
before_index=$(shasum -a 256 "$index" | awk '{print $1}')

printf '%s\n' 'Gate 5 demonstration: OCI-backed VFS and balanced native userspace.'
printf '%s\n' 'Expected: the cached Rocky image remains immutable; Ginux runs the image'
printf '%s\n' '          loader and Bash; sensitive operations fail before host effect;'
printf '%s\n' '          supervisor loss and unexpected stops terminate the tracee.'
printf '\n'

./scripts/verify-rocky-oci.sh >/dev/null
printf '  [1/8] PASS  pinned OCI objects verified offline: %s\n' "$GINUX_OCI_MANIFEST_DIGEST"

./scripts/test-oci-layers.sh >/dev/null
printf '%s\n' '  [2/8] PASS  ordered layers: replacement, whiteout, opaque directory, repeatability'

./scripts/test-vfs.sh >/dev/null
printf '%s\n' '  [3/8] PASS  Ginux VFS: Rocky identity, ELF lookup, private overlay, teardown'

./scripts/test-native-vfs.sh
printf '%s\n' '  [4/8] PASS  host loaded unmodified Rocky loader and Bash under Ginux supervision'
printf '%s\n' '  [5/8] PASS  balanced policy: read-only image/console allowed; mutation/network/ioctl denied'
printf '%s\n' '  [6/8] PASS  virtual identity: PID 4242, parent 1, instance UID/GID 0'
printf '%s\n' '  [7/8] PASS  fail-closed tracing: supervisor loss and unexpected stop killed tracees'

./scripts/verify-rocky-oci.sh >/dev/null
after_rootfs=$(shasum -a 256 "$rootfs" | awk '{print $1}')
after_index=$(shasum -a 256 "$index" | awk '{print $1}')
test "$before_rootfs" = "$after_rootfs"
test "$before_index" = "$after_index"
printf '  [8/8] PASS  cache unchanged and instance artifacts cleaned: rootfs=%s\n' "$after_rootfs"
printf '%s\n' 'Not claimed: full Linux VFS, devices, networking, /proc, /sys, or systemd boot.'
printf '%s\n' 'GATE5: PASS (8/8 demonstration checks)'
