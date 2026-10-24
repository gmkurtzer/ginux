#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tools/benchmarks/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

mkdir -p "$benchmark_dir"
log="$benchmark_dir/ginux-sysbench.log"
volume="$benchmark_dir/ginux-volume.raw"

GINUX_ROCKY_LOG="$log" \
GINUX_VOLUME_IMAGE="$volume" \
GINUX_MEMORY="$BENCHMARK_MEMORY" \
GINUX_ROCKY_IMAGE="$ROCKY_GINUX_IMAGE" \
GINUX_REQUIRE_OCI_CACHE_HIT="${GINUX_REQUIRE_OCI_CACHE_HIT:-0}" \
GINUX_BENCHMARK_SYSBENCH=1 \
GINUX_BENCHMARK_WORKLOAD_RUNS="$BENCHMARK_WORKLOAD_RUNS" \
GINUX_BENCHMARK_WORKLOAD_SECONDS="$BENCHMARK_WORKLOAD_SECONDS" \
GINUX_ROCKY_RUNTIME_TIMEOUT=3600 \
"$benchmark_root/tests/qemu/rocky-systemd.sh"

echo "Ginux benchmark completed: $log"
