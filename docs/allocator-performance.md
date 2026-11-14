# Ginux allocator performance

This matched Rocky Linux 10 x86-64 QEMU/TCG run compares the native distribution allocator, Rocky packaged jemalloc, and the opt-in Ginux allocator. Each row contains five retained samples from the same guest. Results characterize this fixture only; they are not a universal allocator claim.

| Threads | Allocator | Samples | Mean ops/s | Stddev | Mean ns/op | Mean peak RSS KiB | Mean live RSS/requested | Versus native |
|---:|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | native | 5 | 1968651 | 304039 | 520.4 | 2241 | 1.116x | 1.000x |
| 1 | jemalloc | 5 | 8473412 | 2830211 | 140.6 | 12334 | 8.481x | 4.304x |
| 1 | ginux-guarded | 5 | 4523166 | 153229 | 221.3 | 2676 | 1.671x | 2.298x |
| 2 | native | 5 | 4085997 | 55352 | 244.8 | 2859 | 1.118x | 1.000x |
| 2 | jemalloc | 5 | 19306628 | 667798 | 51.8 | 16452 | 8.475x | 4.725x |
| 2 | ginux-guarded | 5 | 8785447 | 226306 | 113.9 | 3349 | 1.609x | 2.150x |
| 4 | native | 5 | 7206329 | 457260 | 139.2 | 4199 | 1.077x | 1.000x |
| 4 | jemalloc | 5 | 29859293 | 6875166 | 35.3 | 24676 | 8.427x | 4.143x |
| 4 | ginux-guarded | 5 | 12253638 | 3117770 | 86.4 | 4753 | 1.578x | 1.700x |

Raw samples and exact environment provenance are retained under `benchmarks/allocator/`. The benchmark keeps a touched 16-4096 byte working set while replacing allocations, and reports throughput, latency, peak RSS, and live-set RSS/requested as a bounded fragmentation proxy. It does not represent every application allocation pattern.
