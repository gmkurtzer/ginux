BEGIN {
    FS = "\t"
    tolerance = tolerance + 0
}

FNR == 1 { next }

{
    key = $2 SUBSEP $3 SUBSEP $4
    if ($1 != "ginux") next
    if ($6 != "higher" && $6 != "lower") {
        printf "GATE11_REGRESSION: FAIL reason=unknown-direction direction=%s\n", $6
        invalid = 1
        next
    }
    if (FILENAME == accepted_file) {
        accepted_count[key]++
        accepted_value[key, accepted_count[key]] = $8 + 0
        accepted_direction[key] = $6
        accepted_label[key] = $2 " vcpus=" $3 " threads=" $4
        accepted_present[key] = 1
    } else {
        current_count[key]++
        current_value[key, current_count[key]] = $8 + 0
        current_direction[key] = $6
        current_label[key] = $2 " vcpus=" $3 " threads=" $4
        current_present[key] = 1
    }
}

function median(which, key,    n, i, j, tmp, work) {
    if (which == "accepted") {
        n = accepted_count[key]
        for (i = 1; i <= n; i++) work[i] = accepted_value[key, i]
    } else {
        n = current_count[key]
        for (i = 1; i <= n; i++) work[i] = current_value[key, i]
    }
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
    for (key in accepted_present) {
        rows++
        if (!(key in current_present)) {
            printf "GATE11_REGRESSION: FAIL row=%s reason=missing-current-row\n", accepted_label[key]
            failed = 1
            continue
        }
        if (accepted_direction[key] != current_direction[key]) {
            printf "GATE11_REGRESSION: FAIL row=%s reason=direction-changed\n", accepted_label[key]
            failed = 1
            continue
        }
        accepted = median("accepted", key)
        current = median("current", key)
        if (accepted_direction[key] == "higher") {
            ratio = current / accepted
        } else {
            ratio = accepted / current
        }
        if (ratio + 0.000000001 < 1 - tolerance) {
            printf "GATE11_REGRESSION: FAIL row=%s performance=%.6fx minimum=%.6fx\n", \
                accepted_label[key], ratio, 1 - tolerance
            failed = 1
        }
    }
    if (rows == 0) {
        print "GATE11_REGRESSION: FAIL reason=no-accepted-rows"
        failed = 1
    }
    for (key in current_present) {
        if (!(key in accepted_present)) {
            printf "GATE11_REGRESSION: FAIL row=%s reason=unaccepted-row\n", current_label[key]
            failed = 1
        }
    }
    if (failed) exit 1
    print "GATE11_REGRESSION: PASS"
}
