#!/bin/sh
set -eu

raw_image=$1
rootfs_output=$2
manifest_output=$3
mountpoint=/mnt/ginus-rocky-source
loop_device=

cleanup() {
    mountpoint -q "$mountpoint" && umount "$mountpoint" || true
    [ -z "$loop_device" ] || losetup -d "$loop_device" || true
    rmdir "$mountpoint" 2>/dev/null || true
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$mountpoint"
partition=$(sfdisk -d "$raw_image" |
    sed -n 's/.*4 : start= *\([0-9][0-9]*\), size= *\([0-9][0-9]*\),.*/\1 \2/p')
if [ -z "$partition" ]; then
    printf '%s\n' 'Rocky root partition 4 was not found' >&2
    exit 1
fi
set -- $partition
root_offset=$(($1 * 512))
root_size=$(($2 * 512))
loop_device=$(losetup --find --show --read-only \
    --offset "$root_offset" --sizelimit "$root_size" "$raw_image")
mount -t xfs -o ro,nouuid "$loop_device" "$mountpoint"

(
    printf '%s\n' 'GINUS_ROOTFS_MANIFEST_VERSION=1'
    cd "$mountpoint"
    find . -xdev -printf 'META\t%y\t%m\t%U\t%G\t%s\t%p\t%l\n' |
        LC_ALL=C sort
    find . -xdev -type f -print0 |
        LC_ALL=C sort -z |
        xargs -0 -r sha256sum |
        sed 's/^/SHA256\t/'
) > "$manifest_output"

tar \
    --sort=name \
    --numeric-owner \
    --xattrs \
    --acls \
    --selinux \
    --sparse \
    --format=posix \
    --mtime=@0 \
    --pax-option=delete=atime,delete=ctime \
    -cpf "$rootfs_output" \
    -C "$mountpoint" .
