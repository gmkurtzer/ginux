#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tools/benchmarks/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

linux_summary="$benchmark_dir/rocky-linux-summary.tsv"
ginux_summary="$benchmark_dir/ginux-boot-summary.tsv"
output="$benchmark_dir/boot-comparison.tsv"

for input in "$linux_summary" "$ginux_summary"; do
    [[ -s "$input" ]] || {
        echo "benchmark: missing boot results: $input" >&2
        exit 1
    }
done

summarize() {
    local label=$1
    local input=$2
    awk -F '\t' -v label="$label" '
        NR > 1 { values[++count] = $2; sum += $2 }
        END {
            if (count == 0) exit 1
            for (i = 1; i <= count; i++)
                for (j = i + 1; j <= count; j++)
                    if (values[i] > values[j]) {
                        tmp = values[i]; values[i] = values[j]; values[j] = tmp
                    }
            if (count % 2) median = values[(count + 1) / 2]
            else median = (values[count / 2] + values[count / 2 + 1]) / 2
            printf "%s\t%d\t%.0f\t%d\t%d\t%.1f\n", label, count, median,
                values[1], values[count], sum / count
        }
    ' "$input"
}

printf 'system\truns\tmedian_ms\tmin_ms\tmax_ms\tmean_ms\n' >"$output"
summarize Linux "$linux_summary" >>"$output"
summarize Ginux "$ginux_summary" >>"$output"

cat "$output"
