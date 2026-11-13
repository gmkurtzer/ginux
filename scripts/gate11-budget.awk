BEGIN {
    FS = "\t"
    tolerance = tolerance + 0
}

NR == 1 { next }

{
    key = $2 SUBSEP $3 SUBSEP $4
    side_key = $1 SUBSEP key
    if ($1 != "native" && $1 != "ginux") {
        printf "GATE11_BUDGET: FAIL reason=unknown-side side=%s\n", $1
        invalid = 1
        next
    }
    if ($6 != "higher" && $6 != "lower") {
        printf "GATE11_BUDGET: FAIL reason=unknown-direction direction=%s\n", $6
        invalid = 1
        next
    }
    if ((key in direction) && direction[key] != $6) {
        printf "GATE11_BUDGET: FAIL row=%s reason=direction-changed\n", label[key]
        invalid = 1
        next
    }
    count[side_key]++
    value[side_key, count[side_key]] = $8 + 0
    direction[key] = $6
    label[key] = $2 " vcpus=" $3 " threads=" $4
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

END {
    failed = invalid
    rows = 0
    for (key in present) {
        rows++
        native_key = "native" SUBSEP key
        ginux_key = "ginux" SUBSEP key
        if (count[native_key] == 0 || count[ginux_key] == 0) {
            printf "GATE11_BUDGET: FAIL row=%s reason=missing-side\n", label[key]
            failed = 1
            continue
        }
        native = median(native_key)
        ginux = median(ginux_key)
        if (direction[key] == "higher") {
            ratio = ginux / native
        } else {
            ratio = native / ginux
        }
        if (ratio + 0.000000001 < 1 - tolerance) {
            printf "GATE11_BUDGET: FAIL row=%s performance=%.6fx minimum=%.6fx\n", \
                label[key], ratio, 1 - tolerance
            failed = 1
        }
    }
    if (rows == 0) {
        print "GATE11_BUDGET: FAIL reason=no-rows"
        failed = 1
    }
    if (failed) exit 1
    print "GATE11_BUDGET: PASS"
}
