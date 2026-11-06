#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

domain_dir=/var/lib/ginux/domains/gate9
runtime=/usr/local/libexec/ginux-runtime
helper=/usr/local/libexec/ginux-domain-helper

GINUX_GATE9_DNF=off \
GINUX_GATE9_NETWORK=off \
GINUX_GATE9_CONFIG=config/domain-gate9.75-hooks.toml \
    ./scripts/stage-systemd-domain.sh >/dev/null

rocky_ssh "sudo install -m 755 /var/tmp/ginux '$runtime'; sudo install -m 755 /var/tmp/ginux-host-domain-helper '$helper'; sudo install -m 644 /var/tmp/gate9.toml '$domain_dir/config.toml'"

run_case() {
    case_name=$1
    config=$2
    expected=$3
    rocky_scp "$config" "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-hook-case.toml"
    rocky_ssh "sudo install -m 644 /var/tmp/ginux-hook-case.toml '$domain_dir/config.toml'; sudo rm -f '$domain_dir/root/run/ginux-init-ran' '$domain_dir/root/run/ginux-timeout-init-ran' '$domain_dir/root/run/ginux-hook-continued'; set +e; sudo '$helper' run-runtime gate9 >/var/tmp/ginux-hook-$case_name.console 2>/var/tmp/ginux-hook-$case_name.audit; status=\$?; set -e; printf '%s\n' \$status >/var/tmp/ginux-hook-$case_name.status; test \$status -eq $expected"
}

run_case success config/domain-gate9.75-hooks.toml 0
rocky_ssh "test \"\$(sudo cat '$domain_dir/root/run/ginux-hook-order')\" = onetwo; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=start index=0 context=domain mediated=true uid=0 gid=0' /var/tmp/ginux-hook-success.audit; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=complete index=1 result=pass' /var/tmp/ginux-hook-success.audit; sudo grep -Fq 'surface=virtual-mount' /var/tmp/ginux-hook-success.audit"

run_case stop config/domain-gate9.75-hook-stop.toml 1
rocky_ssh "test ! -e '$domain_dir/root/run/ginux-init-ran'; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=complete index=0 result=failed status=1' /var/tmp/ginux-hook-stop.audit"

run_case continue config/domain-gate9.75-hook-continue.toml 0
rocky_ssh "test -e '$domain_dir/root/run/ginux-hook-continued'; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=complete index=0 result=failed status=1' /var/tmp/ginux-hook-continue.audit; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=complete index=1 result=pass' /var/tmp/ginux-hook-continue.audit"

run_case timeout config/domain-gate9.75-hook-timeout.toml 124
rocky_ssh "test ! -e '$domain_dir/root/run/ginux-timeout-init-ran'; sudo grep -Fq 'GINUX_HOOK_AUDIT: action=complete index=0 result=timeout status=124' /var/tmp/ginux-hook-timeout.audit"

printf '%s\n' 'test-gate975-hooks: PASS (order, mediation, stop, continue, timeout, no-init-on-failure)'
