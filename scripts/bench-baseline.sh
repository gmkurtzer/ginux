#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

raw=build/bench-baseline.log
samples=build/bench-baseline.csv
summary=build/bench-baseline-summary.csv

rocky_scp \
    target/x86_64-unknown-linux-gnu/release/baseline-runner \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/baseline-runner"
rocky_ssh 'chmod 0755 /var/tmp/baseline-runner; sudo /var/tmp/baseline-runner' > "$raw" 2>&1
grep -Fq 'BENCH: PASS' "$raw"

printf '%s\n' 'mode,workload,repetition,nanoseconds' > "$samples"
awk '
    /^BENCH mode=/ {
        split($2, mode, "=");
        split($3, workload, "=");
        split($4, repetition, "=");
        split($5, nanoseconds, "=");
        gsub(/\r/, "", nanoseconds[2]);
        print mode[2] "," workload[2] "," repetition[2] "," nanoseconds[2];
    }
' "$raw" >> "$samples"

[ "$(($(wc -l < "$samples") - 1))" -eq 30 ]

printf '%s\n' 'mode,workload,repetitions,mean_ns,variance_ns2,stddev_ns' > "$summary"
awk -F, '
    NR > 1 {
        key = $1 SUBSEP $2;
        count[key]++;
        sum[key] += $4;
        square[key] += $4 * $4;
        mode[key] = $1;
        workload[key] = $2;
    }
    END {
        for (key in count) {
            mean = sum[key] / count[key];
            variance = square[key] / count[key] - mean * mean;
            if (variance < 0) variance = 0;
            printf "%s,%s,%d,%.0f,%.0f,%.0f\n", mode[key], workload[key], count[key], mean, variance, sqrt(variance);
        }
    }
' "$samples" | LC_ALL=C sort >> "$summary"

grep '^BENCH_META ' "$raw" | tr -d '\r'
rocky_ssh '. /etc/os-release; printf "BENCH_META host=%s version=%s kernel=%s\\n" "$ID" "$VERSION_ID" "$(uname -r)"'
cat "$summary"
printf 'benchmark samples: %s\n' "$samples"
