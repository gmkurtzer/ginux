#!/bin/sh
set -eu

instance=gate11-benchmark
native_command=/var/tmp/gate11-sysbench-command.sh
directory=/var/lib/ginux/instances/$instance
helper=/usr/local/libexec/ginux-instance-helper
base_config=/var/tmp/instance-gate11-benchmark.toml
result=${GINUX_BENCH_RESULT:-/var/tmp/gate11-results.tsv}
raw_dir=${GINUX_BENCH_RAW_DIR:-/var/tmp/gate11-raw}
duration=${GINUX_BENCH_DURATION:-1}
repetitions=${GINUX_BENCH_REPETITIONS:-3}
allocations=${GINUX_BENCH_VCPUS:-"1 2 4"}
workloads=${GINUX_BENCH_WORKLOADS:-"cpu memory threads mutex"}
audit_mode=${GINUX_BENCH_AUDIT_MODE:-stderr}
memory_bytes=1073741824

case "$duration:$repetitions" in
    *[!0-9:]*|0:*|*:0|:*|*:) printf '%s\n' 'invalid Gate 11 duration or repetition count' >&2; exit 2 ;;
esac
case "$audit_mode" in stderr|disabled) ;; *) printf '%s\n' 'invalid Gate 11 audit mode' >&2; exit 2 ;; esac

test -x /usr/bin/sysbench
test -x "$directory/root/usr/bin/sysbench"
test -x "$helper"
test -x "$native_command"
host_hash=$(sha256sum /usr/bin/sysbench | awk '{ print $1 }')
instance_hash=$(sha256sum "$directory/root/usr/bin/sysbench" | awk '{ print $1 }')
test "$host_hash" = "$instance_hash"

rm -rf "$raw_dir"
mkdir -p "$raw_dir"
printf '%s\n' 'side	workload	vcpus	threads	repetition	direction	unit	value' >"$result"

cpu_list() {
    case "$1" in
        1) printf '0' ;;
        2) printf '0,1' ;;
        4) printf '0,1,2,3' ;;
        *) return 1 ;;
    esac
}

toml_args() {
    workload=$1
    threads=$2
    case "$workload" in
        cpu)
            printf '["cpu", "--threads=%s", "--time=%s", "--cpu-max-prime=10000", "run"]' \
                "$threads" "$duration"
            ;;
        memory)
            # The omitted memory options are pinned sysbench 1.0.20 defaults:
            # 1K blocks, 100G total, sequential global writes.
            printf '["memory", "--threads=%s", "--time=%s", "run"]' \
                "$threads" "$duration"
            ;;
        threads)
            printf '["threads", "--threads=%s", "--time=%s", "--thread-yields=1000", "--thread-locks=8", "run"]' \
                "$threads" "$duration"
            ;;
        mutex)
            # Use the pinned sysbench 1.0.20 mutex defaults. The earlier
            # shortened loop completed in about 15 ms and measured fixed
            # process startup cost rather than sustained mutex behavior.
            printf '["mutex", "--threads=%s", "run"]' \
                "$threads"
            ;;
        *) return 1 ;;
    esac
}

write_config() {
    workload=$1
    vcpus=$2
    threads=$3
    args=$(toml_args "$workload" "$threads")
    sed -e "s/^vcpus = .*/vcpus = $vcpus/" \
        -e "s|^args = .*|args = $args|" \
        -e "s/^mode = \"stderr\"/mode = \"$audit_mode\"/" \
        "$base_config" | sudo tee "$directory/config.toml" >/dev/null
}

run_one() {
    side=$1
    workload=$2
    vcpus=$3
    threads=$4
    output=$5
    cpus=$(cpu_list "$vcpus")
    if test "$side" = native; then
        # Redirection is intentionally performed by the unprivileged driver;
        # sudo only selects the measured process identity.
        # shellcheck disable=SC2024
        sudo -u nobody taskset -c "$cpus" prlimit --as="$memory_bytes" -- \
            "$native_command" "$workload" "$threads" "$duration" >"$output" 2>&1
    else
        write_config "$workload" "$vcpus" "$threads"
        sudo truncate -s 0 "$directory/audit.log"
        # shellcheck disable=SC2024
        sudo taskset -c "$cpus" prlimit --as="$memory_bytes" -- \
            "$helper" run-runtime "$instance" >"$output" 2>&1
    fi
}

metric() {
    workload=$1
    output=$2
    case "$workload" in
        cpu)
            awk '/events per second:/ { print $4; found=1 } END { if (!found) exit 1 }' "$output"
            ;;
        memory)
            awk '/MiB transferred/ { value=$4; sub(/^\(/, "", value); print value; found=1 } END { if (!found) exit 1 }' "$output"
            ;;
        threads)
            awk '/total time:/ { time=$3; sub(/s$/, "", time) } /total number of events:/ { events=$5 } END { if (!time || !events) exit 1; printf "%.6f\n", events/time }' "$output"
            ;;
        mutex)
            awk '/total time:/ { time=$3; sub(/s$/, "", time) } /total number of events:/ { events=$5 } END { if (!time || !events) exit 1; printf "%.6f\n", time/events }' "$output"
            ;;
    esac
}

direction() {
    test "$1" = mutex && printf 'lower' || printf 'higher'
}

unit() {
    case "$1" in
        memory) printf 'MiB/s' ;;
        mutex) printf 's/event' ;;
        *) printf 'events/s' ;;
    esac
}

sample() {
    side=$1 workload=$2 vcpus=$3 threads=$4 repetition=$5
    output=$raw_dir/$side-$workload-v$vcpus-t$threads-r$repetition.log
    run_one "$side" "$workload" "$vcpus" "$threads" "$output"
    value=$(metric "$workload" "$output")
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$side" "$workload" "$vcpus" "$threads" "$repetition" \
        "$(direction "$workload")" "$(unit "$workload")" "$value" >>"$result"
}

run_row() {
    workload=$1 vcpus=$2 threads=$3
    warm=$raw_dir/warm-$workload-v$vcpus-t$threads
    run_one native "$workload" "$vcpus" "$threads" "$warm-native.log"
    run_one ginux "$workload" "$vcpus" "$threads" "$warm-ginux.log"

    repetition=1
    while test "$repetition" -le "$repetitions"; do
        if test $((repetition % 2)) -eq 1; then
            sample native "$workload" "$vcpus" "$threads" "$repetition"
            sample ginux "$workload" "$vcpus" "$threads" "$repetition"
        else
            sample ginux "$workload" "$vcpus" "$threads" "$repetition"
            sample native "$workload" "$vcpus" "$threads" "$repetition"
        fi
        repetition=$((repetition + 1))
    done
    printf 'GATE11_SAMPLE: workload=%s vcpus=%s threads=%s repetitions=%s\n' \
        "$workload" "$vcpus" "$threads" "$repetitions"
}

for workload in $workloads; do
    case "$workload" in cpu|memory|threads|mutex) ;; *) printf 'invalid workload: %s\n' "$workload" >&2; exit 2 ;; esac
    for vcpus in $allocations; do
        cpu_list "$vcpus" >/dev/null || { printf 'invalid vCPU allocation: %s\n' "$vcpus" >&2; exit 2; }
        run_row "$workload" "$vcpus" 1
        test "$vcpus" -eq 1 || run_row "$workload" "$vcpus" "$vcpus"
    done
done

printf 'GATE11_RESULTS: %s\n' "$result"
