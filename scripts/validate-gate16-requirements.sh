#!/bin/sh
set -eu

matrix=${1:-docs/kernel-assisted-requirements.tsv}

test -f "$matrix"
header=$(sed -n '1p' "$matrix")
test "$header" = "class	pre_effect	pointer_or_object_context	direct_allowed_path	preferred	fallback	disposition"

expected='syscall.exact-deny filesystem.path network.socket device.access process.limits ioctl.object proc.virtual sys.virtual credential.transition signal.target time.virtual memory.translation abi.emulation syscall.selected-module'
for class in $expected; do
    count=$(awk -F '\t' -v class="$class" 'NR > 1 && $1 == class { count++ } END { print count + 0 }' "$matrix")
    test "$count" -eq 1 || {
        printf 'Gate 16 matrix requires exactly one %s row (found %s)\n' "$class" "$count" >&2
        exit 1
    }
done

awk -F '\t' '
    NR == 1 { next }
    NF != 7 { printf "invalid Gate 16 matrix row %d: expected 7 columns\n", NR > "/dev/stderr"; exit 1 }
    $2 != "yes" { printf "row %d does not require pre-effect enforcement\n", NR > "/dev/stderr"; exit 1 }
    $5 == "" || $7 == "" { printf "row %d lacks a mechanism or disposition\n", NR > "/dev/stderr"; exit 1 }
    seen[$1]++ > 0 { printf "duplicate Gate 16 class %s\n", $1 > "/dev/stderr"; exit 1 }
' "$matrix"

printf 'GATE16_REQUIREMENTS: rows=%s status=PASS\n' "$(($(wc -l < "$matrix") - 1))"
