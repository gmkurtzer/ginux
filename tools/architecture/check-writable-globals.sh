#!/usr/bin/env bash

set -euo pipefail

root=$(git rev-parse --show-toplevel)
manifest="$root/docs/feedback-hardening/writable-globals.tsv"
inventory=$(mktemp "${TMPDIR:-/tmp}/ginux-global-inventory.XXXXXX")
actual=$(mktemp "${TMPDIR:-/tmp}/ginux-global-actual.XXXXXX")
expected=$(mktemp "${TMPDIR:-/tmp}/ginux-global-expected.XXXXXX")
negative=$(mktemp "${TMPDIR:-/tmp}/ginux-global-negative.XXXXXX")
trap 'rm -f "$inventory" "$actual" "$expected" "$negative"' EXIT

fail() {
    printf 'writable-global check: %s\n' "$*" >&2
    exit 1
}

[[ -f "$manifest" ]] || fail 'classification manifest is missing'
"$root/tools/architecture/inventory-writable-globals.sh" >"$inventory"

tail -n +2 "$inventory" | awk -F '\t' '{print $1 "\t" $2 "\t" $4}' | LC_ALL=C sort >"$actual"
tail -n +2 "$manifest" | awk -F '\t' '
    NF != 7 { bad = 1 }
    $4 !~ /^(bootstrap-init|per-cpu-arch|domain-runtime|process-runtime|shared-runtime|platform-device|service-runtime|verification-only)$/ { bad = 1 }
    $5 !~ /^(allowlist|migrate)$/ { bad = 1 }
    $6 !~ /^(none|5|6)$/ { bad = 1 }
    { print $1 "\t" $2 "\t" $3 }
    END { exit bad }
' | LC_ALL=C sort >"$expected" || fail 'manifest contains an invalid or incomplete classification row'

if [[ $(wc -l <"$expected" | tr -d ' ') != $(sort -u "$expected" | wc -l | tr -d ' ') ]]; then
    fail 'manifest contains duplicate file/symbol classifications'
fi

if ! cmp -s "$actual" "$expected"; then
    diff -u "$expected" "$actual" >&2 || true
    fail 'inventory and classification manifest differ'
fi

# Prove the comparison fails closed when a new writable global appears.
cp "$actual" "$negative"
printf 'interior-mutable\tkernel/src/synthetic.rs\tUNCLASSIFIED_GLOBAL\n' >>"$negative"
LC_ALL=C sort -o "$negative" "$negative"
if cmp -s "$negative" "$expected"; then
    fail 'negative classification test did not detect a new writable global'
fi

printf 'Writable-global classification check passed (%s globals)\n' \
    "$(wc -l <"$actual" | tr -d ' ')"
