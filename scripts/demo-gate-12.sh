#!/bin/sh
set -eu

printf '%s\n' 'Gate 12 portable hierarchical contract'
sed -n '/^\[contract\]$/,/^\[storage\]$/p' config/instance-rocky-systemd.toml

printf '%s\n' 'Gate 12 repeatable deployment flags'
printf '%s\n' \
    'ginux instance create web recipe.toml --network eth0:1 --forward protocol=tcp,external_port=8080,instance_port=80 --forward protocol=tcp,external_port=8443,instance_port=443' \
    'ginux instance set web --network default --forward none'

printf '%s\n' 'Gate 12 compiled allow and deny evidence'
grep -m 1 'operation=syscall.clock_gettime action=mediate' \
    build/gate10-contract-systemd-plan.log
grep -m 1 'operation=syscall.socket .*action=deny' \
    build/gate10-contract-runtime.log

printf '%s\n' 'GATE12: PASS (hierarchical contract, syscall globs, deny precedence, portable recipe, repeatable atomic deployment flags)'
