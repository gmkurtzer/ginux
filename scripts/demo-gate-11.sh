#!/bin/sh
set -eu

printf '%s\n' \
    'Gate 11 demonstration: matched native and selective-dispatch Ginux benchmarks.' \
    'Prerequisite: run make bench-prepare once with network access.' \
    'This bounded run uses one-second workloads and three repetitions.' \
    'Observe: raw measurements and metadata under build/gate11, followed by' \
    'a nonzero result if any mandatory row is more than 4% slower than native.'

GINUX_BENCH_DURATION=${GINUX_BENCH_DURATION:-1} \
GINUX_BENCH_REPETITIONS=${GINUX_BENCH_REPETITIONS:-3} \
    ./scripts/with-gate11-host.sh ./scripts/collect-gate11-baseline.sh

./scripts/bench-compare.sh

printf '%s\n' \
    'Gate 11 comparison passed.' \
    'Inspect build/gate11/results.tsv, metadata.txt, raw.tar.gz, and report.md.'
