# Detached-domain scheduler regressions

Run on a Linux/KVM test host with the repository's Rust/QEMU prerequisites.
Every harness copies the supplied seed volume, ESP and firmware variables,
and terminates only its own QEMU
process group. It does not forward host ports. Use unique output directories.
The Tamm commands and accepted image hashes are in `docs/scheduler/REVIEW.md`.

## Host checks

```
make check-scheduler
cargo test --package ginux-kernel-policy
```

The host harness includes the real native scheduler, with platform/domain stubs.
It exercises both overflow boundaries and 1024 controller lifecycle handoffs.

## Payloads

From a new payload build directory, with `source` naming this repository:

```
gcc -O2 -static-pie "$source/tests/scheduler/spin.c" -o spin
gcc -O2 -static-pie -DCOOPERATIVE "$source/tests/scheduler/spin.c" -o cooperative
for n in 1 2 4; do
    for suffix in a b; do
        seed=4951
        test "$suffix" != b || seed=27368
        gcc -O2 -Werror -static-pie -pthread -DWORKERS="$n" -DSEED="$seed" \
            "$source/tests/scheduler/context.c" "$source/tests/scheduler/context.S" \
            -o "context$n$suffix"
    done
done
for suffix in a b; do
    seed=4951
    test "$suffix" != b || seed=27368
    gcc -O2 -Werror -static-pie -DSEED="$seed" \
        "$source/tests/scheduler/signal_wait.c" "$source/tests/scheduler/context.S" \
        -o "signal1$suffix"
done
python3 "$source/tests/scheduler/make_layer.py" spin cooperative context1a context1b \
    context2a context2b context4a context4b signal1a signal1b
```

Import the resulting USTAR layer as `docker.io/library/alpine:latest` into an
**isolated** cached volume. On Tamm the existing rig's `volcraft` accepts:

```
volcraft volume.raw docker.io/library/alpine:latest layer-ustar.tar.gz
```

This private cache reference is a test fixture, not an upstream Alpine image.
Pin the layer, binaries and EFI hashes for comparisons. Do not modify a volume
owned by another VM. The harness uses KVM; set `QEMU_SYSTEM_X86_64`, `OVMF_CODE`
and `OVMF_VARS` for the local rig when necessary.

## Integration commands

```
python3 tests/scheduler/probe_responsiveness.py --source "$source" \
    --volume "$seed_volume" --output "$new_output_dir" --init spin \
    --cpus 1 --samples 30 --lifecycle --remove-volume
python3 tests/scheduler/probe_context.py --source "$source" \
    --volume "$seed_volume" --output "$new_output_dir" --cpus 4 --quiet-seconds 30
python3 tests/scheduler/probe_context.py --source "$source" \
    --volume "$seed_volume" --output "$new_output_dir" --cpus 1 \
    --prefix signal --quiet-seconds 15
```

Use a fresh output directory for each invocation. Add `--workers 4 --cpus 1`
to the context harness (with a 75-second quiet interval) to exercise sibling
preemption inside both competing domains. Repeat the spinner/context
runs on 1/2/4 CPUs. Use `--init context4a --cpus 4 --samples 1 --lifecycle` to
stop/restart a domain while its workers are still computing. The normal context
run starts two detached domains, sends no input during the quiet interval, then
requires both independent sentinel reports and clean process exits. CPU masks
come from hardware CPUID, not Ginux's pre-existing `getcpu()` stub.

The assembly tests all GPRs, CF/DF, stack/red-zone sentinels, FS/TLS, x87 control
and value, MXCSR, and three YMM registers. Distinct domain/thread seeds expose
state leakage. The signal fixture runs the same computation inside SIGUSR1,
then requires five successful timed sleeps and a clean console resume.

The spinner response check rejects stopped/failed domains, kernel panic and
controller restarts. Polling resolution is 50 ms; its response times are not a
microbenchmark of interrupt latency. First-query latency includes init startup.
