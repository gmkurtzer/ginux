#!/bin/sh
set -eu

document=docs/security-boundary.md
test -f "$document"

for text in \
    'not production-ready' \
    'Host kernel and ptrace' \
    'Residual supervisor authority' \
    'NoNewPrivs' \
    'Known compatibility gaps' \
    'Performance limitation' \
    'Independent security review'; do
    grep -Fq "$text" "$document" || {
        printf 'security boundary is missing required claim: %s\n' "$text" >&2
        exit 1
    }
done

grep -Fq '(docs/security-boundary.md)' README.md
printf '%s\n' 'check-security-doc: PASS (claims, residual authority, gaps, performance, and review status visible)'
