#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

./scripts/install-ginux-host.sh >/dev/null
for config in \
    config/examples/agent-workspace.toml \
    config/examples/network-service.toml \
    config/examples/paranoid-shell.toml \
    config/examples/alpine-native.toml
do
    rocky_scp "$config" "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/$(basename "$config")"
done

run_example() {
    name=$1
    file=$2
    boot=$3
    check=$4
    shutdown=$5
    log=build/gate19-$name-native.log

    rocky_ssh "ginux stop '$name' >/dev/null 2>&1 || true"
    rocky_ssh "ginux import '$name' '/var/tmp/$file'" >/dev/null
    ./scripts/expect-gate19-native-login.exp \
        "$ROCKY_KEY" "$ROCKY_SSH_PORT" "$ROCKY_SSH_USER" \
        "$name" "$log" "$check" "$shutdown" >/dev/null
    grep -Fq "$boot" "$log"
    grep -Fq "GINUX_GATE19_BOOT_OK:$name" "$log"
    printf 'Gate 19 %s native boot: PASS\n' "$name"
}

run_example agent-workspace agent-workspace.toml \
    'Reached target multi-user.target - Multi-User System.' \
    "printf 'GINUX_GATE19_BOOT_OK:agent-workspace\\n'" \
    'kill -RTMIN+4 1; exit'
run_example ubuntu-cms network-service.toml \
    'Reached target multi-user.target - Multi-User System.' \
    "/usr/bin/python3 -c 'import urllib.request; assert b\"Blank CMS\" in urllib.request.urlopen(\"http://127.0.0.1:8080\").read()' && printf 'GINUX_GATE19_BOOT_OK:ubuntu-cms\\n'" \
    'kill -RTMIN+4 1; exit'
run_example paranoid-shell paranoid-shell.toml \
    'Reached target multi-user.target - Multi-User System.' \
    "printf 'GINUX_GATE19_BOOT_OK:paranoid-shell\\n'" \
    'kill -RTMIN+4 1; exit'
run_example alpine-native alpine-native.toml \
    'ginux-alpine login:' \
    "printf 'GINUX_GATE19_BOOT_OK:alpine-native\\n'" \
    'poweroff; exit'

printf '%s\n' 'test-gate19-native-examples: PASS'
