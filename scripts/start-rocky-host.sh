#!/bin/sh
set -eu

. ./config/rocky-host-image.env
. ./scripts/rocky-host-common.sh

start_epoch=$(date +%s)
host_progress() {
    now=$(date +%s)
    printf 'Rocky host: [%ss] %s\n' "$((now - start_epoch))" "$1"
}

if [ -f "$ROCKY_PID_FILE" ]; then
    existing=$(cat "$ROCKY_PID_FILE")
    if kill -0 "$existing" 2>/dev/null; then
        printf 'Rocky host already running with PID %s\n' "$existing"
        exit 0
    fi
    rm -f "$ROCKY_PID_FILE"
fi

base=${ROCKY_IMAGE_PATH:-build/cache/rocky/$ROCKY_IMAGE_NAME}
case $base in
    /*) ;;
    *) base=$(pwd -P)/$base ;;
esac
ROCKY_IMAGE_PATH=$base ./scripts/prepare-rocky-host.sh
network_restrict=${ROCKY_NETWORK_RESTRICT:-on}
vcpus=${ROCKY_VCPUS:-2}
memory_mib=${ROCKY_MEMORY_MIB:-2048}
case "$network_restrict" in
    on|off) ;;
    *)
        printf 'ROCKY_NETWORK_RESTRICT must be on or off\n' >&2
        exit 2
        ;;
esac
case "$vcpus:$memory_mib" in
    *[!0-9:]*|:*|*:) printf '%s\n' 'ROCKY_VCPUS and ROCKY_MEMORY_MIB must be positive integers' >&2; exit 2 ;;
esac
test "$vcpus" -gt 0 && test "$memory_mib" -gt 0 || {
    printf '%s\n' 'ROCKY_VCPUS and ROCKY_MEMORY_MIB must be positive integers' >&2
    exit 2
}
overlay=$ROCKY_OVERLAY_DISK
mkdir -p "$(dirname "$overlay")" "$(dirname "$ROCKY_PID_FILE")" \
    "$(dirname "$ROCKY_SERIAL_LOG")"
if [ -f "$overlay" ]; then
    host_progress "reusing persistent host disk overlay $overlay"
else
    host_progress "creating persistent host disk overlay $overlay"
    qemu-img create -q -f qcow2 -F qcow2 -b "$base" "$overlay"
fi
rm -f "$ROCKY_SERIAL_LOG"

host_progress "starting x86-64 QEMU (TCG, $vcpus vCPUs, $memory_mib MiB)"
qemu-system-x86_64 \
    -machine q35 \
    -accel tcg,thread=multi \
    -cpu max \
    -smp "$vcpus" \
    -m "${memory_mib}M" \
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

host_progress 'waiting for SSH'
attempt=0
while [ "$attempt" -lt 120 ]; do
    if rocky_ssh true >/dev/null 2>&1; then
        host_progress 'SSH ready; waiting for cloud-init'
        cloud_attempt=0
        while ! rocky_ssh 'sudo cloud-init status --wait >/dev/null'; do
            cloud_attempt=$((cloud_attempt + 1))
            if [ "$cloud_attempt" -ge 12 ]; then
                printf '%s\n' 'Rocky cloud-init connection did not stabilize' >&2
                exit 1
            fi
            host_progress 'cloud-init SSH session reset; retrying'
            sleep 5
        done
        if [ "$network_restrict" = off ]; then
            host_progress 'cloud-init ready; enabling guest network access'
            rocky_ssh "sudo ip route replace default via 10.0.2.2 dev enp0s2; printf '%s\n' 'nameserver 10.0.2.3' | sudo tee /etc/resolv.conf >/dev/null"
        fi
        host_progress 'SSH and cloud-init ready'
        exit 0
    fi
    attempt=$((attempt + 1))
    sleep 5
done

printf '%s\n' 'Rocky host did not become ready' >&2
tail -n 80 "$ROCKY_SERIAL_LOG" >&2 || true
exit 1
