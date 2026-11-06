#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

if test -f "$ROCKY_PID_FILE"; then
    existing=$(cat "$ROCKY_PID_FILE")
    if kill -0 "$existing" 2>/dev/null; then
        printf 'A Rocky test host is already running with PID %s.\n' "$existing" >&2
        printf '%s\n' 'Stop it with ./scripts/stop-rocky-host.sh before running make run-host.' >&2
        exit 1
    fi
fi

host_started=false
cleanup() {
    status=$?
    trap - 0 HUP INT TERM
    if test "$host_started" = true; then
        ./scripts/stop-rocky-host.sh || true
    fi
    exit "$status"
}
trap cleanup 0 HUP INT TERM

printf '%s\n' 'Starting the general Rocky Linux QEMU host...'
host_started=true
ROCKY_NETWORK_RESTRICT=off ./scripts/start-rocky-host.sh
./scripts/install-ginux-host.sh

printf '%s\n' ''
printf '%s\n' 'Rocky QEMU host ready. Ginux is installed; host cache and domains persist across runs.'
printf '%s\n' ''
printf '%s\n' 'Example workflow:'
printf '%s\n' '  ginux domain create rocky --config /etc/ginux/examples/rocky-systemd.toml'
printf '%s\n' '  ginux domain list'
printf '%s\n' '  ginux domain start rocky --attach'
printf '%s\n' '  ginux domain status rocky'
printf '%s\n' '  ginux domain stop rocky'
printf '%s\n' ''
printf '%s\n' 'Domain login: root     Password: ginux'
printf '%s\n' 'Inside an attached domain, press Ctrl-] to detach without stopping it.'
printf '%s\n' 'To power off the domain from its root shell, use: kill -RTMIN+4 1'
printf '%s\n' 'Exit the Rocky host shell to stop QEMU and clean up.'
printf '%s\n' 'GINUX_HOST: READY'

rocky_ssh_tty "exec bash -l"
