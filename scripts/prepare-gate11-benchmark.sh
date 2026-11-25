#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

./scripts/install-ginux-host.sh >/dev/null
rocky_scp config/instance-gate11-benchmark.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/instance-gate11-benchmark.toml"
rocky_scp scripts/gate11-guest-setup.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate11-guest-setup.sh"
rocky_ssh 'chmod 0755 /var/tmp/gate11-guest-setup.sh; GINUX_BENCH_ACQUIRE=on /var/tmp/gate11-guest-setup.sh'

printf '%s\n' 'Gate 11 benchmark dependencies and instance image are cached.'
