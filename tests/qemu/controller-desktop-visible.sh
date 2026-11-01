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
power_screenshot="$(mktemp -t ginux-desktop-power.XXXXXX).ppm"
latency_reference="$(mktemp -t ginux-desktop-latency-reference.XXXXXX).ppm"
latency_capture="$(mktemp -t ginux-desktop-latency-capture.XXXXXX).ppm"
latency_samples="$(mktemp -t ginux-desktop-latency-samples.XXXXXX)"
monitor_fifo="$(mktemp -u /tmp/ginux-desktop-monitor-input.XXXXXX)"
volume_image="$(mktemp -t ginux-desktop-volume.XXXXXX)"
artifact_dir="$repo_root/build/qualification/controller-desktop"
qemu_pid=""
monitor_client_pid=""
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

frame_is_target_resolution() {
    [[ "$(sed -n '2p' "$1")" == '1024 768' ]]
}

frame_pixel_hex() {
    local frame="$1" x="$2" y="$3" header_size offset
    header_size="$(head -n 3 "$frame" | wc -c | tr -d ' ')"
    offset=$((header_size + (y * 1024 + x) * 3))
    dd if="$frame" bs=1 skip="$offset" count=3 2>/dev/null \
        | od -An -tx1 | tr -d ' \n'
}

timestamp_ms() {
    perl -MTime::HiRes=time -e 'printf "%.3f\n", time() * 1000'
}

move_pointer() {
    local remaining_x="$1" remaining_y="$2" step_x step_y
    while ((remaining_x != 0 || remaining_y != 0)); do
        step_x="$remaining_x"
        step_y="$remaining_y"
        ((step_x > 100)) && step_x=100
        ((step_x < -100)) && step_x=-100
        ((step_y > 100)) && step_y=100
        ((step_y < -100)) && step_y=-100
        monitor_command "mouse_move $step_x $step_y"
        sleep 0.002
        remaining_x=$((remaining_x - step_x))
        remaining_y=$((remaining_y - step_y))
    done
}

monitor_command() {
    printf '%s\n' "$1" >&8
}

capture_frame() {
    local frame="$1"
    rm -f "$frame"
    monitor_command "screendump $frame"
    for _ in $(seq 1 100); do
        if [[ -s "$frame" ]] && frame_is_target_resolution "$frame" \
            && [[ "$(wc -c <"$frame" | tr -d ' ')" -ge 2359312 ]]; then
            return 0
        fi
        sleep 0.001
    done
    echo "Visible desktop test failed: incomplete framebuffer capture $frame." >&2
    exit 1
}

start_monitor_client() {
    mkfifo "$monitor_fifo"
    nc -U "$monitor_socket" <"$monitor_fifo" >/dev/null &
    monitor_client_pid=$!
    exec 8>"$monitor_fifo"
}

measure_visible_change() {
    local label="$1" command="$2" start observed_at end elapsed changed=0
    start="$(timestamp_ms)"
    printf '%b' "$command" >&8
    # Let the emulated input IRQ run before requesting a screendump. An
    # immediate screendump pauses vCPU execution and measures the observer's
    # interference rather than the presentation path.
    sleep 0.008
    for _ in $(seq 1 80); do
        # Timestamp when QEMU is asked to sample the display. Encoding and
        # writing the PPM happens after that sample and is observer overhead,
        # not controller presentation latency.
        observed_at="$(timestamp_ms)"
        capture_frame "$latency_capture"
        if ! cmp -s "$latency_reference" "$latency_capture"; then
            changed=1
            end="$observed_at"
            break
        fi
        sleep 0.005
    done
    if ((changed == 0)); then
        mkdir -p "$artifact_dir"
        cp "$latency_reference" "$artifact_dir/debug-$label.ppm"
        if command -v sips >/dev/null 2>&1; then
            sips -s format png "$latency_reference" \
                --out "$artifact_dir/debug-$label.png" >/dev/null
        fi
        echo "Visible desktop test failed: $label did not produce a visible frame." >&2
        exit 1
    fi
    elapsed="$(awk -v start="$start" -v end="$end" 'BEGIN { printf "%.3f", end - start }')"
    printf '%s\n' "$elapsed" >>"$latency_samples"
    cp "$latency_capture" "$latency_reference"
    printf '%s %s ms\n' "$label" "$elapsed"
}

measure_visible_pixel() {
    local label="$1" command="$2" x="$3" y="$4" expected="$5"
    local start observed_at end elapsed changed=0
    start="$(timestamp_ms)"
    printf '%b' "$command" >&8
    sleep 0.008
    for _ in $(seq 1 80); do
        observed_at="$(timestamp_ms)"
        capture_frame "$latency_capture"
        if [[ "$(frame_pixel_hex "$latency_capture" "$x" "$y")" == "$expected" ]]; then
            changed=1
            end="$observed_at"
            break
        fi
        sleep 0.005
    done
    if ((changed == 0)); then
        mkdir -p "$artifact_dir"
        cp "$latency_capture" "$artifact_dir/debug-$label.ppm"
        if command -v sips >/dev/null 2>&1; then
            sips -s format png "$latency_capture" \
                --out "$artifact_dir/debug-$label.png" >/dev/null
        fi
        echo "Visible desktop test failed: $label did not complete its frame." >&2
        exit 1
    fi
    elapsed="$(awk -v start="$start" -v end="$end" 'BEGIN { printf "%.3f", end - start }')"
    printf '%s\n' "$elapsed" >>"$latency_samples"
    cp "$latency_capture" "$latency_reference"
    printf '%s %s ms\n' "$label" "$elapsed"
}

cleanup() {
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    exec 8>&- 2>/dev/null || true
    if [[ -n "$monitor_client_pid" ]] && kill -0 "$monitor_client_pid" 2>/dev/null; then
        kill "$monitor_client_pid" 2>/dev/null || true
        wait "$monitor_client_pid" 2>/dev/null || true
    fi
    exec 9>&- 9<&- || true
    rm -f "$output_file" "$monitor_socket" "$serial_fifo" "$screenshot" "$keyboard_screenshot" \
        "$pointer_screenshot" "$volume_image"
    rm -f "$click_screenshot" "$overview_screenshot" "$live_screenshot"
    rm -f "$power_screenshot"
    rm -f "$latency_reference" "$latency_capture" "$latency_samples"
    rm -f "$monitor_fifo"
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
    if frame_has_palette "$keyboard_screenshot"; then
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

start_monitor_client
cp "$pointer_screenshot" "$latency_reference"
for sample in $(seq 1 20); do
    if ((sample % 2 == 1)); then
        delta=6
    else
        delta=-6
    fi
    measure_visible_change "pointer-$sample" "mouse_move $delta 0\n"
done

# HMP relative movement is scaled by the emulated PS/2 resolution. Clamp to a
# known edge before selecting exact shell controls so the test is independent
# of that scale and of its earlier latency samples.
move_pointer -5000 5000
sleep 0.05
move_pointer 104 -303
sleep 0.05
capture_frame "$latency_reference"
measure_visible_pixel 'page-settings' 'mouse_button 1\nmouse_button 0\n' 16 440 0d7ff2
cp "$latency_capture" "$click_screenshot"
click_hash="$pointer_hash"
click_hash="$(shasum -a 256 "$click_screenshot" | awk '{print $1}')"
if [[ "$(frame_pixel_hex "$click_screenshot" 16 440)" != '0d7ff2' ]]; then
    mkdir -p "$artifact_dir"
    cp "$click_screenshot" "$artifact_dir/debug-target-settings.ppm"
    if command -v sips >/dev/null 2>&1; then
        sips -s format png "$click_screenshot" --out "$artifact_dir/debug-target-settings.png" >/dev/null
    fi
    echo 'Visible desktop test failed: Settings click did not select Settings.' >&2
    exit 1
fi

move_pointer -5000 -5000
sleep 0.05
move_pointer 104 98
sleep 0.05
printf 'mouse_button 1\nmouse_button 0\n' >&8
sleep 0.5
capture_frame "$overview_screenshot"
overview_hash="$(shasum -a 256 "$overview_screenshot" | awk '{print $1}')"
if [[ "$(frame_pixel_hex "$overview_screenshot" 16 110)" != '0d7ff2' ]]; then
    echo 'Visible desktop test failed: Overview click did not select Overview.' >&2
    exit 1
fi

# A recovery-CLI mutation must update the same live GUI model without
# restarting the controller. Native keyboard input was exercised above; this
# deliberately proves that the independent serial path remains authoritative.
printf 'domain new visible\n' >&9
for _ in $(seq 1 80); do
    if grep -Fq 'visible: created' "$output_file"; then
        break
    fi
    sleep 0.25
done
printf 'domain list\n' >&9
for _ in $(seq 1 40); do
    if grep -Fq 'NAME            STATE' "$output_file"; then
        break
    fi
    sleep 0.1
done
# The mapped framebuffer is persistent. Poll until the controller's periodic
# live-state snapshot has incorporated the serial mutation.
live_hash="$overview_hash"
for _ in $(seq 1 20); do
    sleep 0.05
    capture_frame "$live_screenshot"
    live_hash="$(shasum -a 256 "$live_screenshot" | awk '{print $1}')"
    [[ "$live_hash" != "$overview_hash" ]] && break
done

# Open the real power menu from the known pointer position and retain visual
# evidence without triggering either destructive action.
move_pointer 5000 -5000
sleep 0.05
move_pointer -56 30
sleep 0.05
sleep 0.2
monitor_command 'mouse_button 1'
sleep 0.1
monitor_command 'mouse_button 0'
sleep 1
capture_frame "$power_screenshot"
power_hash="$(shasum -a 256 "$power_screenshot" | awk '{print $1}')"
monitor_command 'quit'
wait "$qemu_pid" 2>/dev/null || true
qemu_pid=""

if [[ ! -s "$screenshot" ]] || [[ "$(head -c 2 "$screenshot")" != P6 ]] \
    || [[ ! -s "$keyboard_screenshot" ]] \
    || [[ ! -s "$pointer_screenshot" ]] \
    || [[ ! -s "$click_screenshot" ]] \
    || [[ ! -s "$overview_screenshot" ]] \
    || [[ ! -s "$live_screenshot" ]] \
    || [[ ! -s "$power_screenshot" ]]; then
    echo 'Visible desktop test failed: QEMU did not produce a PPM framebuffer.' >&2
    exit 1
fi
for frame in "$screenshot" "$keyboard_screenshot" "$pointer_screenshot" "$click_screenshot" \
    "$overview_screenshot" "$live_screenshot" "$power_screenshot"; do
    if ! frame_is_target_resolution "$frame"; then
        echo "Visible desktop test failed: expected 1024x768 framebuffer in $frame." >&2
        exit 1
    fi
done
for color in "${palette[@]}"; do
    pattern="$(printf '%b' "$color")"
    if ! LC_ALL=C grep -aFq "$pattern" "$screenshot"; then
        echo "Visible desktop test failed: rendered color $color is absent." >&2
        exit 1
    fi
done

if ! grep -Fq 'Usage:' "$output_file"; then
    cat "$output_file" >&2
    echo 'Visible desktop test failed: serial recovery command did not execute.' >&2
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

sorted_latencies="$artifact_dir/interaction-latencies-sorted-ms.txt"
mkdir -p "$artifact_dir"
LC_ALL=C sort -n "$latency_samples" >"$sorted_latencies"
latency_count="$(wc -l <"$sorted_latencies" | tr -d ' ')"
p95_index=$(((latency_count * 95 + 99) / 100))
p95_latency="$(sed -n "${p95_index}p" "$sorted_latencies")"
max_latency="$(tail -n 1 "$sorted_latencies")"
page_latency="$(tail -n 1 "$latency_samples")"
if ! awk -v p95="$p95_latency" -v max="$max_latency" -v page="$page_latency" \
    'BEGIN { exit !(p95 <= 50 && max <= 100 && page <= 250) }'; then
    printf 'Visible desktop latency failed: p95=%s ms max=%s ms page=%s ms\n' \
        "$p95_latency" "$max_latency" "$page_latency" >&2
    exit 1
fi
if ! grep -Fq 'visible: created' "$output_file" || [[ "$overview_hash" == "$live_hash" ]]; then
    mkdir -p "$artifact_dir"
    cp "$click_screenshot" "$artifact_dir/debug-settings.ppm"
    cp "$overview_screenshot" "$artifact_dir/debug-overview.ppm"
    cp "$live_screenshot" "$artifact_dir/debug-live.ppm"
    if command -v sips >/dev/null 2>&1; then
        sips -s format png "$click_screenshot" --out "$artifact_dir/debug-settings.png" >/dev/null
        sips -s format png "$overview_screenshot" --out "$artifact_dir/debug-overview.png" >/dev/null
        sips -s format png "$live_screenshot" --out "$artifact_dir/debug-live.png" >/dev/null
    fi
    printf 'settings=%s overview=%s live=%s\n' "$click_hash" "$overview_hash" "$live_hash" >&2
    cat "$output_file" >&2
    echo 'Visible desktop test failed: CLI state change did not update the GUI.' >&2
    exit 1
fi
danger_pattern="$(printf '\341\133\144')"
if [[ "$live_hash" == "$power_hash" ]] || ! LC_ALL=C grep -aFq "$danger_pattern" "$power_screenshot"; then
    cat "$output_file" >&2
    echo 'Visible desktop test failed: power button did not open its menu.' >&2
    exit 1
fi

cp "$output_file" "$artifact_dir/serial.log"
cp "$screenshot" "$artifact_dir/baseline.ppm"
cp "$keyboard_screenshot" "$artifact_dir/keyboard.ppm"
cp "$pointer_screenshot" "$artifact_dir/pointer.ppm"
cp "$click_screenshot" "$artifact_dir/click.ppm"
cp "$overview_screenshot" "$artifact_dir/overview.ppm"
cp "$live_screenshot" "$artifact_dir/live-state.ppm"
cp "$power_screenshot" "$artifact_dir/power-menu.ppm"
if command -v sips >/dev/null 2>&1; then
    sips -s format png "$keyboard_screenshot" --out "$artifact_dir/keyboard.png" >/dev/null
    sips -s format png "$pointer_screenshot" --out "$artifact_dir/pointer.png" >/dev/null
    sips -s format png "$click_screenshot" --out "$artifact_dir/click.png" >/dev/null
    sips -s format png "$overview_screenshot" --out "$artifact_dir/overview.png" >/dev/null
    sips -s format png "$live_screenshot" --out "$artifact_dir/live-state.png" >/dev/null
    sips -s format png "$power_screenshot" --out "$artifact_dir/power-menu.png" >/dev/null
fi

printf 'resolution=1024x768\np95_ms=%s\nmax_ms=%s\npage_ms=%s\n' \
    "$p95_latency" "$max_latency" "$page_latency" >"$artifact_dir/latency-summary.txt"

echo 'Visible interactive controller desktop framebuffer passed.'
printf 'Latency: p95=%s ms max=%s ms page=%s ms\n' \
    "$p95_latency" "$max_latency" "$page_latency"
echo "Evidence: $artifact_dir"
