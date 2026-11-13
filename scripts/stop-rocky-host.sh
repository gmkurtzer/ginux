#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

pid=
if [ -f "$ROCKY_PID_FILE" ]; then
    pid=$(cat "$ROCKY_PID_FILE")
elif command -v lsof >/dev/null 2>&1; then
    pid=$(lsof -t "$ROCKY_OVERLAY_DISK" 2>/dev/null | sed -n '1p')
    if [ -z "$pid" ]; then
        candidate=$(lsof -nP -t -iTCP:"$ROCKY_SSH_PORT" -sTCP:LISTEN 2>/dev/null | sed -n '1p')
        case $ROCKY_OVERLAY_DISK in
            /*) overlay_path=$ROCKY_OVERLAY_DISK ;;
            *) overlay_path=$(pwd -P)/$ROCKY_OVERLAY_DISK ;;
        esac
        if [ -n "$candidate" ] &&
            lsof -nP -a -p "$candidate" -d txt 2>/dev/null | grep -q 'qemu-system-x86_64' &&
            lsof -nP -p "$candidate" 2>/dev/null | grep -Fq " $overlay_path"; then
            pid=$candidate
        fi
    fi
    if [ -n "$pid" ]; then
        printf 'Recovering Rocky host PID %s from the open overlay disk.\n' "$pid"
        mkdir -p "$(dirname "$ROCKY_PID_FILE")"
        printf '%s\n' "$pid" >"$ROCKY_PID_FILE"
    fi
fi

if [ -z "$pid" ]; then
    printf '%s\n' 'Rocky host already stopped'
    exit 0
fi

case "$pid" in
    *[!0-9]*|'')
        printf 'Invalid Rocky host PID file: %s\n' "$ROCKY_PID_FILE" >&2
        exit 1
        ;;
esac

if ! kill -0 "$pid" 2>/dev/null; then
    rm -f "$ROCKY_PID_FILE"
    printf '%s\n' 'Rocky host already stopped'
    exit 0
fi

rocky_ssh 'sudo systemctl poweroff' >/dev/null 2>&1 || true

attempt=0
while kill -0 "$pid" 2>/dev/null && [ "$attempt" -lt 30 ]; do
    attempt=$((attempt + 1))
    sleep 2
done
if kill -0 "$pid" 2>/dev/null; then
    kill "$pid"
fi

attempt=0
while kill -0 "$pid" 2>/dev/null && [ "$attempt" -lt 10 ]; do
    attempt=$((attempt + 1))
    sleep 1
done
if kill -0 "$pid" 2>/dev/null; then
    kill -KILL "$pid"
fi

attempt=0
while kill -0 "$pid" 2>/dev/null && [ "$attempt" -lt 5 ]; do
    attempt=$((attempt + 1))
    sleep 1
done
if kill -0 "$pid" 2>/dev/null; then
    printf 'Rocky host PID %s did not stop; preserving %s for recovery.\n' \
        "$pid" "$ROCKY_PID_FILE" >&2
    exit 1
fi

rm -f "$ROCKY_PID_FILE"
printf '%s\n' 'Rocky host stopped'
