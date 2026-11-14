#!/bin/sh
set -eu

cd "$(dirname "$0")/.."

temporary=${TMPDIR:-/tmp}/ginux-security-inventory.$$
cleanup() {
    rm -f "$temporary" "$temporary.allowed"
}
trap cleanup 0 HUP INT TERM

rg -l '\bunsafe[[:space:]]*(\{|fn|impl|trait|extern)' crates --glob '*.rs' \
    | LC_ALL=C sort >"$temporary"
cat >"$temporary.allowed" <<'EOF'
crates/baseline-runner/src/main.rs
crates/ginux-allocator/src/lib.rs
crates/ginux-init/src/launch.rs
crates/ginux-init/src/main.rs
crates/ginux-init/src/native_clone.rs
crates/ginux-init/src/native_network.rs
crates/ginux-init/src/supervisor.rs
crates/process-native-fixture/src/main.rs
EOF

if ! cmp -s "$temporary.allowed" "$temporary"; then
    printf '%s\n' 'production unsafe-code inventory changed:' >&2
    diff -u "$temporary.allowed" "$temporary" >&2 || true
    exit 1
fi

test "$(rg -c 'PR_SET_NO_NEW_PRIVS' crates/ginux-init/src/main.rs)" -ge 4
rg -Fq 'PR_SET_NO_NEW_PRIVS' crates/ginux-init/src/launch.rs
rg -Fq '%tmpfiles_create ginux.conf' packaging/rpm/ginux.spec.in
rg -Fq '%config(noreplace)' packaging/rpm/ginux.spec.in
rg -Fq 'Package removal deletes package-owned programs' docs/packaging.md
rg -Fq 'build weaponized payloads' docs/security-assurance.md
rg -Fq 'not VM-equivalent' docs/security-assurance.md
rg -Fq 'No unresolved high- or critical-impact finding remains' \
    docs/security-review.md
rg -Fq 'not an independent audit' docs/security-review.md
rg -Fq 'does not claim its proof' docs/allocator-evaluation.md

printf '%s\n' 'SECURITY_INVENTORY: PASS'
