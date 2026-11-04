#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

log=build/gate6-devices.log

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/domain-gate6.toml \
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
outside=$(rocky_ssh 'dd if=/var/tmp/ginux-block.img bs=1 count=1024 status=none | od -An -v -tu1 | tr -d " 0\n"')
test -z "$outside"

rocky_ssh 'rm -f /var/tmp/ginux /var/tmp/gate6.toml /var/tmp/ginux-block.img'
printf '%s\n' 'test-devices: PASS'
