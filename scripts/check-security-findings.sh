#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
document=docs/security-review.md
test -r "$document"

expected='SR-01
SR-02
SR-03
SR-04
SR-05'
actual=$(sed -n 's/^| \(SR-[0-9][0-9]\) |.*$/\1/p' "$document")
test "$actual" = "$expected" || {
    printf '%s\n' 'security finding ledger has missing, duplicate, or reordered IDs' >&2
    exit 1
}

if sed -n '/^| SR-/p' "$document" | grep -Ev '\| Fixed:.*\| [^|][^|]* \|$' >/dev/null; then
    printf '%s\n' 'security finding is missing a fixed disposition or evidence' >&2
    exit 1
fi

grep -Fq 'No unresolved high- or critical-impact finding remains' "$document"
grep -Fq 'not an independent audit' "$document"
printf '%s\n' 'SECURITY_FINDINGS: PASS (5 fixed findings, evidence mapped)'
