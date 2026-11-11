#!/bin/sh
set -eu

config_dir=${GINUX_GATE15_CONFIG_DIR:-benchmarks/gate15}
output_dir=${GINUX_GATE15_OUTPUT_DIR:-build/gate15}

./scripts/validate-gate15-config.sh "$config_dir"

config_value() {
    awk -F= -v key="$2" '$1 == key { print substr($0, index($0, "=") + 1); found=1 } END { if (!found) exit 1 }' "$1"
}
suite=$config_dir/suite.conf
duration=$(config_value "$suite" duration_seconds)
repetitions=$(config_value "$suite" repetitions)
allocations=$(config_value "$suite" vcpus | tr ',' ' ')
workloads=$(config_value "$suite" workloads | tr ',' ' ')
audit=$(config_value "$suite" audit_enabled)
capture=$(config_value "$suite" capture_audit)
supervisor=$(config_value "$suite" supervisor_cpu)
test "$supervisor" != shared || supervisor=''

mkdir -p "$output_dir/config"
cp "$config_dir"/*.conf "$output_dir/config/"

for mode in selective paranoid; do
    printf 'GATE15_COLLECT: mode=%s duration=%s repetitions=%s vcpus=%s workloads=%s\n' \
        "$mode" "$duration" "$repetitions" "$allocations" "$workloads"
    GINUX_BENCH_OUTPUT_DIR="$output_dir/$mode" \
    GINUX_BENCH_DURATION="$duration" \
    GINUX_BENCH_REPETITIONS="$repetitions" \
    GINUX_BENCH_VCPUS="$allocations" \
    GINUX_BENCH_WORKLOADS="$workloads" \
    GINUX_BENCH_AUDIT_ENABLED="$audit" \
    GINUX_BENCH_CAPTURE_AUDIT="$capture" \
    GINUX_BENCH_SUPERVISOR_CPU="$supervisor" \
    GINUX_BENCH_DISPATCH_MODE="$mode" \
        ./scripts/collect-gate11-baseline.sh >/dev/null
done

./scripts/combine-gate15-results.sh \
    "$output_dir/selective/results.tsv" \
    "$output_dir/paranoid/results.tsv" \
    "$output_dir/results.tsv"
./scripts/validate-gate15-results.sh "$output_dir/results.tsv" \
    "$output_dir/selective/metadata.txt" "$output_dir/paranoid/metadata.txt" \
    "$config_dir"
./scripts/gate15-report.sh "$output_dir" "$config_dir"

printf 'GATE15_MATRIX: PASS output=%s\n' "$output_dir"
