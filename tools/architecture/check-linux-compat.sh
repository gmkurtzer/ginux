#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
matrix="${repo_root}/docs/LINUX-COMPATIBILITY.md"
dispatcher="${repo_root}/kernel/src/arch/x86_64/cpu.rs"

awk '
function fail(message) {
    print "Linux compatibility check: " message > "/dev/stderr"
    failed = 1
}
function trim(value) {
    gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
    return value
}
FNR == NR {
    if ($0 ~ /^Current totals:/) {
        totals = $0
        gsub(/[^0-9]+/, " ", totals)
        totals = trim(totals)
        declared_count = split(totals, declared, /[[:space:]]+/)
    }
    if ($0 !~ /^\|[[:space:]]*[0-9]+[[:space:]]*\|/) {
        next
    }
    split($0, columns, "|")
    number = trim(columns[2]) + 0
    status = trim(columns[4])
    if (number in status_by_number) {
        fail("duplicate syscall number " number)
    }
    if (have_previous && number <= previous) {
        fail("syscall " number " is out of order")
    }
    if (status != "Complete" && status != "Partial" && status != "Policy-denied" && status != "Missing") {
        fail("invalid status \047" status "\047 for syscall " number)
    }
    status_by_number[number] = status
    count_by_status[status]++
    previous = number
    have_previous = 1
    next
}
$0 ~ /^const LINUX_[A-Z0-9_]+_SYSCALL: u64 = [0-9]+;$/ {
    constant = $2
    sub(/:$/, "", constant)
    number = $5
    sub(/;$/, "", number)
    if (!(number in status_by_number)) {
        fail(constant " (" number ") is absent from the matrix")
    } else if (status_by_number[number] == "Missing") {
        fail(constant " (" number ") is implemented but marked Missing")
    }
}
END {
    if (declared_count != 4 \
        || declared[1] != count_by_status["Complete"] \
        || declared[2] != count_by_status["Partial"] \
        || declared[3] != count_by_status["Policy-denied"] \
        || declared[4] != count_by_status["Missing"]) {
        fail("declared syscall totals do not match the matrix")
    }
    if (failed) {
        exit 1
    }
    print "Linux compatibility matrix check passed (" count_by_status["Complete"] \
        " complete, " count_by_status["Partial"] " partial, " \
        count_by_status["Policy-denied"] " policy-denied, " \
        count_by_status["Missing"] " missing)"
}
' "$matrix" "$dispatcher"
