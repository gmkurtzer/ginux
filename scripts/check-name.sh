#!/bin/sh
set -eu

old_title=$(printf '%s%s' 'Gi' 'nus')
old_lower=$(printf '%s%s' 'gi' 'nus')
old_upper=$(printf '%s%s' 'GI' 'NUS')
old_repo=$(printf '%s%s' 'us' 'shim')
transition_title=$(printf '%s%s' 'Ginux' '-us')
transition_lower=$(printf '%s%s' 'ginux' '-us')
pattern="$old_title|$old_lower|$old_upper|$old_repo|$transition_title|$transition_lower"

set +e
# Run the search on its own: in a pipeline the status would come from the
# filter, and a missing or failing rg would read as "no matches".
found=$(rg -n \
    --hidden \
    --glob '!build/**' \
    --glob '!target/**' \
    --glob '!.artifacts/**' \
    --glob '!.git/**' \
    --glob '!hdae/gates-done/**' \
    --glob '!hdae/gates-done/7.5-ginux-us-rename.md' \
    --glob '!hdae/gates-done/7.75-ginux-rename.md' \
    --glob '!scripts/check-name.sh' \
    "$pattern" .)
status=$?
if [ "$status" -gt 1 ]; then
    printf '%s\n' "check-name: rg failed with status $status" >&2
    exit 2
fi
matches=$(printf '%s\n' "$found" | grep -v -E '/Users/gmk/git/ginux-us|hdae/ginux-us-rename|github.com/gmkurtzer/ginux-us/|^$')
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
all_paths=$(find . \
    -path './.git' -prune -o \
    -path './build' -prune -o \
    -path './target' -prune -o \
    -path './.artifacts' -prune -o \
    -path './hdae/gates-done' -prune -o \
    -path './hdae/gates-done/7.5-ginux-us-rename.md' -prune -o \
    -print)
status=$?
if [ "$status" -ne 0 ]; then
    printf '%s\n' "check-name: find failed with status $status" >&2
    exit 2
fi
paths=$(printf '%s\n' "$all_paths" | grep -E "$pattern")
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

printf '%s\n' 'check-name: PASS (active project is Ginux; command is ginux)'
