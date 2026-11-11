#!/bin/sh
set -eu

output_dir=${1:-build/gate15}
config_dir=${2:-benchmarks/gate15}
results=$output_dir/results.tsv
selective_metadata=$output_dir/selective/metadata.txt
paranoid_metadata=$output_dir/paranoid/metadata.txt
report=$output_dir/report.md
graph=$output_dir/performance.svg

./scripts/validate-gate15-results.sh "$results" "$selective_metadata" \
    "$paranoid_metadata" "$config_dir" >/dev/null

config_value() {
    awk -F= -v key="$2" '$1 == key { print substr($0, index($0, "=") + 1); found=1 } END { if (!found) exit 1 }' "$1"
}
workloads=$(config_value "$config_dir/suite.conf" workloads)
allocations=$(config_value "$config_dir/suite.conf" vcpus)

awk -v workload_list="$workloads" -v allocation_list="$allocations" \
    -f scripts/gate15-svg.awk "$results" >"$graph"

{
    printf '%s\n\n' '# Gate 15 dispatch-mode performance matrix'
    printf '%s\n\n' 'Native Rocky is the no-Ginux control. Selective and paranoid values are normalized to the native samples paired with that mode; higher ratios are always better.'
    printf '%s\n\n' '![Native-relative performance](performance.svg)'
    printf '%s\n\n' '## Results'
    awk -v workload_list="$workloads" -v allocation_list="$allocations" \
        -f scripts/gate15-report.awk "$results"
    # shellcheck disable=SC2016
    printf '\n%s\n\n' 'Standard deviation is shown in each workload unit. Raw samples remain in `results.tsv`; each mode retains its own raw transcripts and metadata directory.'
    printf '%s\n\n' '## Security and interpretation'
    printf '%s\n' \
        '- **Native** executes without Ginux and is not an instance mode.' \
        '- **Selective** permits only contract-allowed, independently classified direct-safe syscalls to execute directly; all other operations remain controlled by Ginux.' \
        '- **Paranoid** routes every syscall through the Ginux userspace decision path before completion.'
    printf '\n'
    printf '%s\n\n' 'Do not compare modes as if they provide identical enforcement. These workloads characterize fixed and workload-sensitive overhead; they do not establish universal application performance.'
    printf '%s\n\n' '## Reproduction metadata'
    printf '%s\n' '### Selective pair' '```text'
    cat "$selective_metadata"
    printf '%s\n\n' '```'
    printf '%s\n' '### Paranoid pair' '```text'
    cat "$paranoid_metadata"
    printf '%s\n' '```'
} >"$report"

printf 'Gate 15 report: %s\nGate 15 graph: %s\n' "$report" "$graph"
