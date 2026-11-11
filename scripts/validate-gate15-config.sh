#!/bin/sh
set -eu

config_dir=${1:-benchmarks/gate15}

value() {
    file=$1
    key=$2
    awk -F= -v key="$key" '
        /^[[:space:]]*(#|$)/ { next }
        $1 == key { count++; value = substr($0, index($0, "=") + 1) }
        END { if (count != 1 || value == "") exit 1; print value }
    ' "$file"
}

valid_csv() {
    input=$1
    allowed=$2
    awk -v input="$input" -v allowed="$allowed" '
        BEGIN {
            count = split(input, item, ",")
            if (!count) exit 1
            for (i = 1; i <= count; i++) {
                if (item[i] == "" || index(" " allowed " ", " " item[i] " ") == 0 || seen[item[i]]++) exit 1
            }
        }
    '
}

for name in suite native selective paranoid; do
    file=$config_dir/$name.conf
    test -f "$file" || {
        printf 'missing Gate 15 template: %s\n' "$file" >&2
        exit 2
    }
    test "$(value "$file" schema)" = 1 || {
        printf 'unsupported Gate 15 template schema: %s\n' "$file" >&2
        exit 2
    }
done

for specification in \
    'cpu:higher:events/s' \
    'memory:higher:MiB/s' \
    'threads:higher:events/s' \
    'mutex:lower:s/event'
do
    workload=${specification%%:*}
    remainder=${specification#*:}
    direction=${remainder%%:*}
    unit=${remainder#*:}
    file=$config_dir/$workload.conf
    test -f "$file" || {
        printf 'missing Gate 15 workload template: %s\n' "$file" >&2
        exit 2
    }
    test "$(value "$file" schema)" = 1
    test "$(value "$file" workload)" = "$workload"
    test "$(value "$file" direction)" = "$direction"
    test "$(value "$file" unit)" = "$unit"
    test "$(value "$file" threads)" = matrix
done

test "$(value "$config_dir/cpu.conf" time_seconds)" = suite
test "$(value "$config_dir/cpu.conf" cpu_max_prime)" = 10000
test "$(value "$config_dir/memory.conf" time_seconds)" = suite
test "$(value "$config_dir/memory.conf" block_size)" = 1K
test "$(value "$config_dir/memory.conf" total_size)" = 100G
test "$(value "$config_dir/memory.conf" operation)" = write
test "$(value "$config_dir/memory.conf" scope)" = global
test "$(value "$config_dir/memory.conf" access_mode)" = seq
test "$(value "$config_dir/threads.conf" time_seconds)" = suite
test "$(value "$config_dir/threads.conf" thread_yields)" = 1000
test "$(value "$config_dir/threads.conf" thread_locks)" = 8
test "$(value "$config_dir/mutex.conf" time_seconds)" = sysbench-default
test "$(value "$config_dir/mutex.conf" mutex_parameters)" = sysbench-1.0.20-defaults

for specification in \
    'native:native:none' \
    'selective:ginux:selective' \
    'paranoid:ginux:paranoid'
do
    profile=${specification%%:*}
    remainder=${specification#*:}
    runtime=${remainder%%:*}
    control=${remainder#*:}
    file=$config_dir/$profile.conf
    test "$(value "$file" profile)" = "$profile"
    test "$(value "$file" runtime)" = "$runtime"
    test "$(value "$file" syscall_control)" = "$control"
done

suite=$config_dir/suite.conf
duration=$(value "$suite" duration_seconds)
repetitions=$(value "$suite" repetitions)
vcpus=$(value "$suite" vcpus)
workloads=$(value "$suite" workloads)
audit=$(value "$suite" audit_enabled)
capture=$(value "$suite" capture_audit)
supervisor=$(value "$suite" supervisor_cpu)

case "$duration:$repetitions" in
    *[!0-9:]*|0:*|*:0|:*|*:) printf '%s\n' 'invalid Gate 15 duration or repetitions' >&2; exit 2 ;;
esac
valid_csv "$vcpus" '1 2 4' || {
    printf 'invalid Gate 15 vCPU list: %s\n' "$vcpus" >&2
    exit 2
}
valid_csv "$workloads" 'cpu memory threads mutex' || {
    printf 'invalid Gate 15 workload list: %s\n' "$workloads" >&2
    exit 2
}
case "$audit:$capture" in
    0:0|0:1|1:0|1:1) ;;
    *) printf '%s\n' 'invalid Gate 15 audit setting' >&2; exit 2 ;;
esac
case "$supervisor" in
    shared|0|1|2|3) ;;
    *) printf 'invalid Gate 15 supervisor CPU: %s\n' "$supervisor" >&2; exit 2 ;;
esac

printf 'GATE15_CONFIG: PASS directory=%s duration=%s repetitions=%s vcpus=%s workloads=%s\n' \
    "$config_dir" "$duration" "$repetitions" "$vcpus" "$workloads"
