#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

case $ROCKY_OVERLAY_DISK in
    ''|/|.|..)
        printf 'refusing unsafe Rocky host overlay path: %s\n' \
            "$ROCKY_OVERLAY_DISK" >&2
        exit 2
        ;;
esac

./scripts/stop-rocky-host.sh
rm -f -- "$ROCKY_OVERLAY_DISK" "$ROCKY_PID_FILE" "$ROCKY_SERIAL_LOG"
printf 'Rocky host reset: removed persistent overlay %s\n' "$ROCKY_OVERLAY_DISK"
