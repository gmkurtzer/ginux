#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
stress_runs="${GINUX_SMP_STRESS_RUNS:-2}"
output_file="$(mktemp -t ginux-smp-matrix.XXXXXX)"

cleanup() {
    rm -f "$output_file"
}
trap cleanup EXIT

if [[ ! "$stress_runs" =~ ^[1-9][0-9]*$ ]]; then
    echo "GINUX_SMP_STRESS_RUNS must be a positive integer" >&2
    exit 2
fi

run_case() {
    local cpus="$1"
    local label="$2"

    if ! GINUX_CPUS="$cpus" "$repo_root/tests/qemu/boot.sh" >"$output_file" 2>&1; then
        cat "$output_file"
        echo "SMP matrix failed: $label" >&2
        exit 1
    fi
    tail -n 1 "$output_file"
}

for cpus in 1 2 4; do
    echo "Qualifying Ginux with $cpus CPU(s)"
    run_case "$cpus" "$cpus-CPU qualification"
done

for ((run = 1; run <= stress_runs; run++)); do
    echo "Running two-CPU SMP stress pass $run of $stress_runs"
    run_case 2 "two-CPU stress pass $run"
done

echo "SMP topology and stress matrix passed."
