#!/bin/sh
set -eu

. ./scripts/rocky-domain-common.sh

printf '%s\n' 'Gate 2 automated checks:'
./scripts/prepare-rocky-domain.sh

for artifact in \
    "$ROCKY_DOMAIN_ROOTFS_PATH" \
    "$ROCKY_DOMAIN_MANIFEST_PATH" \
    "$ROCKY_DOMAIN_METADATA_PATH"; do
    test -s "$artifact"
done
source_sha=$(shasum -a 256 "$ROCKY_SOURCE_IMAGE" | awk '{print $1}')
test "$source_sha" = "$ROCKY_IMAGE_SHA256"
printf '  [1/8] PASS  Pinned source: size=%s sha256=%s\n' \
    "$(wc -c < "$ROCKY_SOURCE_IMAGE" | tr -d ' ')" "$source_sha"
printf '  [2/8] PASS  Prepared artifacts: rootfs=%s bytes manifest=%s bytes\n' \
    "$(wc -c < "$ROCKY_DOMAIN_ROOTFS_PATH" | tr -d ' ')" \
    "$(wc -c < "$ROCKY_DOMAIN_MANIFEST_PATH" | tr -d ' ')"

archive_list=$ROCKY_DOMAIN_WORK_DIR/archive.list
mkdir -p "$ROCKY_DOMAIN_WORK_DIR"
tar -tf "$ROCKY_DOMAIN_ROOTFS_PATH" > "$archive_list"
for required in \
    ./etc/os-release \
    ./etc/machine-id \
    ./usr/lib/systemd/systemd \
    ./usr/lib64/ld-linux-x86-64.so.2; do
    grep -Fqx "$required" "$archive_list"
done

os_release=$ROCKY_DOMAIN_WORK_DIR/os-release
tar -xOf "$ROCKY_DOMAIN_ROOTFS_PATH" ./usr/lib/os-release > "$os_release"
grep -Eq '^NAME="?Rocky Linux"?$' "$os_release"
version=$(sed -n 's/^VERSION_ID="\{0,1\}\([^" ]*\)"\{0,1\}$/\1/p' "$os_release")
test "$version" = 10.2
printf '  [3/8] PASS  Userspace identity: Rocky Linux %s\n' "$version"

systemd_description=$(tar -xOf "$ROCKY_DOMAIN_ROOTFS_PATH" ./usr/lib/systemd/systemd |
    file -)
printf '%s\n' "$systemd_description" |
    grep -Fq 'ELF 64-bit LSB pie executable, x86-64'
printf '%s\n' '  [4/8] PASS  Runtime inputs: machine-id, x86-64 systemd, dynamic loader'

manifest_sha=$(shasum -a 256 "$ROCKY_DOMAIN_MANIFEST_PATH" | awk '{print $1}')
grep -Fqx "SOURCE_SHA256=$source_sha" "$ROCKY_DOMAIN_METADATA_PATH"
grep -Fqx "MANIFEST_SHA256=$manifest_sha" "$ROCKY_DOMAIN_METADATA_PATH"
printf '  [5/8] PASS  Provenance metadata: source and manifest hashes match\n'

first_manifest=$ROCKY_DOMAIN_WORK_DIR/first.manifest
cp "$ROCKY_DOMAIN_MANIFEST_PATH" "$first_manifest"
first_manifest_sha=$manifest_sha
GINUS_IMAGE_FORCE=1 ./scripts/prepare-rocky-domain.sh
cmp "$first_manifest" "$ROCKY_DOMAIN_MANIFEST_PATH"
second_manifest_sha=$(shasum -a 256 "$ROCKY_DOMAIN_MANIFEST_PATH" | awk '{print $1}')
printf '  [6/8] PASS  Repeatability: first=%s second=%s\n' \
    "$first_manifest_sha" "$second_manifest_sha"

./scripts/clean-build.sh >/dev/null
test -s "$ROCKY_DOMAIN_ROOTFS_PATH"
test -s "$ROCKY_DOMAIN_MANIFEST_PATH"
test "$second_manifest_sha" = "$(shasum -a 256 "$ROCKY_DOMAIN_MANIFEST_PATH" | awk '{print $1}')"
if git ls-files build | grep -q .; then
    printf '%s\n' 'generated build artifacts are unexpectedly tracked by Git' >&2
    exit 1
fi
printf '%s\n' '  [7/8] PASS  Cache policy: normal cleanup retained artifacts; Git tracks none'

mkdir -p "$ROCKY_DOMAIN_WORK_DIR"
corrupt_source=$ROCKY_DOMAIN_WORK_DIR/corrupt.qcow2
dd if="$ROCKY_SOURCE_IMAGE" of="$corrupt_source" bs=4096 count=1 2>/dev/null
if ROCKY_IMAGE_PATH=$corrupt_source ./scripts/verify-rocky-host.sh >/dev/null 2>&1; then
    printf '%s\n' 'corrupt Rocky source was incorrectly accepted' >&2
    exit 1
fi
if ROCKY_IMAGE_PATH=$ROCKY_DOMAIN_WORK_DIR/missing.qcow2 \
    ./scripts/verify-rocky-host.sh >/dev/null 2>&1; then
    printf '%s\n' 'missing Rocky source was incorrectly accepted' >&2
    exit 1
fi
printf '%s\n' '  [8/8] PASS  Failure paths: corrupt source rejected; missing source rejected'

rm -f "$archive_list" "$os_release" "$first_manifest" "$corrupt_source"
printf '%s\n' 'test-image: PASS (8/8 checks)'
