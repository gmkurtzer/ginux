#!/bin/sh
set -eu

. ./scripts/gate16-host-env.sh
. ./scripts/rocky-host-common.sh

if rocky_ssh true >/dev/null 2>&1; then
    printf '%s\n' 'Gate 16 Rocky host already reachable; reusing it'
else
    ./scripts/start-rocky-host.sh
fi
rocky_ssh 'rm -rf /home/ginux/ginux-control'
rocky_scp -r kernel/ginux-control ginux@127.0.0.1:/home/ginux/
rocky_scp scripts/gate16-kernel-assist-guest.sh \
    ginux@127.0.0.1:/home/ginux/
rocky_ssh 'chmod +x /home/ginux/gate16-kernel-assist-guest.sh && \
    /home/ginux/gate16-kernel-assist-guest.sh /home/ginux/ginux-control'
rm -rf build/gate16-results
rocky_scp -r ginux@127.0.0.1:/home/ginux/gate16-results build/
printf '%s\n' 'Gate 16 raw benchmark samples: build/gate16-results/'
