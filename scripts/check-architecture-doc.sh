#!/bin/sh
set -eu

document=docs/how-ginux-differs.md
test -f "$document"

for text in \
    'How the current implementation runs code' \
    'Why Ginux is not User-Mode Linux' \
    'Why namespaces and seccomp are not the same layer' \
    'gVisor is the closest established comparison' \
    'Why Ginux is not a VM or microVM' \
    'What is distinctive about Ginux' \
    'does not promise that an arbitrary Linux program will run' \
    'not evidence of correctness'; do
    grep -Fq "$text" "$document" || {
        printf 'architecture comparison is missing required claim: %s\n' "$text" >&2
        exit 1
    }
done

for text in \
    'mandatory seccomp filter' \
    'contract-approved `sched_yield`' \
    'not a VM or microVM security boundary' \
    'not User-Mode Linux' \
    'not a mature gVisor alternative'; do
    grep -Fq "$text" README.md || {
        printf 'README is missing required explanation: %s\n' "$text" >&2
        exit 1
    }
done

test -f docs/security-boundary.md
test -f hdae/ARCHITECTURE.md
grep -Fq '(docs/how-ginux-differs.md)' README.md
grep -Fq '(how-ginux-differs.md)' docs/security-boundary.md
grep -Fq '(../docs/how-ginux-differs.md)' hdae/ARCHITECTURE.md

printf '%s\n' 'check-architecture-doc: PASS (mechanism, comparisons, differentiation, and limitations visible)'
