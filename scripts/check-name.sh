#!/bin/sh
set -eu

old_title=$(printf '%s%s' 'Gi' 'nus')
old_lower=$(printf '%s%s' 'gi' 'nus')
old_upper=$(printf '%s%s' 'GI' 'NUS')
old_repo=$(printf '%s%s' 'us' 'shim')
pattern="$old_title|$old_lower|$old_upper|$old_repo"

set +e
matches=$(rg -n \
    --hidden \
    --glob '!build/**' \
    --glob '!target/**' \
    --glob '!.git/**' \
    --glob '!hdae/gates-done/**' \
    --glob '!hdae/gates-todo/7.5-ginux-us-rename.md' \
    --glob '!scripts/check-name.sh' \
    "$pattern" .)
status=$?
set -e

case "$status" in
    0)
        printf '%s\n' 'check-name: stale active identifiers found:' >&2
        printf '%s\n' "$matches" >&2
        exit 1
        ;;
    1) ;;
    *) exit "$status" ;;
esac

set +e
paths=$(find . \
    -path './.git' -prune -o \
    -path './build' -prune -o \
    -path './target' -prune -o \
    -path './hdae/gates-done' -prune -o \
    -print | grep -E "$pattern")
status=$?
set -e

case "$status" in
    0)
        printf '%s\n' 'check-name: stale active path identifiers found:' >&2
        printf '%s\n' "$paths" >&2
        exit 1
        ;;
    1) ;;
    *) exit "$status" ;;
esac

printf '%s\n' 'check-name: PASS (active project is Ginux-us; command is ginux)'
