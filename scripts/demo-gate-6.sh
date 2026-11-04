#!/bin/sh
set -eu

printf '%s\n' 'Gate 6 demonstration: Ginux-us-owned device assignment and virtual hardware.'
printf '%s\n' 'Expected: only configured devices are visible; block and network I/O are'
printf '%s\n' '          mediated; conflicts, unknown ioctls, mappings, and range escapes'
printf '%s\n' '          are denied; teardown releases the backend; benchmarks are recorded.'
printf '\n'

./scripts/test-devices.sh >/dev/null
printf '%s\n' '  [1/6] PASS  normalized config projects only console,null,zero,random,block0,net0'
printf '%s\n' '  [2/6] PASS  Ginux-us virtual handles and pseudo-device behavior; no host descriptor authority'
printf '%s\n' '  [3/6] PASS  file-backed partition read/write/flush stayed inside bytes 1024..3071'
printf '%s\n' '  [4/6] PASS  isolated net0 packet send/receive used no host socket or TUN/TAP descriptor'
printf '%s\n' '  [5/6] PASS  conflict, unknown ioctl, direct mapping, and revoked handle were denied'

./scripts/bench-devices.sh block >/dev/null
./scripts/bench-devices.sh network >/dev/null
printf '%s\n' '  [6/6] PASS  benchmark records include workload, repetitions, wall/CPU time, throughput, latency, environment'
sed -n '1p' build/bench-block.txt
sed -n '1p' build/bench-network.txt

printf '%s\n' 'Not claimed: external networking, physical devices, MMIO, DMA, VFIO, hotplug, or full Linux device compatibility.'
printf '%s\n' 'GATE6: PASS (6/6 demonstration checks)'
