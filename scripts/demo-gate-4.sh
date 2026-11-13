#!/bin/sh
set -eu

printf '%s\n' 'Gate 4 demonstration: versioned configuration and one-instance lifecycle.'
printf '%s\n' 'Expected: canonical TOML, security interpreter selection, deterministic'
printf '%s\n' '          lifecycle, fail-closed configuration, signal cleanup, and reaping.'
printf '%s\n' ''

./scripts/test-config.sh
./scripts/test-instance.sh

printf '%s\n' ''
printf '%s\n' '1. Valid configuration and canonical effective authority:'
grep -E '^(GINUX_CONFIG: accepted|schema_version|id =|root =|init =|vcpus =|memory_mib =|profile = "security"|backend = "interpreter"|GINUX_CONFIG: PASS)' build/gate4-config-valid.log
printf '%s\n' ''
printf '%s\n' '2. Equivalent TOML produced byte-identical effective configuration: PASS'
printf '%s\n' ''
printf '%s\n' '3. Unsupported or authority-granting inputs rejected before managed start:'
cat build/gate4-config-unknown.log
cat build/gate4-config-performance.log
cat build/gate4-config-host-path.log
printf '%s\n' ''
printf '%s\n' '4. Managed instance execution and natural cleanup:'
grep -E '^(GINUX_LIFECYCLE|GINUX_INSTANCE|GATE4:)' build/gate4-instance.log
printf '%s\n' ''
printf '%s\n' '5. Failed managed init cleanup:'
grep -E '^(GINUX_LIFECYCLE|GINUX_INSTANCE)' build/gate4-instance-failure.log
printf '%s\n' ''
printf '%s\n' '6. Host SIGTERM, helper reaping, and requested-stop cleanup:'
cat build/gate4-lifecycle.log
printf '%s\n' ''
printf '%s\n' 'Gate 4 does not yet provide dynamic linking, VFS, virtual devices, or systemd.'
printf '%s\n' 'GATE4: PASS (configuration and single-instance lifecycle profile)'
