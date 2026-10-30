# Gate 10 Qualification Environment

- Commit: `94db6cccf3130bacc5c22e14724ab4385782b6e1`
- Host: Apple Silicon (`arm64`), macOS 26.6.2 (25G83)
- Emulator: QEMU 10.1.2, x86-64 TCG
- Machine: `q35`
- CPU model: `max,+smep,+smap,+umip`
- Guest memory: 2 GiB
- Published baseline: `docs/paper/artifact/benchmark-2026-10-23`
- Sysbench binary SHA-256:
  `7cf6b71521436bd32a8ab197851035ff1632bee383fec5a35468d61969e7c18d`

The single-CPU comparison used one vCPU and one sysbench thread. Scaling used
the same root filesystem and binary with one vCPU/one thread and two vCPUs/two
threads. Each workload/topology has nine fresh samples. Boot has one warm-up
followed by three measured samples.

The OCI cache and Rocky package tests required outbound network access. Their
initial sandbox failures are retained as diagnostics; packet capture showed
valid guest DNS traffic followed by an unreachable response from the restricted
QEMU backend. The exact frozen tests passed with network permission.
