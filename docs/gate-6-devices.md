# Gate 6 device demonstration

Run:

```sh
make demo-gate-6
```

The target boots the cached Rocky Linux 10 x86-64 host under QEMU TCG and
runs Ginus as an ordinary host process. Ginus reads
`config/domain-gate6.toml`, projects only its six assigned virtual devices,
and exercises the console, pseudo-device, bounded block-partition, and isolated
network frontends.

The disposable 4096-byte block backing is opened only by Ginus. The domain
view begins at byte 1024 and is 2048 bytes long. The demonstration checks a
successful write, read, flush, and close; rejects a write crossing the end;
and independently confirms bytes before the partition remain zero.

The `net0` backend is a bounded in-memory Ginus packet queue. It deliberately
has no external connectivity, host socket, or TUN/TAP descriptor. The demo
also rejects conflicting exclusive assignment, an unknown ioctl, a direct
mapping request, and use of a revoked virtual handle.

Benchmark records remain in `build/bench-block.txt` and
`build/bench-network.txt`. They characterize the emulated environment and do
not enforce a performance budget. All remote disposable files and the QEMU
process are removed when the target exits.
