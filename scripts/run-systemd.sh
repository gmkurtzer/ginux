#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

if test -f "$ROCKY_PID_FILE"; then
    existing=$(cat "$ROCKY_PID_FILE")
    if kill -0 "$existing" 2>/dev/null; then
        printf 'A Rocky test host is already running with PID %s.\n' "$existing" >&2
        printf '%s\n' 'Stop it with ./scripts/stop-rocky-host.sh before starting the interactive demonstration.' >&2
        exit 1
    fi
fi

host_started=false
cleanup() {
    status=$?
    trap - 0 HUP INT TERM
    if test "$host_started" = true; then
        ./scripts/cleanup-systemd-domain.sh >/dev/null 2>&1 || true
        ./scripts/stop-rocky-host.sh || true
    fi
    exit "$status"
}
trap cleanup 0 HUP INT TERM

printf '%s\n' 'Starting the Rocky Linux QEMU host with shared external networking...'
host_started=true
ROCKY_NETWORK_RESTRICT=off ./scripts/start-rocky-host.sh
GINUX_GATE9_DNF=on GINUX_GATE9_NETWORK=on ./scripts/stage-systemd-domain.sh

printf '%s\n' ''
printf '%s\n' 'You are about to enter the Rocky Linux QEMU host as user ginux.'
printf '%s\n' 'Ginux is staged but NOT running. Start it yourself with:'
printf '%s\n' ''
printf '%s\n' '  sudo /var/tmp/ginux native /var/tmp/gate9.toml /var/lib/ginux/domains/gate9/root systemd 2>/var/tmp/ginux-systemd-audit.log'
printf '%s\n' ''
printf '%s\n' 'The attached terminal becomes the domain console.'
printf '%s\n' 'Domain login: root     Password: ginux'
printf '%s\n' 'Explore normally. To shut down the domain, run this as domain root:'
printf '%s\n' ''
printf '%s\n' '  kill -RTMIN+4 1'
printf '%s\n' ''
printf '%s\n' 'After shutdown, the complete Ginux audit remains on the Rocky host at:'
printf '%s\n' '  /var/tmp/ginux-systemd-audit.log'
printf '%s\n' ''
printf '%s\n' 'After Ginux returns you to the Rocky host, type exit to stop QEMU and clean up.'
printf '%s\n' 'GINUX_INTERACTIVE_HOST: READY'

rocky_ssh_tty "printf '\\nRocky QEMU host ready. Ginux has not been started.\\n\\n'; exec bash -l"
