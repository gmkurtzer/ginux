#!/bin/sh
set -eu

test_dir=$(mktemp -d "${TMPDIR:-/tmp}/ginux-bench-compare.XXXXXX")
cleanup() {
    rm -rf "$test_dir"
}
trap cleanup 0 HUP INT TERM

metadata() {
    destination=$1
    kernel=$2
    cat >"$destination" <<EOF
host_kernel=$kernel
host_arch=x86_64
host_vcpus=4
sysbench_nevra=sysbench-test
sysbench_sha256=test-digest
rocky_release=Rocky test
instance_image=test-image
instance_config_sha256=test-config
benchmark_duration_seconds=5
repetitions=3
memory_limit_bytes=1073741824
cpu_affinity=test-affinity
audit_mode=stderr
dispatch_mode=selective
ginux_supervisor_cpu=shared-with-instance
EOF
}

results() {
    destination=$1
    native=$2
    ginux=$3
    cat >"$destination" <<EOF
side	workload	vcpus	threads	repetition	direction	unit	value
native	cpu	1	1	1	higher	events/s	$native
native	cpu	1	1	2	higher	events/s	$native
native	cpu	1	1	3	higher	events/s	$native
ginux	cpu	1	1	1	higher	events/s	$ginux
ginux	cpu	1	1	2	higher	events/s	$ginux
ginux	cpu	1	1	3	higher	events/s	$ginux
EOF
}

metadata "$test_dir/current.meta" test-kernel
metadata "$test_dir/accepted.meta" test-kernel
results "$test_dir/accepted.tsv" 100 100
results "$test_dir/accepted-native-fail.tsv" 100 95
results "$test_dir/pass.tsv" 100 97
results "$test_dir/native-fail.tsv" 100 95
results "$test_dir/regression-fail.tsv" 100 95

./scripts/bench-compare.sh "$test_dir/pass.tsv" "$test_dir/current.meta" \
    "$test_dir/accepted.tsv" "$test_dir/accepted.meta" >/dev/null

if ./scripts/bench-compare.sh "$test_dir/native-fail.tsv" "$test_dir/current.meta" \
    "$test_dir/accepted.tsv" "$test_dir/accepted.meta" >/dev/null 2>&1; then
    printf '%s\n' 'native budget perturbation unexpectedly passed' >&2
    exit 1
fi

GINUX_BENCH_REQUIRE_NATIVE_BUDGET=0 ./scripts/bench-compare.sh \
    "$test_dir/native-fail.tsv" "$test_dir/current.meta" \
    "$test_dir/accepted-native-fail.tsv" "$test_dir/accepted.meta" >/dev/null

# This row passes a 4% native budget when native is lowered to 98, while
# still regressing 5% from the accepted Ginux value of 100.
results "$test_dir/regression-fail.tsv" 98 95
if ./scripts/bench-compare.sh "$test_dir/regression-fail.tsv" "$test_dir/current.meta" \
    "$test_dir/accepted.tsv" "$test_dir/accepted.meta" >/dev/null 2>&1; then
    printf '%s\n' 'accepted Ginux regression perturbation unexpectedly passed' >&2
    exit 1
fi

metadata "$test_dir/incompatible.meta" other-kernel
set +e
./scripts/bench-compare.sh "$test_dir/pass.tsv" "$test_dir/incompatible.meta" \
    "$test_dir/accepted.tsv" "$test_dir/accepted.meta" >/dev/null 2>&1
status=$?
set -e
test "$status" -eq 2

sed 's/^dispatch_mode=selective$/dispatch_mode=paranoid/' \
    "$test_dir/current.meta" >"$test_dir/incompatible-dispatch.meta"
set +e
./scripts/bench-compare.sh "$test_dir/pass.tsv" "$test_dir/incompatible-dispatch.meta" \
    "$test_dir/accepted.tsv" "$test_dir/accepted.meta" >/dev/null 2>&1
status=$?
set -e
test "$status" -eq 2

if GINUX_BENCH_TOLERANCE=invalid ./scripts/bench-compare.sh \
    "$test_dir/pass.tsv" "$test_dir/current.meta" \
    "$test_dir/accepted.tsv" "$test_dir/accepted.meta" >/dev/null 2>&1; then
    printf '%s\n' 'invalid tolerance unexpectedly passed' >&2
    exit 1
fi


if GINUX_BENCH_REQUIRE_NATIVE_BUDGET=invalid ./scripts/bench-compare.sh \
    "$test_dir/pass.tsv" "$test_dir/current.meta" \
    "$test_dir/accepted.tsv" "$test_dir/accepted.meta" >/dev/null 2>&1; then
    printf '%s\n' 'invalid native-budget requirement unexpectedly passed' >&2
    exit 1
fi

./scripts/bench-compare.sh "$test_dir/pass.tsv" "$test_dir/current.meta" \
    "$test_dir/missing.tsv" "$test_dir/missing.meta" >/dev/null

printf '%s\n' 'test-bench-compare: PASS'
