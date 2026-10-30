# Gate 10 Raw and Reduced Evidence

The final machine-readable outcomes are `qualification.tsv`,
`single-cpu-results.tsv`, and `smp-scaling.tsv`. `environment.md` records the
matched environment and binary identity.

Files beginning with `final-` are the authoritative command logs. Raw final
boot and single-CPU sysbench samples are under `single-cpu-raw/`; matched raw
scaling samples are under `scaling-raw/`. Earlier attempts and diagnostics are
retained because they document regressions found and corrected during the
autonomous Gate 10 loop. In particular, the OCI diagnostics distinguish a
restricted-host network failure from a guest kernel defect.

Passing these tests qualifies only the listed hardware model, workloads, Linux
ABI surface, and the two reported hardening findings. It is not a general
security audit or a claim of complete Linux compatibility.
