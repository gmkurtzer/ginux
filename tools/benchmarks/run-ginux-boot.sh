#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tools/benchmarks/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

mkdir -p "$benchmark_dir"
volume="$benchmark_asset_dir/ginux-volume.raw"
summary="$benchmark_dir/ginux-boot-summary.tsv"
accelerator=$(benchmark_accelerator)
cpu_model=$(benchmark_cpu "$accelerator")

run_boot() {
    local label=$1
    local require_cache=$2
    local log="$benchmark_dir/ginux-boot-$label.log"
    local metrics="$benchmark_dir/ginux-boot-$label.metrics"

    rm -f "$metrics"
    GINUX_ROCKY_LOG="$log" \
    GINUX_ROCKY_METRICS_FILE="$metrics" \
    GINUX_VOLUME_IMAGE="$volume" \
    GINUX_MEMORY="$BENCHMARK_MEMORY" \
    GINUX_CPUS="$BENCHMARK_VCPUS" \
    GINUX_ROCKY_IMAGE="$ROCKY_GINUX_IMAGE" \
    GINUX_REQUIRE_OCI_CACHE_HIT="$require_cache" \
    GINUX_ROCKY_RUNTIME_TIMEOUT=600 \
    "$benchmark_root/tests/qemu/rocky-systemd.sh"

    [[ -s "$metrics" ]] || {
        echo "benchmark: Ginux run $label did not write boot metrics" >&2
        exit 1
    }
}

# Populate and verify the persistent OCI cache outside the measured runs.
run_boot warmup 0

printf 'run\tqemu_start_to_multi_user_ms\taccelerator\tcpu\tvcpus\tthreads\tmemory\n' >"$summary"
for ((run = 1; run <= BENCHMARK_BOOT_RUNS; run++)); do
    run_boot "$run" 1
    value=$(awk -F '\t' '$1 == "qemu_start_to_multi_user_ms" { print $2 }' \
        "$benchmark_dir/ginux-boot-$run.metrics")
    [[ "$value" =~ ^[0-9]+$ ]] || {
        echo "benchmark: invalid Ginux boot metric for run $run" >&2
        exit 1
    }
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$run" "$value" "$accelerator" "$cpu_model" \
        "$BENCHMARK_VCPUS" "$BENCHMARK_THREADS" "$BENCHMARK_MEMORY" >>"$summary"
done

echo "Ginux boot benchmark completed: $summary"
