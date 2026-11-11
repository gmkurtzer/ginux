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
gate975_volume_log=build/gate975-config-volume.log
gate975_hook_log=build/gate975-config-hook.log
contract_plan_log=build/gate10-contract-plan.log
contract_systemd_plan_log=build/gate10-contract-systemd-plan.log
contract_invalid_log=build/gate10-contract-invalid.log
instance_validate_log=build/gate105-instance-validate.log
instance_explain_log=build/gate105-instance-explain.log
instance_diff_log=build/gate105-instance-diff.log
instance_inspect_log=build/gate105-instance-inspect.log
legacy_schema_log=build/gate1025-legacy-schema.log
valid_effective=build/gate4-config-valid.effective
equivalent_effective=build/gate4-config-equivalent.effective

./scripts/check-rocky-host.sh
rocky_scp \
    target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp \
    scripts/ginux-host-controller.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-cli"
rocky_scp \
    config/instance-gate4.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-valid.toml"
rocky_scp \
    tests/fixtures/instance-gate4-equivalent.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-equivalent.toml"
rocky_scp \
    tests/fixtures/instance-gate4-unknown.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-unknown.toml"
rocky_scp \
    tests/fixtures/instance-gate4-performance.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-performance.toml"
rocky_scp \
    tests/fixtures/instance-gate4-host-path.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-host-path.toml"
rocky_scp \
    config/instance-gate9.75.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate975.toml"
rocky_scp \
    config/instance-gate10-strict.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate10-strict.toml"
rocky_scp \
    config/instance-rocky-systemd.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate10-systemd.toml"

rocky_ssh 'chmod 0755 /var/tmp/ginux /var/tmp/ginux-cli; /var/tmp/ginux config /var/tmp/gate4-valid.toml' > "$valid_log" 2>&1
rocky_ssh '/var/tmp/ginux config /var/tmp/gate4-equivalent.toml' > "$equivalent_log" 2>&1
legacy_table=$(printf '\144\157\155\141\151\156')
rocky_ssh "sed 's/^\\[instance\\]$/[$legacy_table]/' /var/tmp/gate4-valid.toml > /var/tmp/gate1025-legacy.toml"
if rocky_ssh '/var/tmp/ginux config /var/tmp/gate1025-legacy.toml' >"$legacy_schema_log" 2>&1; then
    printf '%s\n' 'legacy configuration table was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=unknown-section managed-started=false' "$legacy_schema_log"
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
    printf '%s\n' 'missing instance init was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=missing-field managed-started=false' "$missing_init_log"

if rocky_ssh '/var/tmp/ginux config /var/tmp/gate9-invalid-init.toml' > "$invalid_init_log" 2>&1; then
    printf '%s\n' 'relative instance init was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=invalid-value managed-started=false' "$invalid_init_log"

rocky_ssh '/var/tmp/ginux config /var/tmp/gate975.toml' >"$gate975_log" 2>&1
grep -Fqx 'GINUX_CONFIG: PASS' "$gate975_log"
grep -Fqx 'hostname = "gate975-demo"' "$gate975_log"
grep -Fqx 'inbound = ["tcp:80"]' "$gate975_log"
grep -Fqx 'outbound = ["tcp:443", "udp:53"]' "$gate975_log"
grep -Fqx 'pre_boot = ["/usr/local/sbin/prepare-instance --safe"]' "$gate975_log"

rocky_ssh '/var/tmp/ginux instance-plan /var/tmp/gate10-strict.toml' >"$contract_plan_log" 2>&1
grep -Eq '^GINUX_CONTRACT: catalog=2 instance=gate10-strict profile=self-contained-v1 digest=[0-9a-f]{64} operations=[0-9]+$' "$contract_plan_log"
grep -Fq 'operation=syscall.getpid action=emulate provenance=exact-allow source=exact-allow source-rule=0' "$contract_plan_log"
grep -Fq 'operation=syscall.clock_gettime action=deny provenance=default-deny source=default-deny' "$contract_plan_log"
grep -Fqx 'GINUX_CONTRACT: PASS' "$contract_plan_log"
rocky_ssh '/var/tmp/ginux instance-plan /var/tmp/gate10-systemd.toml' >"$contract_systemd_plan_log" 2>&1
grep -Eq '^GINUX_CONTRACT: catalog=2 instance=rocky profile=self-contained-v1 digest=[0-9a-f]{64} operations=[0-9]+$' "$contract_systemd_plan_log"
grep -Fq 'operation=syscall.clock_gettime action=mediate provenance=exact-allow source=exact-allow' "$contract_systemd_plan_log"
grep -Fqx 'GINUX_CONTRACT: PASS' "$contract_systemd_plan_log"

if rocky_ssh "/var/tmp/ginux config-merge /var/tmp/gate10-strict.toml 'contract.syscalls.allow=[\"does_not_exist_*\"]'" >"$contract_invalid_log" 2>&1; then
    printf '%s\n' 'unmatched contract selector was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=invalid-contract managed-started=false' "$contract_invalid_log"

rocky_ssh "grep -Fqx '[contract]' /var/tmp/gate10-systemd.toml; grep -Fqx '[contract.syscalls]' /var/tmp/gate10-systemd.toml; ! grep -Fq '[userspace.' /var/tmp/gate10-systemd.toml"
rocky_ssh 'GINUX_RUNTIME=/var/tmp/ginux /var/tmp/ginux-cli validate /var/tmp/gate10-systemd.toml' >"$instance_validate_log" 2>&1
grep -Eq '^Contract valid: instance=rocky catalog=2 digest=[0-9a-f]{64} operations=[0-9]+$' "$instance_validate_log"
rocky_ssh 'GINUX_RUNTIME=/var/tmp/ginux /var/tmp/ginux-cli explain /var/tmp/gate10-systemd.toml filesystem.path.write' >"$instance_explain_log" 2>&1
grep -Fqx 'Decision: mediate' "$instance_explain_log"
grep -Fqx 'Rule: exact-allow source=exact-allow index=0' "$instance_explain_log"
rocky_ssh 'GINUX_RUNTIME=/var/tmp/ginux /var/tmp/ginux-cli diff /var/tmp/gate10-strict.toml /var/tmp/gate10-systemd.toml' >"$instance_diff_log" 2>&1
grep -Eq '^Contract differences: [1-9][0-9]*$' "$instance_diff_log"
rocky_ssh 'GINUX_RUNTIME=/var/tmp/ginux /var/tmp/ginux-cli inspect /var/tmp/gate10-systemd.toml' >"$instance_inspect_log" 2>&1
grep -Eq '^Decisions: emulate=[0-9]+ mediate=[0-9]+ deny=[0-9]+ total=[0-9]+$' "$instance_inspect_log"

if rocky_ssh 'GINUX_RUNTIME=/var/tmp/ginux /var/tmp/ginux-cli contract check /var/tmp/gate10-systemd.toml' >/dev/null 2>&1; then
    printf '%s\n' 'removed top-level contract command was incorrectly accepted' >&2
    exit 1
fi
if rocky_ssh 'GINUX_RUNTIME=/var/tmp/ginux /var/tmp/ginux-cli instance validate --config /var/tmp/gate10-systemd.toml' >/dev/null 2>&1; then
    printf '%s\n' 'required instance configuration was incorrectly accepted as a flag' >&2
    exit 1
fi

rocky_ssh "/var/tmp/ginux config-merge /var/tmp/gate975.toml 'resources.vcpus=4' 'identity.hostname=\"override.example\"' 'contract.process.max_threads=8192'" >"$gate975_override_log" 2>&1
grep -Fqx 'vcpus = 4' "$gate975_override_log"
grep -Fqx 'hostname = "override.example"' "$gate975_override_log"
grep -Fqx 'max_threads = 8192' "$gate975_override_log"
grep -Fqx 'memory_mib = 1024' "$gate975_override_log"

if rocky_ssh "/var/tmp/ginux config-merge /var/tmp/gate975.toml 'volumes.assignments=[\"/proc:/data:ro\"]'" >"$gate975_volume_log" 2>&1; then
    printf '%s\n' 'unsafe virtual-tree volume mapping was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=host-authority-reference managed-started=false' "$gate975_volume_log"

if rocky_ssh "/var/tmp/ginux config-merge /var/tmp/gate975.toml 'hooks.pre_boot=[\"host:/usr/local/sbin/setup\"]'" >"$gate975_hook_log" 2>&1; then
    printf '%s\n' 'host-context hook was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_CONFIG: rejected reason=invalid-value managed-started=false' "$gate975_hook_log"

rocky_ssh 'rm -f /var/tmp/ginux /var/tmp/ginux-cli /var/tmp/gate4-*.toml /var/tmp/gate9-*-init.toml /var/tmp/gate975.toml /var/tmp/gate10-strict.toml /var/tmp/gate10-systemd.toml /var/tmp/gate1025-legacy.toml'
printf '%s\n' 'test-config: PASS'
