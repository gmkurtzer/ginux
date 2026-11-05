#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

kind=${1:?usage: bench-devices.sh block|network}
case "$kind" in
    block|network) ;;
    *) printf 'invalid benchmark kind: %s\n' "$kind" >&2; exit 2 ;;
esac

log="build/bench-$kind.txt"
./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/domain-gate6.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate6.toml"
rocky_ssh 'chmod 0755 /var/tmp/ginux; dd if=/dev/zero of=/var/tmp/ginux-bench-block.img bs=4096 count=1 status=none'
# Not piped into tee: without pipefail a failing bench would be masked.
rocky_ssh "/var/tmp/ginux device-bench /var/tmp/gate6.toml /var/tmp/ginux-bench-block.img $kind" > "$log" 2>&1 || {
    cat "$log" >&2
    exit 1
}
cat "$log"
grep -Eq "^GINUX_BENCH: kind=$kind workload=rw-pairs repetitions=1024 operations=2048 bytes=[0-9]+ wall_ns=[1-9][0-9]* cpu_ns=[1-9][0-9]* throughput_Bps=[1-9][0-9]* latency_ns_per_op=[0-9]+ environment=rocky10-x86_64-qemu-tcg$" "$log"
grep -Fqx 'GINUX_BENCH: PASS' "$log"
rocky_ssh 'rm -f /var/tmp/ginux /var/tmp/gate6.toml /var/tmp/ginux-bench-block.img'
