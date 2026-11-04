#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
stress_runs="${GINUX_DESKTOP_STRESS_RUNS:-2}"
output_file="$(mktemp -t ginux-desktop-stress.XXXXXX)"

cleanup() {
    rm -f "$output_file"
}
trap cleanup EXIT

if [[ ! "$stress_runs" =~ ^[1-9][0-9]*$ ]]; then
    echo "GINUX_DESKTOP_STRESS_RUNS must be a positive integer" >&2
    exit 2
fi

for cpus in 2 4; do
    for ((run = 1; run <= stress_runs; run++)); do
        echo "Desktop recovery stress: ${cpus} CPU(s), pass $run of $stress_runs"
        if ! GINUX_CPUS="$cpus" "$repo_root/tests/qemu/boot.sh" >"$output_file" 2>&1; then
            cat "$output_file"
            echo "Desktop recovery stress failed with $cpus CPU(s)" >&2
            exit 1
        fi
        if ! grep -q '^ginux: controller desktop recovery validated in ring 3' "$output_file"; then
            cat "$output_file"
            echo "Desktop recovery stress marker missing with $cpus CPU(s)" >&2
            exit 1
        fi
        tail -n 1 "$output_file"
    done
done

echo "Controller desktop SMP recovery stress passed."
