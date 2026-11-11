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
    direction[$3 SUBSEP $4 SUBSEP $5] = $7
}

function mean(key) { return sum[key] / count[key] }
function ratio(control, measured, direction_value) {
    if (direction_value == "higher") return measured / control
    return control / measured
}
function bar_width(value) {
    capped = value
    if (capped > 1.2) capped = 1.2
    if (capped < 0) capped = 0
    return capped / 1.2 * 480
}

END {
    rows = workload_count * (allocation_count * 2 - 1)
    height = 100 + rows * 72
    print "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"900\" height=\"" height "\" viewBox=\"0 0 900 " height "\">"
    print "<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>"
    print "<style>text{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:13px;fill:#172033}.title{font-size:20px;font-weight:700}.axis{stroke:#9aa4b2;stroke-width:1}.native{fill:#7b8794}.selective{fill:#1677ff}.paranoid{fill:#8b5cf6}</style>"
    print "<text x=\"24\" y=\"30\" class=\"title\">Gate 15 native-relative performance</text>"
    print "<text x=\"24\" y=\"52\">Each Ginux bar is normalized to its paired native control (1.00x).</text>"
    print "<rect x=\"610\" y=\"20\" width=\"14\" height=\"10\" class=\"native\"/><text x=\"630\" y=\"30\">native</text>"
    print "<rect x=\"690\" y=\"20\" width=\"14\" height=\"10\" class=\"selective\"/><text x=\"710\" y=\"30\">selective</text>"
    print "<rect x=\"790\" y=\"20\" width=\"14\" height=\"10\" class=\"paranoid\"/><text x=\"810\" y=\"30\">paranoid</text>"
    print "<line x1=\"300\" y1=\"65\" x2=\"300\" y2=\"" height - 20 "\" class=\"axis\"/>"
    print "<line x1=\"700\" y1=\"65\" x2=\"700\" y2=\"" height - 20 "\" class=\"axis\"/>"
    print "<text x=\"292\" y=\"80\">0</text><text x=\"686\" y=\"80\">1.0x</text><text x=\"770\" y=\"80\">1.2x</text>"
    y = 96
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
                selective_ratio = ratio(mean(ns), mean(s), direction[row])
                paranoid_ratio = ratio(mean(np), mean(p), direction[row])
                print "<text x=\"24\" y=\"" y + 12 "\">" workload " v" vcpus " t" threads "</text>"
                print "<rect x=\"300\" y=\"" y "\" width=\"400\" height=\"10\" class=\"native\"/><text x=\"710\" y=\"" y + 10 "\">1.000x</text>"
                print "<rect x=\"300\" y=\"" y + 16 "\" width=\"" bar_width(selective_ratio) "\" height=\"10\" class=\"selective\"/><text x=\"790\" y=\"" y + 26 "\">" sprintf("%.3fx", selective_ratio) "</text>"
                print "<rect x=\"300\" y=\"" y + 32 "\" width=\"" bar_width(paranoid_ratio) "\" height=\"10\" class=\"paranoid\"/><text x=\"790\" y=\"" y + 42 "\">" sprintf("%.3fx", paranoid_ratio) "</text>"
                y += 72
            }
        }
    }
    print "</svg>"
}
