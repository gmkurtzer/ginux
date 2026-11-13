#!/bin/sh
set -eu

raw_image=$1
rootfs_output=$2
manifest_output=$3
mountpoint=/mnt/ginux-rocky-source
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
mount_options=$(findmnt -no OPTIONS --target "$mountpoint")
case ",$mount_options," in
    *,ro,*) ;;
    *) printf '%s\n' 'Rocky source root was not mounted read-only' >&2; exit 1 ;;
esac
if awk -F: 'NR > 2 { name=$1; gsub(/^[ \t]+|[ \t]+$/, "", name); if (name ~ /^(eth|en)/) found=1 } END { exit !found }' \
    /proc/net/dev; then
    printf '%s\n' 'extraction helper unexpectedly has an Ethernet interface' >&2
    exit 1
fi
if awk 'NR > 1 && $2 == "00000000" { found=1 } END { exit !found }' /proc/net/route; then
    printf '%s\n' 'extraction helper unexpectedly has a default IPv4 route' >&2
    exit 1
fi
printf '%s\n' 'EXTRACT: source-mount=read-only helper-network=no-ethernet,no-default-route'

(
    printf '%s\n' 'GINUX_ROOTFS_MANIFEST_VERSION=1'
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
