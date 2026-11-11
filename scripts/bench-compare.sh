#!/bin/sh
set -eu

current_results=${1:-build/gate11/results.tsv}
current_metadata=${2:-build/gate11/metadata.txt}
accepted_results=${3:-benchmarks/gate11/accepted-results.tsv}
accepted_metadata=${4:-benchmarks/gate11/accepted-metadata.txt}
tolerance=${GINUX_BENCH_TOLERANCE:-0.04}
require_native_budget=${GINUX_BENCH_REQUIRE_NATIVE_BUDGET:-1}

test -s "$current_results"
test -s "$current_metadata"
awk -v tolerance="$tolerance" 'BEGIN {
    if (tolerance !~ /^([0-9]+([.][0-9]+)?|[.][0-9]+)$/ ||
        tolerance + 0 < 0 || tolerance + 0 > 1) {
        exit 1
    }
}' || {
    printf 'invalid benchmark tolerance: %s\n' "$tolerance" >&2
    exit 2
}

case "$require_native_budget" in
    0|1) ;;
    *)
        printf 'invalid native-budget requirement: %s\n' \
            "$require_native_budget" >&2
        exit 2
        ;;
esac

if test "$require_native_budget" -eq 1; then
    awk -v tolerance="$tolerance" -f scripts/gate11-budget.awk "$current_results"
else
    printf '%s\n' 'GATE11_BUDGET: SKIP (accepted-Ginux regression comparison only)'
fi

if test ! -s "$accepted_results" && test ! -s "$accepted_metadata"; then
    printf '%s\n' 'GATE11_REGRESSION: SKIP (no accepted Ginux baseline yet)'
    exit 0
fi
test -s "$accepted_results"
test -s "$accepted_metadata"

keys='host_kernel host_arch host_vcpus sysbench_nevra sysbench_sha256 rocky_release instance_image instance_config_sha256 benchmark_duration_seconds repetitions memory_limit_bytes cpu_affinity audit_enabled dispatch_mode ginux_supervisor_cpu'
for key in $keys; do
    current=$(awk -F= -v key="$key" '$1 == key { sub(/^[^=]*=/, ""); print; found=1; exit } END { if (!found) exit 1 }' "$current_metadata") || {
        printf 'GATE11_ENVIRONMENT: INCOMPATIBLE missing-current=%s\n' "$key" >&2
        exit 2
    }
    accepted=$(awk -F= -v key="$key" '$1 == key { sub(/^[^=]*=/, ""); print; found=1; exit } END { if (!found) exit 1 }' "$accepted_metadata") || {
        printf 'GATE11_ENVIRONMENT: INCOMPATIBLE missing-accepted=%s\n' "$key" >&2
        exit 2
    }
    if test "$current" != "$accepted"; then
        printf 'GATE11_ENVIRONMENT: INCOMPATIBLE key=%s accepted=%s current=%s\n' \
            "$key" "$accepted" "$current" >&2
        exit 2
    fi
done
printf '%s\n' 'GATE11_ENVIRONMENT: COMPATIBLE'
awk -v tolerance="$tolerance" -v accepted_file="$accepted_results" \
    -f scripts/gate11-regression.awk \
    "$accepted_results" "$current_results"
