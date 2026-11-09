#!/bin/sh
set -eu

legacy_term=$(printf '\144\157\155\141\151\156')
log=$(mktemp)
trap 'rm -f "$log"' 0 HUP INT TERM

./scripts/check-instance-terminology.sh

if ./scripts/ginux-host-controller.sh "$legacy_term" list >"$log" 2>&1; then
    printf '%s\n' 'legacy CLI noun was incorrectly accepted' >&2
    exit 1
fi
grep -Fq 'ginux import' "$log"
if grep -Fqi "$legacy_term" "$log"; then
    printf '%s\n' 'legacy CLI noun leaked into controller usage' >&2
    exit 1
fi

if git grep -q "/var/lib/ginux/$legacy_term" -- \
    Cargo.toml Cargo.lock Makefile README.md config crates docs scripts tests .github; then
    printf '%s\n' 'legacy persistent-state path remains active' >&2
    exit 1
fi

printf '%s\n' 'test-instance-terminology: PASS'
