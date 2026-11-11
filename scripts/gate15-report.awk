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

function print_mode_table(mode, label,    w, a, t, workload, vcpus, threads, row, native_key, measured_key, native_mean, measured_mean, performance, difference) {
    print "### " label
    print ""
    print "| Workload | vCPUs | Threads | Native mean | Native SD | Ginux mean | Ginux SD | Performance | Difference | Samples (native/Ginux) |"
    print "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
    for (w = 1; w <= workload_count; w++) {
        workload = workloads[w]
        for (a = 1; a <= allocation_count; a++) {
            vcpus = allocations[a]
            for (t = 1; t <= 2; t++) {
                threads = (t == 1 ? 1 : vcpus)
                if (t == 2 && vcpus == 1) continue
                row = workload SUBSEP vcpus SUBSEP threads
                native_key = "native" SUBSEP mode SUBSEP row
                measured_key = mode SUBSEP mode SUBSEP row
                native_mean = mean(native_key)
                measured_mean = mean(measured_key)
                performance = ratio(native_mean, measured_mean, direction[row])
                difference = (performance - 1) * 100
                printf "| %s | %d | %d | %.3f %s | %.3f | %.3f %s | %.3f | %.4fx | %+.2f%% | %d/%d |\n", \
                    workload, vcpus, threads, native_mean, unit[row], sd(native_key), \
                    measured_mean, unit[row], sd(measured_key), performance, difference, \
                    count[native_key], count[measured_key]
            }
        }
    }
    print ""
}

END {
    print_mode_table("selective", "Selective")
    print_mode_table("paranoid", "Paranoid")
}
