#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

./scripts/install-ginux-host.sh >/dev/null
rocky_scp config/instance-gate11-benchmark.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/instance-gate11-benchmark.toml"
rocky_scp scripts/gate11-guest-setup.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate11-guest-setup.sh"

rocky_ssh 'chmod 0755 /var/tmp/gate11-guest-setup.sh; /var/tmp/gate11-guest-setup.sh'
rocky_ssh 'sudo taskset -c 0 /usr/local/libexec/ginux-instance-helper --protocol 1 run-runtime gate11-benchmark false'
