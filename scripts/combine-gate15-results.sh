#!/bin/sh
set -eu

selective=${1:?usage: combine-gate15-results SELECTIVE_RESULTS PARANOID_RESULTS OUTPUT}
paranoid=${2:?usage: combine-gate15-results SELECTIVE_RESULTS PARANOID_RESULTS OUTPUT}
output=${3:?usage: combine-gate15-results SELECTIVE_RESULTS PARANOID_RESULTS OUTPUT}

expected='side workload vcpus threads repetition direction unit value'
for input in "$selective" "$paranoid"; do
    test -s "$input"
    header=$(sed -n '1p' "$input" | tr '\t' ' ')
    test "$header" = "$expected" || {
        printf 'invalid Gate 15 source header: %s\n' "$input" >&2
        exit 2
    }
done

mkdir -p "$(dirname "$output")"
printf 'mode\tpair\tworkload\tvcpus\tthreads\trepetition\tdirection\tunit\tvalue\n' >"$output"

awk -F '\t' -v OFS='\t' -v pair=selective '
    NR == 1 { next }
    $1 == "native" { mode = "native" }
    $1 == "ginux" { mode = pair }
    $1 != "native" && $1 != "ginux" { exit 2 }
    { print mode, pair, $2, $3, $4, $5, $6, $7, $8 }
' "$selective" >>"$output"

awk -F '\t' -v OFS='\t' -v pair=paranoid '
    NR == 1 { next }
    $1 == "native" { mode = "native" }
    $1 == "ginux" { mode = pair }
    $1 != "native" && $1 != "ginux" { exit 2 }
    { print mode, pair, $2, $3, $4, $5, $6, $7, $8 }
' "$paranoid" >>"$output"
