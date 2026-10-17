#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_file="$(mktemp -t ginux-controller.XXXXXX)"
volume_image="$(mktemp -t ginux-controller-volume.XXXXXX)"
qemu_pid_file="$(mktemp -t ginux-controller-pid.XXXXXX)"
qemu_pid=""
: >"$qemu_pid_file"

stop_qemu() {
    local pid=""
    if [[ -s "$qemu_pid_file" ]]; then
        read -r pid <"$qemu_pid_file" || true
    fi
    if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
    fi
}

cleanup() {
    stop_qemu
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    rm -f "$output_file" "$volume_image" "$qemu_pid_file"
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
        '' '   ' $'sta\t' $'statuz\177s' $'statuz\bs' $'domain a\t' help 'domain help' 'domain set --help' 'network help' 'network set --help' 'test linux' 'test linux all' 'test linux syscall read' 'test linux syscall accept' 'test linux syscall 9999' '  status  ' components ls 'ls /etc' 'ls /proc' 'ls /proc/1' ps 'domain list' \
        'network list' 'network new gnet0 type=virtual' 'network new gnet1 type=virtual uplink=gnet0' 'network list' 'network status gnet0' 'network stop gnet0' 'network status gnet0' 'network start gnet0' 'network restart gnet1' 'network status eth0' 'network status lo' 'network status eth9' 'network ping lo 127.0.0.1' 'network set lo dns 10.0.2.3' 'network set eth0 static 10.0.2.15/99 10.0.2.2' 'network set eth0 static 10.0.2.15/24 10.0.2.2' 'network set eth0 dns 10.0.2.3' 'network status eth0' 'network ping eth0 10.0.2.15' 'network ping eth0 10.0.2.2' 'network ping eth0 10.0.2.2' 'network stop eth0' 'network status eth0' 'network start eth0' 'network restart eth0' 'network ping eth0 10.0.2.2' 'network down eth0' 'network set eth0 dhcp' 'network status eth0' 'network ping eth0 10.0.2.2' 'network stop eth0' \
        'domain status default' 'domain new demo' 'domain new demo' 'domain new rocky' 'domain set rocky image=rockylinux/rockylinux:10 init=/bin/sh' 'domain list' \
        'domain start' 'domain set demo cpu=1 cpu=1' 'domain set demo personality=linux image=alpine@sha256:d56c381f961d307a21b3ca004cf1e3910f106644aefb1f43e654c8a56c4fd395 init=/bin/sh' 'domain list' 'domain set demo personality=linux image=rockylinux/rockylinux:10' 'domain list' \
        'domain set demo personality=linux image=embedded://minimal-system-v0 init=/sbin/init cpu=1 memory-mib=1048576 autostart=true network=gnet0 network=gnet1' 'domain set demo autostart=false' 'domain set demo cpu 1' 'domain list' 'network list' status \
        'domain start demo' 'domain list' 'domain status demo' 'ls /proc' 'ls /proc/100' ps \
        'domain stop demo' 'domain status demo' \
        'domain start demo' 'domain status demo' 'domain fail demo' \
        'domain status demo' 'domain start demo' 'domain status demo' unknown; do
        send_command "$command"
    done
    printf 'shutdown\n'
} | GINUX_VOLUME_IMAGE="$volume_image" GINUX_QEMU_PID_FILE="$qemu_pid_file" \
    "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..75}; do
    if grep -Fq 'GINUX_KERNEL_PANIC' "$output_file"; then
        stop_qemu
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
: >"$qemu_pid_file"

for expected in \
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
    'domain COMMAND' \
    'Manage runtime domains' \
    'network COMMAND' \
    'Manage network devices' \
    'test ENVIRONMENT' \
    'Test a userspace compatibility environment' \
    'test linux [summary|interfaces|syscalls|all|syscall NAME|NUMBER]' \
    'Linux userspace runtime conformance summary:' \
    'Interfaces: 16 passed, 7 failed, 23 total' \
    'Syscalls: 0 passed, 200 unverified, 28 policy-denied, 147 unsupported, 375 total' \
    "Run 'test linux all' for every result." \
    'Linux userspace interface runtime tests (bounded profile):' \
    'PASS syscall-entry' \
    'PASS elf64-loader' \
    'PASS initial-stack-and-auxv' \
    'PASS dynamic-loader' \
    'PASS tls' \
    'PASS process-and-thread' \
    'PASS signals' \
    'PASS memory-management' \
    'PASS descriptors-and-events' \
    'PASS vfs-and-path-lookup' \
    'PASS procfs' \
    'PASS devfs-and-terminal' \
    'PASS tmpfs' \
    'PASS credentials-and-user-namespaces' \
    'PASS clocks-and-timers' \
    'PASS ioctl-profile' \
    'FAIL sysfs' \
    'FAIL mount-and-namespace-api' \
    'FAIL socket-api' \
    'FAIL cgroups' \
    'FAIL seccomp-audit-and-lsm' \
    'FAIL vdso-and-vvar' \
    'FAIL linux-module-userspace-api' \
    'Summary: 16 passed, 7 failed, 23 total' \
    'PASS covers the current bounded Linux profile; FAIL is unsupported or unverified.' \
    'Linux x86-64 syscall conformance ledger:' \
    'UNVERIFIED 0 read' \
    'UNSUPPORTED 43 accept' \
    'POLICY-DENIED 105 setuid' \
    'UNSUPPORTED 462 mseal' \
    'Summary: 0 passed, 200 unverified, 28 policy-denied, 147 unsupported, 375 total' \
    'PASS requires a runtime conformance probe; dispatch alone remains UNVERIFIED.' \
    'FAIL unknown Linux syscall' \
    'network list' \
    'List network devices and their state' \
    'network new NAME type=virtual [uplink=DEVICE]' \
    'gnet0: virtual network created; uplink eth0' \
    'gnet1: virtual network created; uplink gnet0' \
    'gnet0: stopped' \
    'gnet0   -        yes        down   virtual' \
    'gnet0: started' \
    'gnet1: restarted' \
    'network status DEVICE' \
    'network set DEVICE {dhcp|dns ADDRESS|static ADDRESS/PREFIX GATEWAY}' \
    'network start DEVICE' \
    'network stop DEVICE' \
    'network restart DEVICE' \
    'DEVICE  DOMAIN   CONNECTED  STATE  CONFIGURATION  ADDRESS          GATEWAY          DNS' \
    'eth0    control  yes        up     configured     10.0.2.15/24' \
    'lo      control  yes        up     configured     127.0.0.1/8' \
    'network: loopback is automatic and immutable' \
    'network: device not found' \
    'network: invalid address or prefix' \
    'eth0: static network configured' \
    'eth0: DNS configured' \
    'eth0    control  yes        up     configured     10.0.2.15/24' \
    'network ping DEVICE ADDRESS' \
    'Send one ICMP echo through a network device' \
    'PING 10.0.2.2' \
    '64 bytes from 127.0.0.1: icmp_seq=1 ttl=64' \
    '64 bytes from 10.0.2.15: icmp_seq=1 ttl=64' \
    '64 bytes from 10.0.2.2: icmp_seq=1 ttl=64' \
    'eth0: stopped' \
    'eth0    control  yes        down   configured     10.0.2.15/24' \
    'eth0: started' \
    'eth0: restarted' \
    'eth0: requesting DHCP lease' \
    'eth0: DHCP configured 10.0.2.15/24 via 10.0.2.2 DNS 10.0.2.3' \
    'domain list' \
    'List all domains' \
    'domain new NAME' \
    'domain set NAME FIELD=VALUE...' \
    'Set one or more domain fields' \
    'controller: running (ring 3)' \
    'domains: 0 configured' \
    'domain: not found' \
    'NAME            STATE       PERSONALITY  INIT                  CPU  MEM-MIB  AUTO   NETWORKS          IMAGE' \
    'demo: created (image required)' \
    'domain: create rejected' \
    'rocky: created (image required)' \
    'rocky           configured  linux' \
    'docker.io/rockylinux/rockylinux:10' \
    'demo            configured  linux' \
    'gnet0,gnet1' \
    'embedded://minimal-system-v0' \
    'Error: missing arguments' \
    'Usage:' \
    'domain: duplicate setting' \
    'demo            configured  linux' \
    'demo: settings accepted; configured' \
    'demo: setting accepted; configured' \
    'demo            running     linux' \
    'domains: 2 configured' \
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
    'PID    PPID   STATE  NAME' \
    '1      0      R      controller' \
    '100    0      R      demo:init' \
    'demo: stopped' \
    'demo: failed (contained)' \
    'ginux: starting configured runtime domain' \
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

if grep -qE 'vtd_iova_to_slpte|vtd_iommu_translate|Interrupt Mask set|New fault is not recorded' "$output_file"; then
    cat "$output_file"
    echo "Controller console test failed: normal boot emitted an intentional VT-d fault diagnostic." >&2
    exit 1
fi

if grep -Fq 'ginux: UEFI boot entered' "$output_file"; then
    cat "$output_file"
    echo "Controller console test failed: obsolete UEFI entry chatter was emitted." >&2
    exit 1
fi

if grep -Fq 'BdsDxe: failed to load' "$output_file"; then
    cat "$output_file"
    echo "Controller console test failed: firmware attempted a non-bootable device first." >&2
    exit 1
fi

if [[ "$(grep -Fc 'Ginux controller v0 (ring 3)' "$output_file")" -ne 1 ]]; then
    cat "$output_file"
    echo "Controller console test failed: controller was relaunched." >&2
    exit 1
fi

if [[ "$(grep -Fc 'ginux> status' "$output_file")" -ne 2 ]]; then
    cat "$output_file"
    echo "Controller console test failed: tab completion did not expand 'sta' to 'status'." >&2
    exit 1
fi

if [[ "$(grep -Fc 'controller: running (ring 3)' "$output_file")" -ne 5 ]]; then
    cat "$output_file"
    echo "Controller console test failed: backspace or delete editing did not execute status." >&2
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

if grep -qE '^([2-9]|[1-9][0-9]+) +0 +R +controller' "$output_file"; then
    cat "$output_file"
    echo "Controller console test failed: replacement controller PID was published." >&2
    exit 1
fi

echo "Interactive controller console test passed."
