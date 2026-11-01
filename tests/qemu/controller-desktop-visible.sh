#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_file="$(mktemp -t ginux-desktop-visible.XXXXXX)"
monitor_socket="$(mktemp -u /tmp/ginux-desktop-monitor.XXXXXX)"
serial_fifo="$(mktemp -u /tmp/ginux-desktop-serial.XXXXXX)"
screenshot="$(mktemp -t ginux-desktop.XXXXXX).ppm"
keyboard_screenshot="$(mktemp -t ginux-desktop-keyboard.XXXXXX).ppm"
pointer_screenshot="$(mktemp -t ginux-desktop-pointer.XXXXXX).ppm"
click_screenshot="$(mktemp -t ginux-desktop-click.XXXXXX).ppm"
overview_screenshot="$(mktemp -t ginux-desktop-overview.XXXXXX).ppm"
live_screenshot="$(mktemp -t ginux-desktop-live.XXXXXX).ppm"
volume_image="$(mktemp -t ginux-desktop-volume.XXXXXX)"
artifact_dir="$repo_root/build/qualification/controller-desktop"
qemu_pid=""
palette=('\010\020\035' '\015\027\046' '\023\040\062' '\015\177\362')

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
    rm -f "$click_screenshot" "$overview_screenshot" "$live_screenshot"
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
printf 'sendkey ret 50\n' | nc -U "$monitor_socket" >/dev/null
for _ in $(seq 1 80); do
    if grep -Fq 'Usage:' "$output_file"; then
        break
    fi
    sleep 0.1
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

# Move from the controller's initial logical center into the Settings row,
# then exercise an actual PS/2 primary-button press and release.
printf 'mouse_move -173 38\nmouse_button 1\nmouse_button 0\n' | nc -U "$monitor_socket" >/dev/null
click_hash="$pointer_hash"
for _ in $(seq 1 80); do
    printf 'screendump %s\n' "$click_screenshot" | nc -U "$monitor_socket" >/dev/null
    click_hash="$(shasum -a 256 "$click_screenshot" | awk '{print $1}')"
    if [[ "$click_hash" != "$pointer_hash" ]] && frame_has_palette "$click_screenshot"; then
        break
    fi
    sleep 0.25
done

printf 'mouse_move 0 -142\nmouse_button 1\nmouse_button 0\n' | nc -U "$monitor_socket" >/dev/null
sleep 0.5
printf 'screendump %s\n' "$overview_screenshot" | nc -U "$monitor_socket" >/dev/null
overview_hash="$(shasum -a 256 "$overview_screenshot" | awk '{print $1}')"

# A CLI mutation must update the same live GUI model without restarting the
# controller. Enter it through native keyboard events to cover the GUI event
# path while the serial console remains available as the recovery path.
for key in d o m a i n spc n e w spc v i s i b l e ret; do
    printf 'sendkey %s 20\n' "$key" | nc -U "$monitor_socket" >/dev/null
    sleep 0.04
done
for _ in $(seq 1 80); do
    if grep -Fq 'visible: created' "$output_file"; then
        break
    fi
    sleep 0.25
done
sleep 0.5
printf 'screendump %s\n' "$live_screenshot" | nc -U "$monitor_socket" >/dev/null
live_hash="$(shasum -a 256 "$live_screenshot" | awk '{print $1}')"
printf 'quit\n' | nc -U "$monitor_socket" >/dev/null
wait "$qemu_pid" 2>/dev/null || true
qemu_pid=""

if [[ ! -s "$screenshot" ]] || [[ "$(head -c 2 "$screenshot")" != P6 ]] \
    || [[ ! -s "$keyboard_screenshot" ]] \
    || [[ ! -s "$pointer_screenshot" ]] \
    || [[ ! -s "$click_screenshot" ]] \
    || [[ ! -s "$overview_screenshot" ]] \
    || [[ ! -s "$live_screenshot" ]]; then
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
if [[ "$pointer_hash" == "$click_hash" ]]; then
    cat "$output_file" >&2
    echo 'Visible desktop test failed: pointer click did not change the active view.' >&2
    exit 1
fi
if ! grep -Fq 'visible: created' "$output_file" || [[ "$overview_hash" == "$live_hash" ]]; then
    cat "$output_file" >&2
    echo 'Visible desktop test failed: CLI state change did not update the GUI.' >&2
    exit 1
fi

mkdir -p "$artifact_dir"
cp "$screenshot" "$artifact_dir/baseline.ppm"
cp "$keyboard_screenshot" "$artifact_dir/keyboard.ppm"
cp "$pointer_screenshot" "$artifact_dir/pointer.ppm"
cp "$click_screenshot" "$artifact_dir/click.ppm"
cp "$overview_screenshot" "$artifact_dir/overview.ppm"
cp "$live_screenshot" "$artifact_dir/live-state.ppm"
if command -v sips >/dev/null 2>&1; then
    sips -s format png "$keyboard_screenshot" --out "$artifact_dir/keyboard.png" >/dev/null
    sips -s format png "$pointer_screenshot" --out "$artifact_dir/pointer.png" >/dev/null
    sips -s format png "$click_screenshot" --out "$artifact_dir/click.png" >/dev/null
    sips -s format png "$overview_screenshot" --out "$artifact_dir/overview.png" >/dev/null
    sips -s format png "$live_screenshot" --out "$artifact_dir/live-state.png" >/dev/null
fi

echo 'Visible interactive controller desktop framebuffer passed.'
echo "Evidence: $artifact_dir"
