# Gate 16 kernel-assisted microbenchmarks

These samples compare native execution with the bounded Gate 16 module and BPF
LSM prototypes inside the same four-vCPU Rocky Linux QEMU host. They are not a
replacement for Gate 15's full native/selective/paranoid application matrix.

Run:

```sh
make gate16-prepare
make test-gate16-kernel-assist
```

The suite uses five repetitions and rotates native, module, and BPF-LSM order
within each repetition so every assisted sample has a same-repetition control.
It measures:

- an unrelated hot `sched_yield`, proving that allowed calls avoid userspace;
- allowed `mkdirat` create/remove pairs; and
- denied raw `mkdirat` calls through seccomp, the module, and BPF LSM.

Each TSV is `repetition<TAB>events-per-second`. `results/provenance.txt` pins
the guest kernel, architecture, vCPU count, Landlock ABI, and prototype object
digests. Generated copies are also left in `build/gate16-results/`.

The checked-in 2026-11-10 run produced these means (higher is better):

| Workload | Native/control | Module | BPF LSM |
|---|---:|---:|---:|
| Unrelated `sched_yield` calls/s | 781,505 | 779,408 (0.997x) | 784,486 (1.004x) |
| Allowed `mkdirat` pairs/s | 7,960 | 7,962 (1.000x) | 7,968 (1.001x) |
| Denied `mkdirat` calls/s | 2,176,029 (seccomp) | 1,605,322 | 320,205 |
