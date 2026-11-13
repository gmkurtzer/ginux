#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

instance_dir=/var/lib/ginux/instances/gate9
audit_log=$instance_dir/audit.log
runtime=/usr/local/libexec/ginux-runtime
helper=/usr/local/libexec/ginux-instance-helper

GINUX_GATE9_DNF=off \
GINUX_GATE9_NETWORK=off \
GINUX_GATE9_CONFIG=config/instance-gate9.75-hooks.toml \
    ./scripts/stage-systemd-instance.sh >/dev/null

rocky_ssh "sudo install -m 755 /var/tmp/ginux '$runtime'; sudo install -m 755 /var/tmp/ginux-host-instance-helper '$helper'; sudo install -m 644 /var/tmp/gate9.toml '$instance_dir/config.toml'"

run_case() {
    case_name=$1
    config=$2
    expected=$3
    rocky_scp "$config" "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-hook-case.toml"
    rocky_ssh "sudo install -m 644 /var/tmp/ginux-hook-case.toml '$instance_dir/config.toml'; sudo rm -f '$instance_dir/root/run/ginux-init-ran' '$instance_dir/root/run/ginux-timeout-init-ran' '$instance_dir/root/run/ginux-hook-continued'; sudo sh -c ': > \"$audit_log\"'; set +e; sudo '$helper' --protocol 1 run-runtime gate9 false >/var/tmp/ginux-hook-$case_name.console 2>/var/tmp/ginux-hook-$case_name.stderr; status=\$?; set -e; printf '%s\n' \$status >/var/tmp/ginux-hook-$case_name.status; test \$status -eq $expected"
}

run_case success config/instance-gate9.75-hooks.toml 0
rocky_ssh "test \"\$(sudo cat '$instance_dir/root/run/ginux-hook-order')\" = onetwo; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=start index=0 context=instance mediated=true uid=0 gid=0' '$audit_log'; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=complete index=1 result=pass' '$audit_log'; sudo grep -Fq 'surface=virtual-mount' '$audit_log'"

run_case stop config/instance-gate9.75-hook-stop.toml 1
rocky_ssh "test ! -e '$instance_dir/root/run/ginux-init-ran'; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=complete index=0 result=failed status=1' '$audit_log'"

run_case continue config/instance-gate9.75-hook-continue.toml 0
rocky_ssh "test -e '$instance_dir/root/run/ginux-hook-continued'; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=complete index=0 result=failed status=1' '$audit_log'; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=complete index=1 result=pass' '$audit_log'"

run_case timeout config/instance-gate9.75-hook-timeout.toml 124
rocky_ssh "test ! -e '$instance_dir/root/run/ginux-timeout-init-ran'; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=complete index=0 result=timeout status=124' '$audit_log'"

printf '%s\n' 'test-gate975-hooks: PASS (order, mediation, stop, continue, timeout, no-init-on-failure)'
