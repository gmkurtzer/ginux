#!/bin/sh
set -eu

legacy_term=$(printf '\144\157\155\141\151\156')

active_paths=$(git ls-files -- \
    Cargo.toml Cargo.lock Makefile README.md \
    config crates docs scripts tests .github |
    grep -v '^scripts/check-instance-terminology\.sh$' || true)

path_matches=$(printf '%s\n' "$active_paths" | grep -i "$legacy_term" || true)
content_matches=$(git grep -n -i "$legacy_term" -- \
    Cargo.toml Cargo.lock Makefile README.md \
    config crates docs scripts tests .github \
    ':(exclude)scripts/check-instance-terminology.sh' || true)

if test -n "$path_matches" || test -n "$content_matches"; then
    printf '%s\n' 'legacy isolation-unit terminology remains in active product surfaces:' >&2
    test -z "$path_matches" || printf '%s\n' "$path_matches" >&2
    test -z "$content_matches" || printf '%s\n' "$content_matches" >&2
    exit 1
fi

printf '%s\n' 'check-instance-terminology: PASS'
