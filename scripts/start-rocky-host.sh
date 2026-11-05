#!/bin/sh
set -eu

. ./config/rocky-host-image.env
. ./scripts/rocky-host-common.sh

if [ -f "$ROCKY_PID_FILE" ]; then
    existing=$(cat "$ROCKY_PID_FILE")
    if kill -0 "$existing" 2>/dev/null; then
        printf 'Rocky host already running with PID %s\n' "$existing"
        exit 0
    fi
    rm -f "$ROCKY_PID_FILE"
fi

./scripts/prepare-rocky-host.sh
network_restrict=${ROCKY_NETWORK_RESTRICT:-on}
case "$network_restrict" in
    on|off) ;;
    *)
        printf 'ROCKY_NETWORK_RESTRICT must be on or off\n' >&2
        exit 2
        ;;
esac
base=$(pwd -P)/build/cache/rocky/$ROCKY_IMAGE_NAME
overlay=build/rocky-host-overlay.qcow2
rm -f "$overlay" "$ROCKY_SERIAL_LOG"
qemu-img create -q -f qcow2 -F qcow2 -b "$base" "$overlay"

qemu-system-x86_64 \
    -machine q35 \
    -accel tcg,thread=multi \
    -cpu max \
    -smp 2 \
    -m 2048M \
    -display none \
    -daemonize \
    -pidfile "$ROCKY_PID_FILE" \
    -serial "file:$ROCKY_SERIAL_LOG" \
    -monitor none \
    -no-reboot \
    -drive "if=virtio,format=qcow2,file=$overlay" \
    -drive "if=ide,media=cdrom,readonly=on,format=raw,file=build/rocky-seed.iso" \
    -netdev "user,id=net0,restrict=$network_restrict,hostfwd=tcp:127.0.0.1:$ROCKY_SSH_PORT-:22" \
    -device virtio-net-pci,netdev=net0

attempt=0
while [ "$attempt" -lt 120 ]; do
    if rocky_ssh true >/dev/null 2>&1; then
        rocky_ssh 'sudo cloud-init status --wait >/dev/null'
        if [ "$network_restrict" = off ]; then
            rocky_ssh "sudo ip route replace default via 10.0.2.2 dev enp0s2; printf '%s\n' 'nameserver 10.0.2.3' | sudo tee /etc/resolv.conf >/dev/null"
        fi
        printf '%s\n' 'Rocky host SSH and cloud-init ready'
        exit 0
    fi
    attempt=$((attempt + 1))
    sleep 5
done

printf '%s\n' 'Rocky host did not become ready' >&2
tail -n 80 "$ROCKY_SERIAL_LOG" >&2 || true
exit 1
