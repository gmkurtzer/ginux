BEGIN {
    FS = "\t"
    workload_count = split(workload_list, workloads, ",")
    allocation_count = split(allocation_list, allocations, ",")
}

NR == 1 { next }

{
    key = $1 SUBSEP $2 SUBSEP $3 SUBSEP $4 SUBSEP $5
    count[key]++
    sum[key] += $9 + 0
    sumsq[key] += ($9 + 0) ^ 2
    direction[$3 SUBSEP $4 SUBSEP $5] = $7
    unit[$3 SUBSEP $4 SUBSEP $5] = $8
}

function mean(key) {
    return sum[key] / count[key]
}

function sd(key,    variance) {
    if (count[key] < 2) return 0
    variance = (sumsq[key] - (sum[key] ^ 2 / count[key])) / (count[key] - 1)
    if (variance < 0) variance = 0
    return sqrt(variance)
}

function ratio(control, measured, direction_value) {
    if (direction_value == "higher") return measured / control
    return control / measured
}

END {
    print "| Workload | vCPUs | Threads | Native (selective pair) | Selective | Ratio | Selective SD | Native (paranoid pair) | Paranoid | Ratio | Paranoid SD |"
    print "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
    for (w = 1; w <= workload_count; w++) {
        workload = workloads[w]
        for (a = 1; a <= allocation_count; a++) {
            vcpus = allocations[a]
            for (t = 1; t <= 2; t++) {
                threads = (t == 1 ? 1 : vcpus)
                if (t == 2 && vcpus == 1) continue
                row = workload SUBSEP vcpus SUBSEP threads
                ns = "native" SUBSEP "selective" SUBSEP row
                s = "selective" SUBSEP "selective" SUBSEP row
                np = "native" SUBSEP "paranoid" SUBSEP row
                p = "paranoid" SUBSEP "paranoid" SUBSEP row
                ns_mean = mean(ns)
                s_mean = mean(s)
                np_mean = mean(np)
                p_mean = mean(p)
                printf "| %s | %d | %d | %.3f %s | %.3f %s | %.4fx | %.3f | %.3f %s | %.3f %s | %.4fx | %.3f |\n", \
                    workload, vcpus, threads, ns_mean, unit[row], s_mean, unit[row], \
                    ratio(ns_mean, s_mean, direction[row]), sd(s), np_mean, unit[row], \
                    p_mean, unit[row], ratio(np_mean, p_mean, direction[row]), sd(p)
            }
        }
    }
}
