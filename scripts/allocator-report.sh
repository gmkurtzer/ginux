#!/bin/sh
set -eu

input=${1:-benchmarks/allocator/rocky-qemu.tsv}
markdown=${2:-docs/allocator-performance.md}
svg=${3:-docs/allocator-performance.svg}
test -r "$input"

summary=$(mktemp "${TMPDIR:-/tmp}/ginux-allocator-summary.XXXXXX")
trap 'rm -f "$summary"' 0 HUP INT TERM

awk -F '\t' '
NR == 1 { next }
{
    key = $1 SUBSEP $2
    count[key]++
    sum[key] += $6
    square[key] += $6 * $6
    latency[key] += $7
    peak[key] += $8
    fragmentation[key] += $11
}
END {
    for (t = 1; t <= 4; t *= 2) {
        for (a = 1; a <= 2; a++) {
            name = a == 1 ? "native" : "ginux-guarded"
            key = name SUBSEP t
            n = count[key]
            if (n == 0) exit 2
            mean = sum[key] / n
            variance = n > 1 ? (square[key] - sum[key] * sum[key] / n) / (n - 1) : 0
            if (variance < 0) variance = 0
            printf "%s\t%d\t%d\t%.2f\t%.2f\t%.2f\t%.2f\t%.4f\n", name, t, n, mean, sqrt(variance), latency[key] / n, peak[key] / n, fragmentation[key] / n
        }
    }
}
' "$input" >"$summary"

{
    printf '%s\n\n' '# Ginux allocator performance'
    printf '%s\n\n' 'This matched Rocky Linux 10 x86-64 QEMU/TCG run compares the native distribution allocator with the opt-in Ginux allocator. Each row contains five retained samples from the same guest. Results characterize this fixture only; they are not a universal allocator claim.'
    printf '%s\n' '| Threads | Allocator | Samples | Mean ops/s | Stddev | Mean ns/op | Mean peak RSS KiB | Mean live RSS/requested | Versus native |'
    printf '%s\n' '|---:|---|---:|---:|---:|---:|---:|---:|---:|'
    for threads in 1 2 4; do
        native=$(awk -F '\t' -v t="$threads" '$1 == "native" && $2 == t { print $4 }' "$summary")
        while IFS="$(printf '\t')" read -r allocator current count mean deviation latency peak fragmentation; do
            test "$current" = "$threads" || continue
            ratio=$(awk -v value="$mean" -v control="$native" 'BEGIN { printf "%.3fx", value / control }')
            printf '| %s | %s | %s | %.0f | %.0f | %.1f | %.0f | %.3fx | %s |\n' \
                "$threads" "$allocator" "$count" "$mean" "$deviation" "$latency" "$peak" "$fragmentation" "$ratio"
        done <"$summary"
    done
    printf '\n%s\n' 'Raw samples and exact environment provenance are retained under `benchmarks/allocator/`. The benchmark keeps a touched 16-4096 byte working set while replacing allocations, and reports throughput, latency, peak RSS, and live-set RSS/requested as a bounded fragmentation proxy. It does not represent every application allocation pattern.'
} >"$markdown"

{
    printf '%s\n' '<svg xmlns="http://www.w3.org/2000/svg" width="760" height="300" viewBox="0 0 760 300" role="img" aria-labelledby="title desc">'
    printf '%s\n' '<title id="title">Ginux allocator throughput relative to native</title>'
    printf '%s\n' '<desc id="desc">Grouped bars compare native and Ginux allocator throughput at one, two, and four threads.</desc>'
    printf '%s\n' '<rect width="760" height="300" fill="#ffffff"/>'
    printf '%s\n' '<line x1="70" y1="240" x2="730" y2="240" stroke="#334155"/>'
    group=0
    for threads in 1 2 4; do
        native=$(awk -F '\t' -v t="$threads" '$1 == "native" && $2 == t { print $4 }' "$summary")
        guarded=$(awk -F '\t' -v t="$threads" '$1 == "ginux-guarded" && $2 == t { print $4 }' "$summary")
        ratio=$(awk -v value="$guarded" -v control="$native" 'BEGIN { printf "%.4f", value / control }')
        base=$((130 + group * 210))
        guarded_height=$(awk -v ratio="$ratio" 'BEGIN { h = 150 * ratio; if (h > 190) h = 190; printf "%d", h }')
        guarded_y=$((240 - guarded_height))
        printf '<rect x="%s" y="90" width="55" height="150" fill="#64748b"/>\n' "$base"
        printf '<rect x="%s" y="%s" width="55" height="%s" fill="#0f766e"/>\n' "$((base + 65))" "$guarded_y" "$guarded_height"
        printf '<text x="%s" y="265" text-anchor="middle" font-family="sans-serif" font-size="14">%s threads</text>\n' "$((base + 60))" "$threads"
        printf '<text x="%s" y="%s" text-anchor="middle" font-family="sans-serif" font-size="12">%.3fx</text>\n' "$((base + 92))" "$((guarded_y - 8))" "$ratio"
        group=$((group + 1))
    done
    printf '%s\n' '<rect x="70" y="20" width="18" height="18" fill="#64748b"/><text x="96" y="34" font-family="sans-serif" font-size="14">native</text>'
    printf '%s\n' '<rect x="180" y="20" width="18" height="18" fill="#0f766e"/><text x="206" y="34" font-family="sans-serif" font-size="14">ginux-guarded</text>'
    printf '%s\n' '</svg>'
} >"$svg"
