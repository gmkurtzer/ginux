#!/bin/sh
set -eu

results=${1:?usage: validate-gate15-results RESULTS SELECTIVE_METADATA PARANOID_METADATA CONFIG_DIR}
selective_metadata=${2:?usage: validate-gate15-results RESULTS SELECTIVE_METADATA PARANOID_METADATA CONFIG_DIR}
paranoid_metadata=${3:?usage: validate-gate15-results RESULTS SELECTIVE_METADATA PARANOID_METADATA CONFIG_DIR}
config_dir=${4:?usage: validate-gate15-results RESULTS SELECTIVE_METADATA PARANOID_METADATA CONFIG_DIR}

./scripts/validate-gate15-config.sh "$config_dir" >/dev/null

metadata_value() {
    file=$1
    key=$2
    awk -F= -v key="$key" '$1 == key { count++; value = substr($0, index($0, "=") + 1) } END { if (count != 1) exit 1; print value }' "$file"
}

for file in "$selective_metadata" "$paranoid_metadata"; do
    test -s "$file"
done
test "$(metadata_value "$selective_metadata" dispatch_mode)" = selective
test "$(metadata_value "$paranoid_metadata" dispatch_mode)" = paranoid

compatible_keys='ginux_commit ginux_working_tree benchmark_duration_seconds repetitions warmups_per_row memory_limit_bytes cpu_affinity ordering audit_enabled capture_per_run_audit ginux_supervisor_cpu host_kernel host_arch host_vcpus host_memory_kib sysbench_nevra sysbench_sha256 rocky_release instance_image'
for key in $compatible_keys; do
    selective_value=$(metadata_value "$selective_metadata" "$key")
    paranoid_value=$(metadata_value "$paranoid_metadata" "$key")
    test "$selective_value" = "$paranoid_value" || {
        printf 'Gate 15 metadata mismatch: %s selective=%s paranoid=%s\n' \
            "$key" "$selective_value" "$paranoid_value" >&2
        exit 2
    }
done

suite=$config_dir/suite.conf
config_value() {
    awk -F= -v key="$2" '$1 == key { print substr($0, index($0, "=") + 1); found=1 } END { if (!found) exit 1 }' "$1"
}
repetitions=$(config_value "$suite" repetitions)
vcpus=$(config_value "$suite" vcpus)
workloads=$(config_value "$suite" workloads)

awk -F '\t' -v repetitions="$repetitions" -v allocations="$vcpus" -v workloads="$workloads" '
    BEGIN {
        expected_header = "mode\tpair\tworkload\tvcpus\tthreads\trepetition\tdirection\tunit\tvalue"
        split(allocations, allocation, ",")
        split(workloads, workload, ",")
        for (w in workload) {
            for (a in allocation) {
                add_expected(workload[w], allocation[a], 1)
                if (allocation[a] != 1) add_expected(workload[w], allocation[a], allocation[a])
            }
        }
    }
    function add_expected(workload_name, vcpu_count, thread_count,    repetition, key) {
        for (repetition = 1; repetition <= repetitions; repetition++) {
            key = "native" SUBSEP "selective" SUBSEP workload_name SUBSEP vcpu_count SUBSEP thread_count SUBSEP repetition
            expected[key] = 1
            key = "selective" SUBSEP "selective" SUBSEP workload_name SUBSEP vcpu_count SUBSEP thread_count SUBSEP repetition
            expected[key] = 1
            key = "native" SUBSEP "paranoid" SUBSEP workload_name SUBSEP vcpu_count SUBSEP thread_count SUBSEP repetition
            expected[key] = 1
            key = "paranoid" SUBSEP "paranoid" SUBSEP workload_name SUBSEP vcpu_count SUBSEP thread_count SUBSEP repetition
            expected[key] = 1
        }
    }
    NR == 1 {
        if ($0 != expected_header) {
            print "invalid Gate 15 result header" > "/dev/stderr"
            exit 2
        }
        next
    }
    {
        key = $1 SUBSEP $2 SUBSEP $3 SUBSEP $4 SUBSEP $5 SUBSEP $6
        if (!(key in expected)) {
            print "unexpected Gate 15 row at line " NR > "/dev/stderr"
            exit 2
        }
        if (seen[key]++) {
            print "duplicate Gate 15 row at line " NR > "/dev/stderr"
            exit 2
        }
        if ($7 != "higher" && $7 != "lower") {
            print "invalid Gate 15 direction at line " NR > "/dev/stderr"
            exit 2
        }
        if ($9 !~ /^[0-9]+([.][0-9]+)?$/ || $9 + 0 <= 0) {
            print "invalid Gate 15 value at line " NR > "/dev/stderr"
            exit 2
        }
    }
    END {
        if (NR == 1) exit 2
        for (key in expected) {
            if (!seen[key]) {
                print "missing Gate 15 row" > "/dev/stderr"
                exit 2
            }
        }
    }
' "$results"

printf 'GATE15_RESULTS: PASS rows=%s\n' "$(($(wc -l <"$results") - 1))"
