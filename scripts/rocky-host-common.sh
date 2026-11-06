#!/bin/sh

ROCKY_SSH_PORT=${ROCKY_SSH_PORT:-22222}
ROCKY_SSH_USER=ginux
ROCKY_KEY=build/cache/rocky/gate1_ed25519
ROCKY_PID_FILE=build/rocky-host.pid
ROCKY_SERIAL_LOG=build/rocky-host-serial.log

rocky_ssh() {
    ssh \
        -i "$ROCKY_KEY" \
        -p "$ROCKY_SSH_PORT" \
        -o BatchMode=yes \
        -o ConnectTimeout=5 \
        -o IdentitiesOnly=yes \
        -o LogLevel=ERROR \
        -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        "$ROCKY_SSH_USER@127.0.0.1" "$@"
}

rocky_scp() {
    scp \
        -i "$ROCKY_KEY" \
        -P "$ROCKY_SSH_PORT" \
        -o BatchMode=yes \
        -o ConnectTimeout=5 \
        -o IdentitiesOnly=yes \
        -o LogLevel=ERROR \
        -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        "$@"
}

rocky_ssh_tty() {
    ssh -tt \
        -i "$ROCKY_KEY" \
        -p "$ROCKY_SSH_PORT" \
        -o BatchMode=yes \
        -o ConnectTimeout=5 \
        -o IdentitiesOnly=yes \
        -o LogLevel=ERROR \
        -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        "$ROCKY_SSH_USER@127.0.0.1" "$@"
}
