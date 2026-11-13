#!/bin/sh
set -eu

root=$(git rev-parse --show-toplevel)
cd "$root"
version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' crates/ginux-cli/Cargo.toml)
test -n "$version" || { printf '%s\n' 'unable to determine Ginux version' >&2; exit 1; }
test "$(git status --porcelain --untracked-files=no)" = '' || {
    printf '%s\n' 'commit tracked changes before creating RPM sources' >&2
    exit 1
}

output=$root/build/rpm
sources=$output/SOURCES
specs=$output/SPECS
work=$output/work
rm -rf "$output"
install -d -m 755 "$sources" "$specs" "$work"

git archive --format=tar --prefix="ginux-$version/" HEAD | gzip -n > \
    "$sources/ginux-$version.tar.gz"
cargo vendor --locked "$work/vendor" >/dev/null
tar -C "$work" -cf - vendor | gzip -n > \
    "$sources/ginux-$version-vendor.tar.gz"
sed "s/@VERSION@/$version/g" packaging/rpm/ginux.spec.in > \
    "$specs/ginux.spec"
install -m 644 packaging/rpm/ginux.tmpfiles "$sources/ginux.tmpfiles"
(
    cd "$sources"
    sha256sum * >SHA256SUMS
)
rm -rf "$work"
printf 'RPM sources ready: %s\n' "$output"
