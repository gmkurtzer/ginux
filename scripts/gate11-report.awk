BEGIN {
    FS = "\t"
}

NR == 1 { next }

{
    key = $2 SUBSEP $3 SUBSEP $4
    side_key = $1 SUBSEP key
    count[side_key]++
    value[side_key, count[side_key]] = $8 + 0
    direction[key] = $6
    unit[key] = $7
    present[key] = 1
}

function median(side_key,    n, i, j, tmp, work) {
    n = count[side_key]
    for (i = 1; i <= n; i++) work[i] = value[side_key, i]
    for (i = 1; i <= n; i++) {
        for (j = i + 1; j <= n; j++) {
            if (work[j] < work[i]) {
                tmp = work[i]
                work[i] = work[j]
                work[j] = tmp
            }
        }
    }
    if (n % 2) return work[(n + 1) / 2]
    return (work[n / 2] + work[n / 2 + 1]) / 2
}

function cv(side_key,    n, i, mean, variance) {
    n = count[side_key]
    for (i = 1; i <= n; i++) mean += value[side_key, i]
    mean /= n
    if (n < 2 || mean == 0) return 0
    for (i = 1; i <= n; i++) variance += (value[side_key, i] - mean) ^ 2
    return sqrt(variance / (n - 1)) / mean * 100
}

END {
    print "| Workload | vCPUs | Threads | Native median | Ginux median | Performance | Difference | Native CV | Ginux CV | Budget |"
    print "|---|---:|---:|---:|---:|---:|---:|---:|---:|:---:|"
    for (workload_index = 1; workload_index <= 4; workload_index++) {
        workload = (workload_index == 1 ? "cpu" : workload_index == 2 ? "memory" : workload_index == 3 ? "threads" : "mutex")
        for (vcpus = 1; vcpus <= 4; vcpus *= 2) {
            for (thread_index = 1; thread_index <= 2; thread_index++) {
                threads = (thread_index == 1 ? 1 : vcpus)
                if (thread_index == 2 && vcpus == 1) continue
                key = workload SUBSEP vcpus SUBSEP threads
                if (!(key in present)) continue
                native_key = "native" SUBSEP key
                ginux_key = "ginux" SUBSEP key
                native = median(native_key)
                ginux = median(ginux_key)
                if (direction[key] == "higher") {
                    ratio = ginux / native
                    difference = (ratio - 1) * 100
                    pass = ginux >= native * 0.96
                } else {
                    ratio = native / ginux
                    difference = (ratio - 1) * 100
                    pass = ginux <= native * 1.04
                }
                printf "| %s | %d | %d | %.3f %s | %.3f %s | %.3fx | %+.2f%% | %.2f%% | %.2f%% | %s |\n", \
                    workload, vcpus, threads, native, unit[key], ginux, unit[key], ratio, difference, \
                    cv(native_key), cv(ginux_key), (pass ? "PASS" : "FAIL")
            }
        }
    }
}
