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
        help 'container help' 'container set --help' '  status  ' components ls 'ls /etc' 'ls /proc' 'ls /proc/1' ps \
        'container status default' 'container new demo' 'container new demo' \
        'container start' 'container start demo' 'container set demo init /sbin/init' 'container set demo cpu 1' \
        'container set demo memory-mib 128' 'container set demo autostart true' status \
        'container start demo' 'container status demo' 'ls /proc' 'ls /proc/100' ps \
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
    'container new NAME' \
    'container set NAME FIELD VALUE' \
    'Set init=/sbin/init, cpu=1, memory-mib=128, or autostart=true' \
    'controller: running (ring 3)' \
    'containers: 0 configured' \
    'container: not found' \
    'demo: created' \
    'container: create rejected' \
    'Error: missing arguments' \
    'Usage:' \
    'container: start rejected' \
    'demo: setting accepted; created (incomplete)' \
    'demo: setting accepted; configured' \
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

if grep -qE '^([2-9]|[1-9][0-9]+) 0 R controller' "$output_file"; then
    cat "$output_file"
    echo "Controller console test failed: replacement controller PID was published." >&2
    exit 1
fi

echo "Interactive controller console test passed."
