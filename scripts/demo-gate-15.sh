#!/bin/sh
set -eu

demo_config=$(mktemp -d "${TMPDIR:-/tmp}/ginux-gate15-demo.XXXXXX")
cleanup() {
    rm -rf "$demo_config"
}
trap cleanup 0 HUP INT TERM

cp benchmarks/gate15/native.conf benchmarks/gate15/selective.conf \
    benchmarks/gate15/paranoid.conf benchmarks/gate15/cpu.conf \
    benchmarks/gate15/memory.conf benchmarks/gate15/threads.conf \
    benchmarks/gate15/mutex.conf "$demo_config/"
cat >"$demo_config/suite.conf" <<'EOF'
schema=1
duration_seconds=1
repetitions=1
vcpus=1
workloads=cpu,threads
audit_enabled=0
capture_audit=0
supervisor_cpu=shared
EOF

printf '%s\n' \
    'Gate 15 demonstration: native, selective, and paranoid performance.' \
    'Prerequisite: run make bench-modes-prepare once with network access.' \
    'This bounded demo runs one-second CPU and thread workloads at one vCPU.'

GINUX_GATE15_CONFIG_DIR="$demo_config" \
GINUX_GATE15_OUTPUT_DIR=build/gate15-demo \
    ./scripts/with-gate11-host.sh ./scripts/collect-gate15-matrix.sh

printf '%s\n' \
    'GATE15_DEMO: PASS' \
    'Inspect build/gate15-demo/report.md, performance.svg, and results.tsv.'
