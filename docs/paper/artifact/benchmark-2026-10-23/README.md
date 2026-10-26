# October 23, 2026 benchmark evidence

This directory preserves the surviving raw output behind the exploratory
single-vCPU results reported in the Ginux paper. It was added after the
evaluated code commit so that readers can inspect the measurements; the files
themselves were produced on October 23, 2026.

Contents:

- `boot-comparison.tsv`, `ginux-boot-summary.tsv`, and
  `rocky-linux-summary.tsv`: reported startup-path summaries.
- `ginux-boot-[1-3].log` and `.metrics`: Ginux systemd boot runs.
- `rocky-linux-run-[1-3].log`: Rocky Linux systemd boot runs.
- `rocky-linux-run-[1-3]-results.log`: Rocky Linux sysbench output.
- `ginux-sysbench-{cpu,memory,threads}-[1-9].log`: the three Ginux
  workloads retained in the paper.
- `sysbench-comparison.tsv`: final workload medians.
- `SHA256SUMS`: integrity digests for the archived evidence.

The omitted mutex logs completed only one event per run and are not used in the
revised paper. Exploratory optimization logs, generated VM images, OCI cache
volumes, firmware variable stores, and prepared rootfs archives are also
excluded. The reproducible procedures, pinned Linux image digest, and fixed
profile are documented in `docs/BENCHMARKING.md` and
`profiles/benchmarks/rocky10-single-vcpu.env`.

These logs improve inspectability but do not eliminate the experiment's stated
limitations: the host was Apple Silicon macOS using x86-64 QEMU TCG; exact host,
macOS, and QEMU version records were not retained; the boot profiles and full
sysbench execution environments were not equivalent; and the small samples do
not support general kernel-performance conclusions.
