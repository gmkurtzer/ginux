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

# Plain mode must reject an archive cut inside an entry's data, inside a
# header, or at an entry boundary without the end-of-archive blocks.
plain=$work/plain
mkdir -p "$plain/usr/bin" "$plain/usr/lib64"
cp "$base/usr/bin/bash" "$plain/usr/bin/bash"
cp "$base/usr/lib64/ld-linux-x86-64.so.2" "$base/usr/lib64/libc.so.6" "$plain/usr/lib64/"
{ printf '%s\n' truncation-marker; seq 1 20000; } > "$plain/zz-large"
(cd "$plain" && find . | sort) > "$work/plain.list"
tar --format=ustar --no-recursion -cf "$work/plain.tar" -C "$plain" -T "$work/plain.list"
marker=$(grep -abo truncation-marker "$work/plain.tar" | cut -d: -f1)
large=$(wc -c < "$work/plain/zz-large")
for cut in "$((marker + 4096))" "$((marker - 512 + 100))" \
    "$((marker + (large + 511) / 512 * 512))"; do
    head -c "$cut" "$work/plain.tar" > "$work/truncated.tar"
    if cargo run -q -p ginux-vfs-index -- "$work/truncated.tar" "$work/truncated.index" \
        >/dev/null 2>"$work/truncated.err"; then
        printf '%s\n' "plain mode accepted an archive truncated at byte $cut" >&2
        exit 1
    fi
    if ! grep -q truncated "$work/truncated.err"; then
        printf '%s\n' "plain mode failed without reporting truncation at byte $cut:" >&2
        cat "$work/truncated.err" >&2
        exit 1
    fi
done

# Plain mode keeps the last entry for a path, as extraction does: the
# appended bash is the only ELF copy (the index check reads its data), and
# a file replacing a directory drops the directory's children.
dup=$work/dup
mkdir -p "$dup/first/usr/bin" "$dup/first/usr/lib64" "$dup/first/etc/swap" "$dup/last/etc"
printf '%s\n' not-elf > "$dup/first/usr/bin/bash"
cp "$base/usr/lib64/ld-linux-x86-64.so.2" "$base/usr/lib64/libc.so.6" "$dup/first/usr/lib64/"
printf '%s\n' child > "$dup/first/etc/swap/child"
cp "$base/usr/bin/bash" "$dup/last/bash"
printf '%s\n' file > "$dup/last/etc/swap"
tar --format=ustar -cf "$work/dup.tar" -C "$dup/first" .
cp "$dup/last/bash" "$dup/first/usr/bin/bash"
tar --format=ustar -rf "$work/dup.tar" -C "$dup/first" ./usr/bin/bash
tar --format=ustar -rf "$work/dup.tar" -C "$dup/last" ./etc/swap
cargo run -q -p ginux-vfs-index -- "$work/dup.tar" "$work/dup.index" >/dev/null
if grep -Fq /etc/swap/child "$work/dup.index"; then
    printf '%s\n' 'plain mode kept children of a replaced directory' >&2
    exit 1
fi

# GNU long-name records (paths and link names over 100 bytes) must keep
# the full names in the assembled tar and in both indexes.
# Print the type of each entry in tar $1 that has a GNU long link name (K).
long_link_entry_types() {
    grep -Fabo '././@LongLink' "$1" | cut -d: -f1 | while read -r at; do
        [ $((at % 512)) -eq 0 ] || continue
        [ "$(dd if="$1" bs=1 skip=$((at + 156)) count=1 2>/dev/null)" = K ] || continue
        while :; do
            type=$(dd if="$1" bs=1 skip=$((at + 156)) count=1 2>/dev/null)
            case $type in L | K) ;; *) break ;; esac
            size=$(dd if="$1" bs=1 skip=$((at + 124)) count=11 2>/dev/null)
            at=$((at + 512 + (0$size + 511) / 512 * 512))
        done
        printf '%s\n' "$type"
    done
}
if tar --version 2>/dev/null | grep -q 'GNU tar'; then
    gnu=$work/gnu
    gnu_upper=$work/gnu-upper
    mkdir -p "$gnu/usr/bin" "$gnu/usr/lib64" "$gnu/opt/$long" "$gnu_upper/opt/$long"
    cp "$base/usr/bin/bash" "$gnu/usr/bin/bash"
    cp "$base/usr/lib64/ld-linux-x86-64.so.2" "$base/usr/lib64/libc.so.6" "$gnu/usr/lib64/"
    printf '%s\n' gnu-long > "$gnu/opt/$long/$long-file"
    # Relative: the index holds link names up to 128 bytes.
    ln -s "$long-file" "$gnu/opt/$long/$long-symlink"
    ln "$gnu/opt/$long/$long-file" "$gnu/opt/$long/$long-hardlink"
    (cd "$gnu" && find . ! -name "$long-hardlink" | sort &&
        printf '%s\n' "./opt/$long/$long-hardlink") > "$work/gnu.list"
    tar --format=gnu --no-recursion -cf "$work/gnu.tar" -C "$gnu" -T "$work/gnu.list"
    # The upper layer replaces the hard link's target; the link keeps the
    # lower data.
    printf '%s\n' gnu-upper > "$gnu_upper/opt/$long/$long-file"
    tar --format=gnu -cf "$work/gnu-upper.tar" -C "$gnu_upper" .
    cargo run -q -p ginux-vfs-index -- assemble \
        "$work/rootfs-gnu.tar" "$work/rootfs-gnu.index" \
        "$work/gnu.tar" "$work/gnu-upper.tar" >/dev/null
    # The hard link's long target name must not survive as a K record:
    # libarchive extracts a file preceded by one as a hard link. Only the
    # symlink keeps its K record.
    test "$(long_link_entry_types "$work/gnu.tar" | sort | tr -d '\n')" = 12
    if [ "$(long_link_entry_types "$work/rootfs-gnu.tar" | tr -d '\n')" != 2 ]; then
        printf '%s\n' 'assembled rootfs gives a non-symlink a GNU long link name' >&2
        exit 1
    fi
    rm -rf "$work/extract-gnu" && mkdir "$work/extract-gnu"
    tar -xf "$work/rootfs-gnu.tar" -C "$work/extract-gnu"
    test "$(cat "$work/extract-gnu/opt/$long/$long-file")" = gnu-upper
    test "$(readlink "$work/extract-gnu/opt/$long/$long-symlink")" = "$long-file"
    test "$(cat "$work/extract-gnu/opt/$long/$long-hardlink")" = gnu-long
    test ! -L "$work/extract-gnu/opt/$long/$long-hardlink"
    cargo run -q -p ginux-vfs-index -- "$work/gnu.tar" "$work/gnu.index" >/dev/null
    for index in "$work/rootfs-gnu.index" "$work/gnu.index"; do
        grep -Fq "/opt/$long/$long-file" "$index"
        grep -Fq "/opt/$long/$long-symlink" "$index"
        grep -Fq "/opt/$long/$long-hardlink" "$index"
    done
else
    printf '%s\n' 'test-oci-layers: skipping GNU long-name case (needs GNU tar)'
fi

# GNU sparse files (tar --sparse) store only their data regions, and PAX
# formats 0.1 and 1.0 put them under a GNUSparseFile.<pid> path. Both modes
# must reject them instead of indexing a wrong name, size and content. A
# --sparse archive without sparse members stays accepted.
sparse_cases=
if tar --version 2>/dev/null | grep -q 'GNU tar'; then
    sparse=$work/sparse
    mkdir -p "$sparse/usr/bin" "$sparse/usr/lib64" "$sparse/opt"
    cp "$base/usr/bin/bash" "$sparse/usr/bin/bash"
    cp "$base/usr/lib64/ld-linux-x86-64.so.2" "$base/usr/lib64/libc.so.6" "$sparse/usr/lib64/"
    tar --sparse --format=posix -cf "$work/dense.tar" -C "$sparse" .
    cargo run -q -p ginux-vfs-index -- "$work/dense.tar" "$work/dense.index" >/dev/null
    # Six data regions: a GNU-format S header holds four, the rest go to an
    # extension block.
    for region in 0 1 2 3 4 5; do
        printf 'region-%s\n' "$region" |
            dd of="$sparse/opt/holes" bs=1 seek=$((region * 65536)) conv=notrunc 2>/dev/null
    done
    tar --sparse --format=posix -cf "$work/sparse-1.0.tar" -C "$sparse" .
    tar --sparse --format=posix --sparse-version=0.0 -cf "$work/sparse-0.0.tar" -C "$sparse" .
    tar --sparse --format=posix --sparse-version=0.1 -cf "$work/sparse-0.1.tar" -C "$sparse" .
    tar --sparse --format=gnu -cf "$work/sparse-gnu.tar" -C "$sparse" .
    if grep -aq 'GNU\.sparse\.' "$work/sparse-1.0.tar"; then
        for archive in sparse-1.0 sparse-0.1 sparse-0.0 sparse-gnu; do
            for mode in plain assemble; do
                if [ "$mode" = plain ]; then
                    set -- "$work/$archive.tar" "$work/$archive.index"
                else
                    set -- assemble "$work/$archive-rootfs.tar" "$work/$archive.index" "$work/$archive.tar"
                fi
                if cargo run -q -p ginux-vfs-index -- "$@" >/dev/null 2>"$work/sparse.err"; then
                    printf '%s\n' "$mode mode accepted GNU sparse file in $archive.tar" >&2
                    exit 1
                fi
                if ! grep -Fq 'GNU sparse files are unsupported' "$work/sparse.err" ||
                    ! grep -Fq /opt/holes "$work/sparse.err"; then
                    printf '%s\n' "$mode mode failed on $archive.tar without naming the sparse file:" >&2
                    cat "$work/sparse.err" >&2
                    exit 1
                fi
            done
        done
        sparse_cases=', GNU sparse files'
    else
        printf '%s\n' 'test-oci-layers: skipping GNU sparse case (file system made no holes)'
    fi
else
    printf '%s\n' 'test-oci-layers: skipping GNU sparse case (needs GNU tar)'
fi
printf '%s\n' "test-oci-layers: PASS (replacement, whiteout, opaque directory, hard links, repeatability, truncation, duplicates, GNU long names$sparse_cases)"
