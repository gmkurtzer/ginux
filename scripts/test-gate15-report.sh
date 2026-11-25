#!/bin/sh
set -eu

test_dir=$(mktemp -d "${TMPDIR:-/tmp}/ginux-gate15.XXXXXX")
cleanup() {
    rm -rf "$test_dir"
}
trap cleanup 0 HUP INT TERM

config=$test_dir/config
mkdir -p "$config" "$test_dir/selective" "$test_dir/paranoid"
cp benchmarks/gate15/native.conf benchmarks/gate15/selective.conf \
    benchmarks/gate15/paranoid.conf benchmarks/gate15/cpu.conf \
    benchmarks/gate15/memory.conf benchmarks/gate15/threads.conf \
    benchmarks/gate15/mutex.conf "$config/"
cat >"$config/suite.conf" <<'EOF'
schema=1
duration_seconds=2
repetitions=2
vcpus=1,2
workloads=cpu,threads
audit_enabled=0
capture_audit=0
supervisor_cpu=shared
EOF

metadata() {
    destination=$1
    mode=$2
    cat >"$destination" <<EOF
ginux_commit=test-commit
ginux_working_tree=clean
benchmark_duration_seconds=2
repetitions=2
warmups_per_row=1
memory_limit_bytes=1073741824
cpu_affinity=logical CPUs 0 through allocation-1
ordering=alternating native/Ginux by repetition
audit_enabled=0
capture_per_run_audit=0
dispatch_mode=$mode
ginux_supervisor_cpu=shared-with-instance
host_kernel=test-kernel
host_arch=x86_64
host_vcpus=4
host_memory_kib=4194304
sysbench_nevra=sysbench-test
sysbench_sha256=test-digest
rocky_release=Rocky test
instance_image=test-image
instance_config_sha256=$mode-config
EOF
}

results() {
    destination=$1
    factor=$2
    printf 'side\tworkload\tvcpus\tthreads\trepetition\tdirection\tunit\tvalue\n' >"$destination"
    for workload in cpu threads; do
        for specification in 1:1 2:1 2:2; do
            vcpus=${specification%%:*}
            threads=${specification#*:}
            repetition=1
            while test "$repetition" -le 2; do
                native=$((1000 + repetition * 10 + vcpus + threads))
                ginux=$(awk -v native="$native" -v factor="$factor" 'BEGIN { printf "%.3f", native * factor }')
                printf 'native\t%s\t%s\t%s\t%s\thigher\tevents/s\t%s\n' \
                    "$workload" "$vcpus" "$threads" "$repetition" "$native" >>"$destination"
                printf 'ginux\t%s\t%s\t%s\t%s\thigher\tevents/s\t%s\n' \
                    "$workload" "$vcpus" "$threads" "$repetition" "$ginux" >>"$destination"
                repetition=$((repetition + 1))
            done
        done
    done
}

./scripts/validate-gate15-config.sh "$config" >/dev/null
metadata "$test_dir/selective/metadata.txt" selective
metadata "$test_dir/paranoid/metadata.txt" paranoid
results "$test_dir/selective/results.tsv" 0.98
results "$test_dir/paranoid/results.tsv" 0.40
./scripts/combine-gate15-results.sh "$test_dir/selective/results.tsv" \
    "$test_dir/paranoid/results.tsv" "$test_dir/results.tsv"
./scripts/validate-gate15-results.sh "$test_dir/results.tsv" \
    "$test_dir/selective/metadata.txt" "$test_dir/paranoid/metadata.txt" \
    "$config" >/dev/null
./scripts/gate15-report.sh "$test_dir" "$config" >/dev/null

grep -Fq '# Gate 15 dispatch-mode performance matrix' "$test_dir/report.md"
grep -Fq '### Selective' "$test_dir/report.md"
grep -Fq '### Paranoid' "$test_dir/report.md"
grep -Fq '| threads | 2 | 2 |' "$test_dir/report.md"
grep -Fq '0.9800x' "$test_dir/report.md"
grep -Fq '0.4000x' "$test_dir/report.md"
grep -Fq '| 2/2 |' "$test_dir/report.md"
grep -Fq '<svg ' "$test_dir/performance.svg"
grep -Fq 'selective' "$test_dir/performance.svg"
grep -Fq 'paranoid' "$test_dir/performance.svg"

cp "$test_dir/results.tsv" "$test_dir/missing.tsv"
sed -n '1,$p' "$test_dir/missing.tsv" | sed '$d' >"$test_dir/missing.tmp"
mv "$test_dir/missing.tmp" "$test_dir/missing.tsv"
if ./scripts/validate-gate15-results.sh "$test_dir/missing.tsv" \
    "$test_dir/selective/metadata.txt" "$test_dir/paranoid/metadata.txt" \
    "$config" >/dev/null 2>&1; then
    printf '%s\n' 'missing Gate 15 row unexpectedly passed' >&2
    exit 1
fi

sed 's/^host_kernel=.*/host_kernel=other-kernel/' \
    "$test_dir/paranoid/metadata.txt" >"$test_dir/paranoid/incompatible.txt"
if ./scripts/validate-gate15-results.sh "$test_dir/results.tsv" \
    "$test_dir/selective/metadata.txt" "$test_dir/paranoid/incompatible.txt" \
    "$config" >/dev/null 2>&1; then
    printf '%s\n' 'incompatible Gate 15 metadata unexpectedly passed' >&2
    exit 1
fi

sed 's/^workloads=.*/workloads=cpu,unknown/' "$config/suite.conf" >"$config/invalid.conf"
mv "$config/suite.conf" "$config/suite.valid"
mv "$config/invalid.conf" "$config/suite.conf"
if ./scripts/validate-gate15-config.sh "$config" >/dev/null 2>&1; then
    printf '%s\n' 'invalid Gate 15 workload unexpectedly passed' >&2
    exit 1
fi
mv "$config/suite.valid" "$config/suite.conf"

printf '%s\n' 'test-gate15-report: PASS'
