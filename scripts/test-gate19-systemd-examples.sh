#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

for entry in \
    'agent-workspace:config/examples/agent-workspace.toml' \
    'network-service:config/examples/network-service.toml' \
    'paranoid-shell:config/examples/paranoid-shell.toml'
do
    name=${entry%%:*}
    config=${entry#*:}
    log="build/gate19-$name-systemd.log"

    GINUX_GATE9_CONFIG=$config GINUX_GATE9_DNF=off GINUX_GATE9_NETWORK=off \
        GINUX_GATE9_EXTERNAL_NETWORK=off ./scripts/stage-systemd-instance.sh >/dev/null
    set +e
    ./scripts/expect-gate19-systemd-login.exp \
        "$ROCKY_KEY" "$ROCKY_SSH_PORT" "$ROCKY_SSH_USER" "$name" "$log" >/dev/null
    status=$?
    set -e
    ./scripts/cleanup-systemd-instance.sh >/dev/null

    test "$status" -eq 0
    grep -Fq 'Reached target multi-user.target - Multi-User System.' "$log"
    grep -Fq "GINUX_GATE19_LOGIN_OK:$name" "$log"
    printf 'Gate 19 %s systemd boot: PASS\n' "$name"
done

printf '%s\n' 'test-gate19-systemd-examples: PASS'
