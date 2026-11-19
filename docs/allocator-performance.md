# Ginux allocator performance

This matched Rocky Linux 10 x86-64 QEMU/TCG run compares the native distribution allocator, Rocky packaged jemalloc, and the opt-in Ginux allocator. Each row contains five retained samples from the same guest. Results characterize this fixture only; they are not a universal allocator claim.

| Threads | Allocator | Samples | Mean ops/s | Stddev | Mean ns/op | Mean peak RSS KiB | Mean live RSS/requested | Versus native |
|---:|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | native | 5 | 2062505 | 34889 | 485.0 | 2220 | 1.043x | 1.000x |
| 1 | jemalloc | 5 | 7925321 | 2371273 | 143.1 | 12338 | 8.507x | 3.843x |
| 1 | ginux-guarded | 5 | 1402030 | 25004 | 713.4 | 6469 | 1.786x | 0.680x |
| 2 | native | 5 | 3999126 | 34758 | 250.1 | 2866 | 1.080x | 1.000x |
| 2 | jemalloc | 5 | 16543941 | 1399531 | 60.8 | 16450 | 8.475x | 4.137x |
| 2 | ginux-guarded | 5 | 2658915 | 57578 | 376.2 | 11078 | 1.666x | 0.665x |
| 4 | native | 5 | 6982580 | 853561 | 145.3 | 4122 | 1.064x | 1.000x |
| 4 | jemalloc | 5 | 30972323 | 934879 | 32.3 | 25070 | 8.630x | 4.436x |
| 4 | ginux-guarded | 5 | 4002184 | 1316581 | 277.1 | 19107 | 1.580x | 0.573x |

Raw samples and exact environment provenance are retained under `benchmarks/allocator/`. The benchmark keeps a touched 16-4096 byte working set while replacing allocations, and reports throughput, latency, peak RSS, and live-set RSS/requested as a bounded fragmentation proxy. It does not represent every application allocation pattern.
