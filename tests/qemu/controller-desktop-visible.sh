#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_file="$(mktemp -t ginux-desktop-visible.XXXXXX)"
monitor_socket="$(mktemp -u /tmp/ginux-desktop-monitor.XXXXXX)"
serial_fifo="$(mktemp -u /tmp/ginux-desktop-serial.XXXXXX)"
screenshot="$(mktemp -t ginux-desktop.XXXXXX).ppm"
keyboard_screenshot="$(mktemp -t ginux-desktop-keyboard.XXXXXX).ppm"
pointer_screenshot="$(mktemp -t ginux-desktop-pointer.XXXXXX).ppm"
volume_image="$(mktemp -t ginux-desktop-volume.XXXXXX)"
artifact_dir="$repo_root/build/qualification/controller-desktop"
qemu_pid=""
palette=('\030\040\050' '\050\064\100' '\030\214\170' '\140\170\240')

frame_has_palette() {
    local frame="$1"
    local color pattern
    [[ -s "$frame" ]] && [[ "$(head -c 2 "$frame")" == P6 ]] || return 1
    for color in "${palette[@]}"; do
        pattern="$(printf '%b' "$color")"
        LC_ALL=C grep -aFq "$pattern" "$frame" || return 1
    done
}

cleanup() {
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    exec 9>&- 9<&- || true
    rm -f "$output_file" "$monitor_socket" "$serial_fifo" "$screenshot" "$keyboard_screenshot" \
        "$pointer_screenshot" "$volume_image"
}
trap cleanup EXIT

dd if=/dev/zero of="$volume_image" bs=1048576 count=1 2>/dev/null
mkfifo "$serial_fifo"
exec 9<>"$serial_fifo"
GINUX_VOLUME_IMAGE="$volume_image" \
    GINUX_QEMU_DISPLAY="${GINUX_TEST_DISPLAY:-none}" \
    GINUX_QEMU_MONITOR="unix:$monitor_socket,server=on,wait=off" \
    "$repo_root/tools/qemu/run.sh" <"$serial_fifo" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in $(seq 1 180); do
    if grep -Fq 'ginux> ' "$output_file"; then
        break
    fi
    if ! kill -0 "$qemu_pid" 2>/dev/null; then
        cat "$output_file"
        echo 'Visible desktop test failed: QEMU exited before rendering.' >&2
        exit 1
    fi
    sleep 1
done

if ! grep -Fq 'ginux> ' "$output_file"; then
    cat "$output_file"
    echo 'Visible desktop test failed: render marker timed out.' >&2
    exit 1
fi

printf 'screendump %s\n' "$screenshot" | nc -U "$monitor_socket" >/dev/null
for key in h e l p; do
    printf 'sendkey %s 50\n' "$key" | nc -U "$monitor_socket" >/dev/null
    sleep 0.1
done

baseline_hash="$(shasum -a 256 "$screenshot" | awk '{print $1}')"
keyboard_hash="$baseline_hash"
for _ in $(seq 1 80); do
    printf 'screendump %s\n' "$keyboard_screenshot" | nc -U "$monitor_socket" >/dev/null
    keyboard_hash="$(shasum -a 256 "$keyboard_screenshot" | awk '{print $1}')"
    if [[ "$keyboard_hash" != "$baseline_hash" ]] && frame_has_palette "$keyboard_screenshot"; then
        break
    fi
    sleep 0.25
done

printf 'mouse_move 18 9\n' | nc -U "$monitor_socket" >/dev/null
pointer_hash="$keyboard_hash"
for _ in $(seq 1 80); do
    printf 'screendump %s\n' "$pointer_screenshot" | nc -U "$monitor_socket" >/dev/null
    pointer_hash="$(shasum -a 256 "$pointer_screenshot" | awk '{print $1}')"
    if [[ "$pointer_hash" != "$keyboard_hash" ]] && frame_has_palette "$pointer_screenshot"; then
        break
    fi
    sleep 0.25
done
printf 'quit\n' | nc -U "$monitor_socket" >/dev/null
wait "$qemu_pid" 2>/dev/null || true
qemu_pid=""

if [[ ! -s "$screenshot" ]] || [[ "$(head -c 2 "$screenshot")" != P6 ]] \
    || [[ ! -s "$keyboard_screenshot" ]] \
    || [[ ! -s "$pointer_screenshot" ]]; then
    echo 'Visible desktop test failed: QEMU did not produce a PPM framebuffer.' >&2
    exit 1
fi
for color in "${palette[@]}"; do
    pattern="$(printf '%b' "$color")"
    if ! LC_ALL=C grep -aFq "$pattern" "$screenshot"; then
        echo "Visible desktop test failed: rendered color $color is absent." >&2
        exit 1
    fi
done

if [[ "$baseline_hash" == "$keyboard_hash" ]]; then
    cat "$output_file" >&2
    echo 'Visible desktop test failed: injected keyboard input did not change the framebuffer.' >&2
    exit 1
fi
if [[ "$keyboard_hash" == "$pointer_hash" ]]; then
    cat "$output_file" >&2
    echo 'Visible desktop test failed: injected pointer input did not change the framebuffer.' >&2
    exit 1
fi

mkdir -p "$artifact_dir"
cp "$screenshot" "$artifact_dir/baseline.ppm"
cp "$keyboard_screenshot" "$artifact_dir/keyboard.ppm"
cp "$pointer_screenshot" "$artifact_dir/pointer.ppm"
if command -v sips >/dev/null 2>&1; then
    sips -s format png "$keyboard_screenshot" --out "$artifact_dir/keyboard.png" >/dev/null
    sips -s format png "$pointer_screenshot" --out "$artifact_dir/pointer.png" >/dev/null
fi

echo 'Visible interactive controller desktop framebuffer passed.'
echo "Evidence: $artifact_dir"
