#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tools/benchmarks/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

linux_log="$benchmark_dir/rocky-linux-run-1-results.log"
ginux_log=$(find "$benchmark_dir" -maxdepth 1 -type f \
    \( -name 'ginux-sysbench-batch-*.log' -o -name 'ginux-sysbench-cpu-*.log' \) \
    -print | LC_ALL=C sort | sed -n '1p')

[[ -s "$linux_log" ]] || {
    echo "benchmark: missing Linux results; run 'make benchmark-linux-comparison'" >&2
    exit 1
}
[[ -n "$ginux_log" && -s "$ginux_log" ]] || {
    echo "benchmark: missing Ginux results; run 'make benchmark-ginux-sysbench'" >&2
    exit 1
}

linux_hash=$(sed -n 's/^GINUX_BENCH_SYSBENCH_SHA256 //p' "$linux_log" | tail -n 1 | tr -d '\r' || true)
ginux_hash=$(sed -n 's/^GINUX_BENCH_SYSBENCH_SHA256 //p' "$ginux_log" | tail -n 1 | tr -d '\r' || true)

[[ "$linux_hash" =~ ^[[:xdigit:]]{64}$ ]] || {
    echo "benchmark: Linux sysbench hash was not recorded" >&2
    exit 1
}
[[ "$ginux_hash" =~ ^[[:xdigit:]]{64}$ ]] || {
    echo "benchmark: Ginux sysbench hash was not recorded" >&2
    exit 1
}
[[ "$linux_hash" == "$ginux_hash" ]] || {
    echo "benchmark: sysbench binaries differ" >&2
    echo "  Linux: $linux_hash" >&2
    echo "  Ginux: $ginux_hash" >&2
    exit 1
}

python3 "$benchmark_root/tools/benchmarks/summarize-sysbench.py" "$benchmark_dir"

cat <<EOF
Benchmark inputs match:
  platform: linux/amd64
  machine:  $BENCHMARK_MACHINE
  vCPUs:    $BENCHMARK_VCPUS
  threads:  $BENCHMARK_THREADS
  memory:   $BENCHMARK_MEMORY
  sysbench: $linux_hash

Raw results:
  $benchmark_dir/rocky-linux-summary.tsv
  $benchmark_dir/rocky-linux-run-*-results.log
  $benchmark_dir/ginux-sysbench-*.log
  $benchmark_dir/sysbench-comparison.tsv
EOF
