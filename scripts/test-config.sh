#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

valid_log=build/gate4-config-valid.log
equivalent_log=build/gate4-config-equivalent.log
unknown_log=build/gate4-config-unknown.log
performance_log=build/gate4-config-performance.log
host_path_log=build/gate4-config-host-path.log
missing_init_log=build/gate9-config-missing-init.log
invalid_init_log=build/gate9-config-invalid-init.log
gate975_log=build/gate975-config.log
gate975_override_log=build/gate975-config-override.log
gate975_invalid_log=build/gate975-config-invalid.log
valid_effective=build/gate4-config-valid.effective
equivalent_effective=build/gate4-config-equivalent.effective

./scripts/check-rocky-host.sh
rocky_scp \
    target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
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
rocky_scp \
    config/domain-gate9.75.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate975.toml"

rocky_ssh 'chmod 0755 /var/tmp/ginux; /var/tmp/ginux config /var/tmp/gate4-valid.toml' > "$valid_log" 2>&1
rocky_ssh '/var/tmp/ginux config /var/tmp/gate4-equivalent.toml' > "$equivalent_log" 2>&1
rocky_ssh "sed '/^init =/d' /var/tmp/gate4-valid.toml > /var/tmp/gate9-missing-init.toml; sed 's#init = \"/runtime-fixture\"#init = \"relative-init\"#' /var/tmp/gate4-valid.toml > /var/tmp/gate9-invalid-init.toml"
grep -Fqx 'GINUX_CONFIG: accepted format=toml schema=1' "$valid_log"
grep -Fqx 'GINUX_CONFIG: PASS' "$valid_log"
sed -n '/^GINUX_CONFIG_EFFECTIVE_BEGIN$/,/^GINUX_CONFIG_EFFECTIVE_END$/p' "$valid_log" > "$valid_effective"
sed -n '/^GINUX_CONFIG_EFFECTIVE_BEGIN$/,/^GINUX_CONFIG_EFFECTIVE_END$/p' "$equivalent_log" > "$equivalent_effective"
grep -Fqx 'GINUX_CONFIG: accepted format=toml schema=1' "$equivalent_log"
grep -Fqx 'GINUX_CONFIG: PASS' "$equivalent_log"
# An absent effective-config block would make cmp compare two empty files.
for effective in "$valid_effective" "$equivalent_effective"; do
    grep -Fqx 'GINUX_CONFIG_EFFECTIVE_BEGIN' "$effective"
    grep -Fqx 'GINUX_CONFIG_EFFECTIVE_END' "$effective"
done
cmp "$valid_effective" "$equivalent_effective"

if rocky_ssh '/var/tmp/ginux config /var/tmp/gate4-unknown.toml' > "$unknown_log" 2>&1; then
    printf '%s\n' 'unknown configuration field was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=unknown-field managed-started=false' "$unknown_log"

if rocky_ssh '/var/tmp/ginux config /var/tmp/gate4-performance.toml' > "$performance_log" 2>&1; then
    printf '%s\n' 'unsupported performance backend was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=unsupported-execution-profile managed-started=false' "$performance_log"

if rocky_ssh '/var/tmp/ginux config /var/tmp/gate4-host-path.toml' > "$host_path_log" 2>&1; then
    printf '%s\n' 'host path authority was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=host-authority-reference managed-started=false' "$host_path_log"

if rocky_ssh '/var/tmp/ginux config /var/tmp/gate9-missing-init.toml' > "$missing_init_log" 2>&1; then
    printf '%s\n' 'missing domain init was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=missing-field managed-started=false' "$missing_init_log"

if rocky_ssh '/var/tmp/ginux config /var/tmp/gate9-invalid-init.toml' > "$invalid_init_log" 2>&1; then
    printf '%s\n' 'relative domain init was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=invalid-value managed-started=false' "$invalid_init_log"

rocky_ssh '/var/tmp/ginux config /var/tmp/gate975.toml' >"$gate975_log" 2>&1
grep -Fqx 'GINUX_CONFIG: PASS' "$gate975_log"
grep -Fqx 'hostname = "gate975-demo"' "$gate975_log"
grep -Fqx 'port_forwards = ["tcp/eth0:1/8080/80"]' "$gate975_log"
grep -Fqx 'pre_boot = ["/usr/local/sbin/prepare-domain --safe"]' "$gate975_log"

rocky_ssh "/var/tmp/ginux config-merge /var/tmp/gate975.toml 'resources.vcpus=4' 'identity.hostname=\"override.example\"' 'network.interface=\"eth1\"'" >"$gate975_override_log" 2>&1
grep -Fqx 'vcpus = 4' "$gate975_override_log"
grep -Fqx 'hostname = "override.example"' "$gate975_override_log"
grep -Fqx 'interface = "eth1"' "$gate975_override_log"
grep -Fqx 'memory_mib = 1024' "$gate975_override_log"

if rocky_ssh "/var/tmp/ginux config-merge /var/tmp/gate975.toml 'network.interface=\"bad interface\"'" >"$gate975_invalid_log" 2>&1; then
    printf '%s\n' 'invalid command-line override was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=invalid-value managed-started=false' "$gate975_invalid_log"

rocky_ssh 'rm -f /var/tmp/ginux /var/tmp/gate4-*.toml /var/tmp/gate9-*-init.toml /var/tmp/gate975.toml'
printf '%s\n' 'test-config: PASS'
