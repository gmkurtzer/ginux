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

printf '%s\n' upper > "$upper/etc/value"
: > "$upper/etc/.wh.remove"
: > "$upper/opt/old/.wh..wh..opq"
printf '%s\n' new > "$upper/opt/old/upper"

tar -cf "$work/base.tar" -C "$base" .
tar -cf "$work/upper.tar" -C "$upper" .
cargo run -q -p ginus-vfs-index -- assemble \
    "$work/rootfs-1.tar" "$work/rootfs-1.index" \
    "$work/base.tar" "$work/upper.tar" >/dev/null
cargo run -q -p ginus-vfs-index -- assemble \
    "$work/rootfs-2.tar" "$work/rootfs-2.index" \
    "$work/base.tar" "$work/upper.tar" >/dev/null

test "$(tar -xOf "$work/rootfs-1.tar" ./etc/value)" = upper
test "$(tar -xOf "$work/rootfs-1.tar" ./opt/old/upper)" = new
test "$(tar -xOf "$work/rootfs-1.tar" ./opt/keep/value)" = keep
if tar -tf "$work/rootfs-1.tar" | grep -Eq '(^|/)remove$|(^|/)lower$|\.wh\.'; then
    printf '%s\n' 'whiteout or opaque-directory assembly failed' >&2
    exit 1
fi
cmp "$work/rootfs-1.tar" "$work/rootfs-2.tar"
cmp "$work/rootfs-1.index" "$work/rootfs-2.index"
printf '%s\n' 'test-oci-layers: PASS (replacement, whiteout, opaque directory, repeatability)'
