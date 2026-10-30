#!/usr/bin/env bash

set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

printf 'kind\tfile\tline\tdeclaration\n'
rg -n --glob '*.rs' '^\s*(pub\([^)]*\)\s+)?static\s+mut\s+' \
    kernel control-plane services boot | while IFS=: read -r file line declaration; do
        printf 'static-mut\t%s\t%s\t%s\n' "$file" "$line" "${declaration#${declaration%%[![:space:]]*}}"
    done
rg -n --glob '*.rs' '^\s*(pub\([^)]*\)\s+)?static\s+[A-Z0-9_]+\s*:\s*[^=]*(Atomic|Mutex|UnsafeCell|Once|Spin|Lock)' \
    kernel control-plane services boot | while IFS=: read -r file line declaration; do
        printf 'interior-mutable\t%s\t%s\t%s\n' "$file" "$line" "${declaration#${declaration%%[![:space:]]*}}"
    done

