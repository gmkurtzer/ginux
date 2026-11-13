#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

log=build/gate6-devices.log

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/instance-gate6.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate6.toml"

rocky_ssh 'chmod 0755 /var/tmp/ginux; dd if=/dev/zero of=/var/tmp/ginux-block.img bs=4096 count=1 status=none; /var/tmp/ginux devices /var/tmp/gate6.toml /var/tmp/ginux-block.img' > "$log" 2>&1

grep -Fqx 'GINUX_DEVICES: visible=console,null,zero,random,block0,net0 unassigned=hidden' "$log"
grep -Fqx 'GINUX_DEVICES: handles=virtual pseudo=PASS ioctl-unknown=denied mmap=denied' "$log"
grep -Fqx 'GINUX_BLOCK: backend=file partition=1024:2048 rw=PASS flush=PASS bounds=denied' "$log"
grep -Fqx 'GINUX_NETWORK: backend=isolated-queue send=PASS receive=PASS control=denied host-fd=none' "$log"
grep -Fqx 'GINUX_DEVICES: conflict=denied teardown=flushed,released,reusable' "$log"
grep -Fqx 'GATE6_DEVICES: PASS' "$log"

block_value=$(rocky_ssh 'dd if=/var/tmp/ginux-block.img bs=1 skip=1024 count=11 status=none')
test "$block_value" = 'GINUX-BLOCK'
# The image must keep its size, or the dd reads below check nothing.
test "$(rocky_ssh 'stat -c %s /var/tmp/ginux-block.img')" = 4096
# The partition is bytes 1024..3071; both sides must stay zero. The bounds
# probe writes at partition offset 2047, so a leak would land past 3071.
outside=$(rocky_ssh 'dd if=/var/tmp/ginux-block.img bs=1 count=1024 status=none | od -An -v -tu1 | tr -d " 0\n"')
test -z "$outside"
outside=$(rocky_ssh 'dd if=/var/tmp/ginux-block.img bs=1 skip=3072 count=1024 status=none | od -An -v -tu1 | tr -d " 0\n"')
test -z "$outside"

# A backing file that does not cover the partition (1024 + 2048 bytes) must
# fail the gate before anything is written to it.
for size in 0 1024 3071; do
    small_log="build/gate6-devices-small-$size.log"
    if rocky_ssh "truncate -s $size /var/tmp/ginux-small.img; /var/tmp/ginux devices /var/tmp/gate6.toml /var/tmp/ginux-small.img" > "$small_log" 2>&1; then
        printf 'test-devices: %s-byte block backend accepted\n' "$size" >&2
        exit 1
    fi
    grep -Fqx 'GATE6_DEVICES: FAIL: block backend smaller than partition' "$small_log"
    if grep -Fqx 'GATE6_DEVICES: PASS' "$small_log"; then
        printf 'test-devices: %s-byte block backend reported PASS\n' "$size" >&2
        exit 1
    fi
    small_size=$(rocky_ssh 'stat -c %s /var/tmp/ginux-small.img')
    test "$small_size" = "$size"
done

rocky_ssh 'rm -f /var/tmp/ginux /var/tmp/gate6.toml /var/tmp/ginux-block.img /var/tmp/ginux-small.img'
printf '%s\n' 'test-devices: PASS'
