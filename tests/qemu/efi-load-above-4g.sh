#!/usr/bin/env bash

# Boot the normal image twice. With 8 GiB of guest RAM OVMF loads GINUX.EFI at
# its 5 GiB link base, so the UEFI stage must relocate it below 4 GiB before
# the kernel starts. With 1 GiB the image already lies below 4 GiB and must
# not be moved. Both boots must reach the controller and shut down cleanly.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
boot_timeout_seconds="${GINUX_BOOT_TIMEOUT_SECONDS:-180}"
if [[ ! "$boot_timeout_seconds" =~ ^[1-9][0-9]*$ ]]; then
    echo "EFI load test failed: GINUX_BOOT_TIMEOUT_SECONDS must be a positive integer." >&2
    exit 2
fi
identity_limit=$((1 << 32))

output_file="$(mktemp -t ginux-efi-load.XXXXXX)"
volume_image="$(mktemp -t ginux-efi-load-volume.XXXXXX)"
qemu_pid_file="$(mktemp -t ginux-efi-load-pid.XXXXXX)"
qemu_pid=""

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

fail() {
    cat "$output_file"
    echo "EFI load test failed ($1): $2" >&2
    exit 1
}

# Prints the first hexadecimal value that follows the given serial prefix.
serial_hex() {
    local line
    line="$(grep -aF -- "$1" "$output_file" | head -n 1 | tr -d '\r')" || true
    if [[ "$line" =~ ^"$1"(0x[0-9a-f]+) ]]; then
        printf '%s\n' "${BASH_REMATCH[1]}"
    fi
}

boot() {
    local memory="$1"

    dd if=/dev/zero of="$volume_image" bs=1048576 count=1 2>/dev/null
    : >"$output_file"
    : >"$qemu_pid_file"
    # shellcheck disable=SC2094
    {
        for ((attempt = 0; attempt < boot_timeout_seconds; attempt++)); do
            if grep -aFq 'ginux> ' "$output_file"; then
                printf 'shutdown\n'
                exit 0
            fi
            if grep -aEq 'GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
                stop_qemu
                exit 0
            fi
            sleep 1
        done
    } | GINUX_MEMORY="$memory" GINUX_CPUS=2 GINUX_VOLUME_IMAGE="$volume_image" \
        GINUX_QEMU_PID_FILE="$qemu_pid_file" \
        "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
    qemu_pid=$!

    for ((attempt = 0; attempt < boot_timeout_seconds; attempt++)); do
        if ! kill -0 "$qemu_pid" 2>/dev/null; then
            break
        fi
        sleep 1
    done
    if kill -0 "$qemu_pid" 2>/dev/null; then
        stop_qemu
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
        qemu_pid=""
        fail "$memory" "boot did not finish within ${boot_timeout_seconds} seconds."
    fi
    if wait "$qemu_pid"; then
        qemu_status=0
    else
        qemu_status=$?
    fi
    qemu_pid=""

    if grep -aEq 'GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
        fail "$memory" "a boot or kernel panic was emitted."
    fi
    if [[ "$qemu_status" -ne 0 ]]; then
        fail "$memory" "QEMU exited with $qemu_status; expected 0."
    fi
    # The kernel rejects a handed-off image outside its identity map before
    # GINUX_KERNEL_OK, so reaching the controller proves the handoff.
    for expected in \
        'GINUX_KERNEL_OK' \
        'ginux: launching embedded controller in ring 3' \
        'Ginux controller v0 (ring 3)' \
        'ginux> ' \
        'ginux: controller requested clean shutdown'; do
        if ! grep -aFq -- "$expected" "$output_file"; then
            fail "$memory" "missing '$expected'."
        fi
    done

    loaded_base="$(serial_hex 'ginux: EFI image loaded at ')"
    loaded_size="$(serial_hex "ginux: EFI image loaded at $loaded_base size ")"
    if [[ -z "$loaded_base" || -z "$loaded_size" ]]; then
        fail "$memory" "the UEFI stage did not report the firmware load address."
    fi
    relocated_base="$(serial_hex 'ginux: EFI image relocated below 4 GiB to ')"
    entered_base="$(serial_hex 'ginux: EFI image entered at relocated base ')"
}

boot 8G
if ((loaded_base < identity_limit)); then
    fail 8G "firmware loaded the image at $loaded_base, not above 4 GiB."
fi
if [[ -z "$relocated_base" ]] || ((relocated_base + loaded_size > identity_limit)); then
    fail 8G "the image was not relocated below 4 GiB."
fi
if [[ "$entered_base" != "$relocated_base" ]]; then
    fail 8G "the UEFI stage did not continue in the relocated copy."
fi
echo "8 GiB guest: firmware loaded the image at $loaded_base, relocated to $relocated_base."

boot 1G
if ((loaded_base + loaded_size > identity_limit)); then
    fail 1G "firmware loaded the image at $loaded_base, not below 4 GiB."
fi
if [[ -n "$relocated_base" || -n "$entered_base" ]]; then
    fail 1G "an image already below 4 GiB was relocated."
fi
echo "1 GiB guest: firmware loaded the image at $loaded_base, not relocated."

echo "EFI load test passed: the image is relocated below 4 GiB only when firmware loads it above, and both boots reach the controller and shut down cleanly."
