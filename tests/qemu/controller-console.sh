#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_file="$(mktemp -t ginux-controller.XXXXXX)"
volume_image="$(mktemp -t ginux-controller-volume.XXXXXX)"
qemu_pid=""

cleanup() {
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    rm -f "$output_file" "$volume_image"
}
trap cleanup EXIT

dd if=/dev/zero of="$volume_image" bs=1048576 count=1 2>/dev/null
{
    for _ in {1..60}; do
        if grep -Fq 'ginux> ' "$output_file"; then
            break
        fi
        if grep -Fq 'GINUX_KERNEL_PANIC' "$output_file"; then
            break
        fi
        sleep 1
    done
    prompt_count=1
    send_command() {
        printf '%s\n' "$1"
        prompt_count=$((prompt_count + 1))
        for _ in {1..60}; do
            observed="$(grep -Fo 'ginux> ' "$output_file" | wc -l | tr -d ' ')"
            if [[ "$observed" -ge "$prompt_count" ]]; then
                return
            fi
            if grep -Fq 'GINUX_KERNEL_PANIC' "$output_file"; then
                return
            fi
            sleep 1
        done
    }
    for command in \
        help 'container help' 'container set --help' 'network help' 'network set --help' '  status  ' components ls 'ls /etc' 'ls /proc' 'ls /proc/1' ps 'container list' \
        'network list' 'network status eth0' 'network status lo' 'network status eth9' 'network ping lo 127.0.0.1' 'network set lo dns 10.0.2.3' 'network set eth0 static 10.0.2.15/99 10.0.2.2' 'network set eth0 static 10.0.2.15/24 10.0.2.2' 'network set eth0 dns 10.0.2.3' 'network status eth0' 'network ping eth0 10.0.2.15' 'network ping eth0 10.0.2.2' 'network ping eth0 10.0.2.2' 'network stop eth0' 'network status eth0' 'network start eth0' 'network restart eth0' 'network ping eth0 10.0.2.2' 'network down eth0' 'network set eth0 dhcp' 'network status eth0' 'network ping eth0 10.0.2.2' 'network stop eth0' \
        'container status default' 'container new demo' 'container new demo' 'container list' \
        'container start' 'container set demo cpu=1 cpu=1' 'container list' \
        'container set demo image=embedded://minimal-system-v0 init=/sbin/init cpu=1 memory-mib=128 autostart=true' 'container set demo autostart=false' 'container set demo cpu 1' 'container list' status \
        'container start demo' 'container list' 'container status demo' 'ls /proc' 'ls /proc/100' ps \
        'container stop demo' 'container status demo' \
        'container start demo' 'container status demo' 'container fail demo' \
        'container status demo' 'container start demo' 'container status demo' unknown; do
        send_command "$command"
    done
    printf 'shutdown\n'
} | GINUX_VOLUME_IMAGE="$volume_image" "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..75}; do
    if grep -Fq 'GINUX_KERNEL_PANIC' "$output_file"; then
        kill "$qemu_pid" 2>/dev/null || true
        break
    fi
    if ! kill -0 "$qemu_pid" 2>/dev/null; then
        break
    fi
    sleep 1
done

if kill -0 "$qemu_pid" 2>/dev/null; then
    cat "$output_file"
    echo "Controller console test failed: QEMU did not shut down within 75 seconds." >&2
    exit 1
fi

if wait "$qemu_pid"; then
    status=0
else
    status=$?
    cat "$output_file"
    echo "Controller console test failed: QEMU exited with $status." >&2
    exit 1
fi
qemu_pid=""

for expected in \
    'ginux: embedded controller runtime measurement accepted' \
    'ginux: launching embedded controller in ring 3' \
    'eth0: automatic DHCP configuration' \
    'Ginux controller v0 (ring 3)' \
    'Ginux controller shell' \
    'Available Commands:' \
    'help [COMMAND...]' \
    'ls [PATH]' \
    'List the embedded measured root or /proc' \
    'ps' \
    'Show live host processes' \
    'container COMMAND' \
    'Manage runtime containers' \
    'network COMMAND' \
    'Manage network devices' \
    'network list' \
    'List network devices and their state' \
    'network status DEVICE' \
    'network set DEVICE {dhcp|dns ADDRESS|static ADDRESS/PREFIX GATEWAY}' \
    'network start DEVICE' \
    'network stop DEVICE' \
    'network restart DEVICE' \
    'DEVICE DOMAIN CONNECTED STATE CONFIGURATION ADDRESS GATEWAY DNS' \
    'eth0 control yes' \
    'lo control yes up configured 127.0.0.1/8 - -' \
    'network: loopback is automatic and immutable' \
    'network: device not found' \
    'network: invalid address or prefix' \
    'eth0: static network configured' \
    'eth0: DNS configured' \
    'eth0 control yes up configured 10.0.2.15/24 10.0.2.2 10.0.2.3' \
    'network ping DEVICE ADDRESS' \
    'Send one ICMP echo through a network device' \
    'PING 10.0.2.2' \
    '64 bytes from 127.0.0.1: icmp_seq=1 ttl=64' \
    '64 bytes from 10.0.2.15: icmp_seq=1 ttl=64' \
    '64 bytes from 10.0.2.2: icmp_seq=1 ttl=64' \
    'eth0: stopped' \
    'eth0 control yes down configured 10.0.2.15/24 10.0.2.2 10.0.2.3' \
    'eth0: started' \
    'eth0: restarted' \
    'eth0: requesting DHCP lease' \
    'eth0: DHCP configured 10.0.2.15/24 via 10.0.2.2 DNS 10.0.2.3' \
    'container list' \
    'List all containers' \
    'container new NAME' \
    'container set NAME FIELD=VALUE...' \
    'Set one or more container fields' \
    'controller: running (ring 3)' \
    'containers: 0 configured' \
    'container: not found' \
    'NAME STATE INIT CPU MEM-MIB AUTO IMAGE' \
    'demo: created (image required)' \
    'container: create rejected' \
    'demo configured /sbin/init 1 128 false embedded://minimal-system-v0' \
    'Error: missing arguments' \
    'Usage:' \
    'container: duplicate setting' \
    'demo: settings accepted; configured' \
    'demo: setting accepted; configured' \
    'demo running /sbin/init 1 128 false embedded://minimal-system-v0' \
    'containers: 1 runtime (demo)' \
    'demo: configured' \
    'demo: running' \
    'integrity: sealed' \
    'GINUX.EFI component catalog v0:' \
    '- kernel: active, ring 0' \
    '- controller: active, ring 3' \
    '- boot modules: embedded, sealed' \
    'bin/' \
    'etc/' \
    'sbin/' \
    'hostname' \
    'os-release' \
    'motd' \
    '1/' \
    '100/' \
    'status' \
    'PID PPID STATE NAME' \
    '1 0 R controller' \
    '100 0 R demo:init' \
    'demo: stopped' \
    'demo: starting' \
    'demo: failed (contained)' \
    'ginux: starting configured runtime container' \
    'Error: unknown command' \
    'Shutting down Ginux.' \
    'ginux: controller requested clean shutdown' \
    'GINUX_KERNEL_OK'; do
    if ! grep -Fq -- "$expected" "$output_file"; then
        cat "$output_file"
        echo "Controller console test failed: missing '$expected'." >&2
        exit 1
    fi
done

if grep -qE '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
    cat "$output_file"
    echo "Controller console test failed: a panic marker was emitted." >&2
    exit 1
fi

if [[ "$(grep -Fc 'Ginux controller v0 (ring 3)' "$output_file")" -ne 1 ]]; then
    cat "$output_file"
    echo "Controller console test failed: controller was relaunched." >&2
    exit 1
fi

if [[ "$(grep -Fc '64 bytes from 10.0.2.2: icmp_seq=1 ttl=64' "$output_file")" -ne 4 ]]; then
    cat "$output_file"
    echo "Controller console test failed: repeated network service generation did not complete." >&2
    exit 1
fi

if [[ "$(grep -Fc 'eth0: DHCP configured 10.0.2.15/24 via 10.0.2.2 DNS 10.0.2.3' "$output_file")" -ne 2 ]]; then
    cat "$output_file"
    echo "Controller console test failed: DHCP did not retain DNS on both leases." >&2
    exit 1
fi

if grep -qE '^([2-9]|[1-9][0-9]+) 0 R controller' "$output_file"; then
    cat "$output_file"
    echo "Controller console test failed: replacement controller PID was published." >&2
    exit 1
fi

echo "Interactive controller console test passed."
