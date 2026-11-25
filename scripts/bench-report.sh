#!/bin/sh
set -eu

input=${1:-build/gate11/results.tsv}
metadata=${2:-build/gate11/metadata.txt}
output=${3:-build/gate11/report.md}

test -s "$input"
test -s "$metadata"
mkdir -p "$(dirname "$output")"

{
    printf '%s\n\n' '# Gate 11 matched sysbench baseline'
    printf '%s\n\n' 'Native Rocky is the 1.00x control. Performance below 1.00x is slower; values above 1.00x are faster. Each row is judged independently against the default 4% unfavorable tolerance.'
    awk -f scripts/gate11-report.awk "$input"
    printf '\n%s\n\n' '## Environment and protocol'
    printf '%s\n' '```text'
    cat "$metadata"
    printf '%s\n' '```'
} >"$output"

printf 'Gate 11 report: %s\n' "$output"
