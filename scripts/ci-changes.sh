#!/bin/sh
# Decide whether CI has code to test at HEAD_SHA.
#
# Usage: ci-changes.sh HEAD_SHA PASSED_SHAS_FILE [TARGET_SHA]
#
# PASSED_SHAS_FILE lists commits that already passed the check workflow, one
# per line. The newest of them that is an ancestor of HEAD_SHA is the green
# commit. For a pull request, TARGET_SHA is the tip of the branch it merges
# into: CI tests the merge, so changes on the target that HEAD_SHA does not
# have count as well.
#
# Prints "code=false" only when every changed file is documentation, so the
# tested code is the same as in a passing run. Anything else, including any
# failure to decide, prints "code=true". Reasons go to standard error.
set -eu

head=$1
passed=$2
target=${3-}

# Documentation: nothing in these paths is built, run or read by a test.
# Markdown below crates/, scripts/ or tests/ is not listed and counts as code.
docs='^(docs|hdae|\.hdae|\.agents|\.claude)/|^[^/]*\.md$|^(LICENSE|NOTICE)$'

test_all() {
    printf '%s\n' 'code=true'
    printf 'ci-changes: running all checks: %s\n' "$1" >&2
    exit 0
}

show() {
    printf '%s\n' "$1" | sed -n '1,20s/^/  /p' >&2
    count=$(printf '%s\n' "$1" | wc -l)
    test "$count" -le 20 || printf '  ... %s files in all\n' "$count" >&2
}

test -r "$passed" || test_all "no list of passing commits"
git cat-file -e "$head^{commit}" 2>/dev/null || test_all "$head is not a known commit"
if grep -qxF "$head" "$passed"; then
    test_all "$head already passed, so this is a re-run"
fi

green=$(git rev-list --max-count=1000 "$head" |
    awk 'NR == FNR { passed[$0]; next } ($0 in passed) { print; exit }' "$passed" -) ||
    test_all "could not walk the history of $head"
test -n "$green" || test_all "no recent ancestor of $head passed"

# --no-renames lists both sides of a rename, so moving code into docs/ counts.
files=$(git diff --no-renames --name-only "$green" "$head") ||
    test_all "could not diff $green..$head"
test -n "$files" || test_all "no files changed since $green"

if test -n "$target"; then
    git cat-file -e "$target^{commit}" 2>/dev/null || test_all "$target is not a known commit"
    fork=$(git merge-base "$head" "$target") ||
        test_all "$head and $target have no merge base"
    # Everything the target gained since the branch point: a superset of what
    # it gained since the green run, whose base is not recorded.
    target_files=$(git diff --no-renames --name-only "$fork" "$target") ||
        test_all "could not diff $fork..$target"
    files=$(printf '%s\n%s\n' "$files" "$target_files" | sed '/^$/d' | sort -u)
fi

code_files=$(printf '%s\n' "$files" | grep -Ev "$docs" || true)
if test -n "$code_files"; then
    printf '%s\n' 'code=true'
    printf 'ci-changes: code changed since %s, which passed:\n' "$green" >&2
    show "$code_files"
    exit 0
fi

printf '%s\n' 'code=false'
printf 'ci-changes: skipping build and tests: only documentation changed since %s, which passed:\n' "$green" >&2
show "$files"
