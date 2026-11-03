#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

valid_log=build/gate4-config-valid.log
equivalent_log=build/gate4-config-equivalent.log
unknown_log=build/gate4-config-unknown.log
performance_log=build/gate4-config-performance.log
host_path_log=build/gate4-config-host-path.log
valid_effective=build/gate4-config-valid.effective
equivalent_effective=build/gate4-config-equivalent.effective

./scripts/check-rocky-host.sh
rocky_scp \
    target/x86_64-unknown-linux-gnu/release/ginus \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus"
rocky_scp \
    config/domain-gate4.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-valid.toml"
rocky_scp \
    tests/fixtures/domain-gate4-equivalent.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-equivalent.toml"
rocky_scp \
    tests/fixtures/domain-gate4-unknown.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-unknown.toml"
rocky_scp \
    tests/fixtures/domain-gate4-performance.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-performance.toml"
rocky_scp \
    tests/fixtures/domain-gate4-host-path.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-host-path.toml"

rocky_ssh 'chmod 0755 /var/tmp/ginus; /var/tmp/ginus config /var/tmp/gate4-valid.toml' > "$valid_log" 2>&1
rocky_ssh '/var/tmp/ginus config /var/tmp/gate4-equivalent.toml' > "$equivalent_log" 2>&1
grep -Fqx 'GINUS_CONFIG: accepted format=toml schema=1' "$valid_log"
grep -Fqx 'GINUS_CONFIG: PASS' "$valid_log"
sed -n '/^GINUS_CONFIG_EFFECTIVE_BEGIN$/,/^GINUS_CONFIG_EFFECTIVE_END$/p' "$valid_log" > "$valid_effective"
sed -n '/^GINUS_CONFIG_EFFECTIVE_BEGIN$/,/^GINUS_CONFIG_EFFECTIVE_END$/p' "$equivalent_log" > "$equivalent_effective"
cmp "$valid_effective" "$equivalent_effective"

if rocky_ssh '/var/tmp/ginus config /var/tmp/gate4-unknown.toml' > "$unknown_log" 2>&1; then
    printf '%s\n' 'unknown configuration field was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUS_CONFIG: rejected reason=unknown-field managed-started=false' "$unknown_log"

if rocky_ssh '/var/tmp/ginus config /var/tmp/gate4-performance.toml' > "$performance_log" 2>&1; then
    printf '%s\n' 'unsupported performance backend was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUS_CONFIG: rejected reason=unsupported-execution-profile managed-started=false' "$performance_log"

if rocky_ssh '/var/tmp/ginus config /var/tmp/gate4-host-path.toml' > "$host_path_log" 2>&1; then
    printf '%s\n' 'host path authority was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUS_CONFIG: rejected reason=host-authority-reference managed-started=false' "$host_path_log"

rocky_ssh 'rm -f /var/tmp/ginus /var/tmp/gate4-*.toml'
printf '%s\n' 'test-config: PASS'
