#!/bin/sh
# Check scripts/ci-changes.sh against a scratch repository.
set -eu

script=$(cd "$(dirname "$0")" && pwd)/ci-changes.sh
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"

git -c init.defaultBranch=main init -q repo
cd repo
commit() {
    git add -A
    git -c user.name=test -c user.email=test@example.invalid \
        commit -q --allow-empty -m "$1"
    git rev-parse HEAD
}
put() {
    mkdir -p "$(dirname "$1")"
    printf '%s\n' "$2" >"$1"
}

# main: code1 - docs1 - code2 - nested - moved - empty
put crates/core/src/lib.rs 'pub fn one() {}'
put README.md 'readme'
code1=$(commit 'code')
put docs/notes.md 'notes'
put hdae/gates-todo/11.md 'gate'
put README.md 'readme, edited'
put LICENSE 'license'
docs1=$(commit 'docs only')
put crates/core/src/lib.rs 'pub fn two() {}'
code2=$(commit 'more code')
put crates/core/README.md 'crate readme'
nested=$(commit 'markdown inside a crate')
git mv crates/core/src/lib.rs docs/lib.rs.md
moved=$(commit 'move code into docs')
empty=$(commit 'empty')

# A pull request branched at code1 that adds documentation, and a target
# branch that, unlike main, gained only documentation since code1.
git checkout -q -b pr "$code1"
put docs/pr.md 'pr notes'
pr_docs=$(commit 'pr docs')
git checkout -q -b docs-target "$code1"
put docs/target.md 'target notes'
docs_target=$(commit 'target docs')
git checkout -q main

failures=0
check() {
    want=$1 name=$2
    shift 2
    got=$(sh "$script" "$@" 2>/dev/null)
    if test "$got" = "code=$want"; then
        printf 'ok   %s\n' "$name"
    else
        printf 'FAIL %s: got %s, want code=%s\n' "$name" "$got" "$want"
        failures=$((failures + 1))
    fi
}
# expect WANT NAME HEAD TARGET PASSED...
expect() {
    want=$1 name=$2 head=$3 target=$4
    shift 4
    printf '%s\n' "$@" >../passed
    if test -n "$target"; then
        check "$want" "$name" "$head" ../passed "$target"
    else
        check "$want" "$name" "$head" ../passed
    fi
}

expect false 'docs after a passing commit' "$docs1" '' "$code1"
expect false 'newest passing ancestor wins' "$docs1" '' "$code2" "$code1"
expect true 'code after a passing commit' "$code2" '' "$docs1"
expect true 'code before the passing docs commit' "$code2" '' "$code1"
expect true 'no passing ancestor' "$docs1" '' "$code2"
expect true 'empty passing list' "$docs1" ''
expect true 're-run of a passing commit' "$docs1" '' "$docs1" "$code1"
expect true 'markdown inside a crate' "$nested" '' "$code2"
expect true 'code moved into docs/' "$moved" '' "$nested"
expect true 'no files changed' "$empty" '' "$moved"
expect true 'unknown head' 0123456789abcdef0123456789abcdef01234567 '' "$code1"
check true 'missing passing list' "$docs1" ../missing
expect false 'pull request, target gained docs' "$pr_docs" "$docs_target" "$code1"
expect true 'pull request, target gained code' "$pr_docs" "$empty" "$code1"
expect true 'pull request, unknown target' "$pr_docs" 0123456789abcdef0123456789abcdef01234567 "$code1"

# CI clones with --filter=blob:none; the script must not need file contents.
# GIT_NO_LAZY_FETCH makes a missing blob an error instead of a fetch.
git config uploadpack.allowFilter true
git -C .. clone -q --no-local --filter=blob:none --no-checkout repo partial
cd ../partial
GIT_NO_LAZY_FETCH=1
export GIT_NO_LAZY_FETCH
expect false 'blobless clone, docs only' "$docs1" '' "$code1"
expect false 'blobless clone, pull request' "$pr_docs" "$docs_target" "$code1"
expect true 'blobless clone, code' "$code2" '' "$docs1"

test "$failures" -eq 0 || exit 1
printf '%s\n' 'test-ci-changes: PASS'
