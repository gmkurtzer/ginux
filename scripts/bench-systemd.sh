#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

driver_log=build/gate9-systemd-bench-driver.log
result=build/gate9-systemd-bench.txt
peak_rss=0
idle_rss=0
peak_processes=0
peak_cpu=0

sample_tree() {
    root=$(rocky_ssh 'pgrep -o -x ginux || true' | tr -d '[:space:]')
    case $root in
        ''|*[!0-9]*) return 1 ;;
    esac
    rocky_ssh "ps -e -o pid=,ppid=,rss=,pcpu= | awk -v root=$root '
        { pid[NR]=\$1; parent[NR]=\$2; rss[NR]=\$3; cpu[NR]=\$4 }
        END {
            selected[root]=1
            for (pass=0; pass<NR; pass++)
                for (i=1; i<=NR; i++)
                    if (selected[parent[i]]) selected[pid[i]]=1
            for (i=1; i<=NR; i++)
                if (selected[pid[i]]) {
                    count++
                    total_rss+=rss[i]
                    total_cpu+=cpu[i]
                }
            printf \"%d %d %.1f\\n\", count, total_rss, total_cpu
        }'"
}

GINUX_GATE9_DNF=off GINUX_GATE9_NETWORK=off GINUX_GATE9_LOGIN_SETTLE=3 \
    ./scripts/probe-systemd.sh \
    >"$driver_log" 2>&1 &
probe_pid=$!
cleanup() {
    if kill -0 "$probe_pid" 2>/dev/null; then
        kill "$probe_pid" 2>/dev/null || true
        wait "$probe_pid" 2>/dev/null || true
    fi
}
trap cleanup HUP INT TERM

while kill -0 "$probe_pid" 2>/dev/null; do
    sample=$(sample_tree || true)
    if test -n "$sample"; then
        set -- $sample
        processes=$1
        rss=$2
        cpu=$3
        test "$processes" -le "$peak_processes" || peak_processes=$processes
        test "$rss" -le "$peak_rss" || peak_rss=$rss
        peak_cpu=$(awk -v old="$peak_cpu" -v new="$cpu" 'BEGIN { print (new > old ? new : old) }')
        if test "$idle_rss" -eq 0 &&
            test -s build/gate9-systemd-measurements.txt
        then
            idle_rss=$rss
        fi
    fi
    sleep 1
done
wait "$probe_pid"
trap - HUP INT TERM

boot_ms=$(sed -n 's/^GINUX_MEASURE: boot-to-login-ms=//p' build/gate9-systemd-measurements.txt | tail -n 1)
total_ms=$(sed -n 's/^GINUX_MEASURE: total-runtime-ms=//p' build/gate9-systemd-measurements.txt | tail -n 1)
idle_entries=$(sed -n 's/^GINUX_MEASURE: idle-managed-proc-entries=//p' build/gate9-systemd-baseline.log | tail -n 1 | tr -d '\r')
peak_tasks=$(sed -n 's/^GINUX_NATIVE_MEASURE: peak-managed-tasks=//p' build/gate9-systemd-baseline.log | tail -n 1 | tr -d '\r')
test -n "$boot_ms"
test -n "$total_ms"
test -n "$idle_rss"
test "$idle_rss" -gt 0
test -n "$peak_tasks"

{
    printf '%s\n' 'GINUX_SYSTEMD_BENCH: environment=Rocky-10.2-x86_64 qemu=TCG vcpus=2 memory-mib=2048 network=restricted dnf=skipped'
    printf 'GINUX_SYSTEMD_BENCH: boot-to-login-ms=%s\n' "$boot_ms"
    printf 'GINUX_SYSTEMD_BENCH: total-runtime-ms=%s\n' "$total_ms"
    printf 'GINUX_SYSTEMD_BENCH: peak-tree-rss-kib=%s\n' "$peak_rss"
    printf 'GINUX_SYSTEMD_BENCH: idle-tree-rss-kib=%s\n' "$idle_rss"
    printf 'GINUX_SYSTEMD_BENCH: peak-tree-processes=%s\n' "$peak_processes"
    printf 'GINUX_SYSTEMD_BENCH: peak-tree-cpu-percent=%s\n' "$peak_cpu"
    printf 'GINUX_SYSTEMD_BENCH: idle-proc-entries=%s\n' "$idle_entries"
    printf 'GINUX_SYSTEMD_BENCH: peak-managed-tasks=%s\n' "$peak_tasks"
} | tee "$result"
