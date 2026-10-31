#!/usr/bin/env bash

set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

source_roots=(kernel control-plane services boot)
scratch=$(mktemp "${TMPDIR:-/tmp}/ginux-writable-globals.XXXXXX")
cell_types=$(mktemp "${TMPDIR:-/tmp}/ginux-interior-types.XXXXXX")
trap 'rm -f "$scratch" "$cell_types"' EXIT

emit_matches() {
    local kind=$1
    local pattern=$2
    { rg -n --glob '*.rs' "$pattern" "${source_roots[@]}" || true; } |
        while IFS=: read -r file line declaration; do
            local rest symbol
            rest=${declaration#*static }
            rest=${rest#mut }
            symbol=${rest%%:*}
            symbol=${symbol//[[:space:]]/}
            printf '%s\t%s\t%s\t%s\t%s\n' \
                "$kind" "$file" "$line" "$symbol" \
                "${declaration#${declaration%%[![:space:]]*}}"
        done
}

emit_matches static-mut '^\s*(pub(\([^)]*\))?\s+)?static\s+mut\s+' >"$scratch"
emit_matches interior-mutable \
    '^\s*(pub(\([^)]*\))?\s+)?static\s+[A-Z0-9_]+\s*:\s*[^=]*(Atomic|Mutex|RwLock|RefCell|UnsafeCell|Once|Spin|Lock)' \
    >>"$scratch"

# Custom owner wrappers can hide atomics or UnsafeCell from the static
# declaration. Find structs whose definition contains either form of interior
# mutability, then include every static declared with one of those types.
find "${source_roots[@]}" -type f -name '*.rs' -print0 |
    while IFS= read -r -d '' file; do
        awk '
            /^(pub(\([^)]*\))? )?struct [A-Za-z_][A-Za-z0-9_]*/ {
                name = $0
                sub(/^.*struct /, "", name)
                sub(/[(<{].*/, "", name)
                gsub(/[[:space:]]/, "", name)
                remaining = 6
            }
            remaining > 0 && /(UnsafeCell|Atomic)/ { print name; remaining = 0 }
            remaining > 0 { remaining-- }
        ' "$file"
    done | sort -u >"$cell_types"

while IFS= read -r wrapper; do
    [[ -n "$wrapper" ]] || continue
    emit_matches interior-mutable \
        "^\\s*(pub(\\([^)]*\\))?\\s+)?static\\s+[A-Z0-9_]+\\s*:\\s*[^=]*${wrapper}([[:space:]<;,\\]]|=)" \
        >>"$scratch"
done <"$cell_types"

printf 'kind\tfile\tline\tsymbol\tdeclaration\n'
LC_ALL=C sort -t $'\t' -k2,2 -k4,4 -k1,1 "$scratch" | awk -F '\t' '!seen[$2 FS $4]++'
