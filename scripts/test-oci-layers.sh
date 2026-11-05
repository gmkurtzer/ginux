#!/bin/sh
set -eu

work=build/oci-layer-test
base=$work/base
upper=$work/upper

rm -rf "$work"
mkdir -p "$base/usr/bin" "$base/usr/lib64" "$base/etc" \
    "$base/opt/old" "$base/opt/keep" "$upper/etc" "$upper/opt/old"
printf '\177ELFbase-bash\n' > "$base/usr/bin/bash"
printf '\177ELFbase-loader\n' > "$base/usr/lib64/ld-linux-x86-64.so.2"
printf '\177ELFbase-libc\n' > "$base/usr/lib64/libc.so.6"
printf '%s\n' base > "$base/etc/value"
printf '%s\n' removed > "$base/etc/remove"
printf '%s\n' old > "$base/opt/old/lower"
printf '%s\n' keep > "$base/opt/keep/value"
# Hard links that sort before and after their target, and one with names
# over 100 bytes (PAX path/linkpath). The upper layer replaces the targets;
# the links keep the lower layer's data, as in overlayfs.
long=$(printf 'n%.0s' $(seq 1 110))
mkdir -p "$base/etc/long"
printf '%s\n' zone > "$base/etc/zone"
ln "$base/etc/zone" "$base/etc/aaa-zone"
ln "$base/etc/zone" "$base/etc/zzz-zone"
printf '%s\n' long > "$base/etc/long/$long-target"
ln "$base/etc/long/$long-target" "$base/etc/long/$long-link"

printf '%s\n' upper > "$upper/etc/value"
: > "$upper/etc/.wh.remove"
: > "$upper/opt/old/.wh..wh..opq"
printf '%s\n' new > "$upper/opt/old/upper"
mkdir -p "$upper/etc/long"
printf '%s\n' upper-zone > "$upper/etc/zone"
printf '%s\n' upper-long > "$upper/etc/long/$long-target"

# List the base explicitly so each target precedes its links: the first name
# of a hard-linked inode becomes the regular file in the archive.
(cd "$base" && find . ! -name aaa-zone ! -name zzz-zone ! -name "$long-link" | sort &&
    printf '%s\n' ./etc/aaa-zone ./etc/zzz-zone "./etc/long/$long-link") > "$work/base.list"
tar --format=pax --no-recursion -cf "$work/base.tar" -C "$base" -T "$work/base.list"
tar --format=pax -cf "$work/upper.tar" -C "$upper" .
cargo run -q -p ginux-vfs-index -- assemble \
    "$work/rootfs-1.tar" "$work/rootfs-1.index" \
    "$work/base.tar" "$work/upper.tar" >/dev/null
cargo run -q -p ginux-vfs-index -- assemble \
    "$work/rootfs-2.tar" "$work/rootfs-2.index" \
    "$work/base.tar" "$work/upper.tar" >/dev/null

test "$(tar -xOf "$work/rootfs-1.tar" ./etc/value)" = upper
test "$(tar -xOf "$work/rootfs-1.tar" ./opt/old/upper)" = new
test "$(tar -xOf "$work/rootfs-1.tar" ./opt/keep/value)" = keep
rm -rf "$work/extract" && mkdir "$work/extract" && tar -xf "$work/rootfs-1.tar" -C "$work/extract"
test "$(cat "$work/extract/etc/zone")" = upper-zone
test "$(cat "$work/extract/etc/aaa-zone")" = zone
test "$(cat "$work/extract/etc/zzz-zone")" = zone
test "$(cat "$work/extract/etc/long/$long-target")" = upper-long
test "$(cat "$work/extract/etc/long/$long-link")" = long
if tar -tvf "$work/rootfs-1.tar" | grep -q '^h'; then
    printf '%s\n' 'assembled rootfs still contains hard-link records' >&2
    exit 1
fi
# No long symlinks in these layers, so no PAX linkpath may survive: a
# regular file carrying one is extracted as a link by libarchive.
if grep -aq ' linkpath=' "$work/rootfs-1.tar"; then
    printf '%s\n' 'assembled regular file still carries a PAX linkpath' >&2
    exit 1
fi
grep -Fq /etc/aaa-zone "$work/rootfs-1.index"
grep -Fq "/etc/long/$long-link" "$work/rootfs-1.index"
cargo run -q -p ginux-vfs-index -- "$work/base.tar" "$work/base.index" >/dev/null
grep -Fq /etc/zzz-zone "$work/base.index"
grep -Fq "/etc/long/$long-link" "$work/base.index"
if tar -tf "$work/rootfs-1.tar" | grep -Eq '(^|/)remove$|(^|/)lower$|\.wh\.'; then
    printf '%s\n' 'whiteout or opaque-directory assembly failed' >&2
    exit 1
fi
cmp "$work/rootfs-1.tar" "$work/rootfs-2.tar"
cmp "$work/rootfs-1.index" "$work/rootfs-2.index"
printf '%s\n' 'test-oci-layers: PASS (replacement, whiteout, opaque directory, hard links, repeatability)'
