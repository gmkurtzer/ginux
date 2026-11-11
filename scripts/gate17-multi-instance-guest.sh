#!/bin/sh
set -eu

runtime=/usr/local/libexec/ginux-runtime
helper=/usr/local/libexec/ginux-instance-helper
controller=/usr/local/bin/ginux
state=/var/lib/ginux/instances
rootfs=/var/tmp/ginux-gate17-rootfs.tar
recipe=/var/tmp/ginux-gate17-rocky.toml

wait_for_log() {
    wait_name=$1
    wait_pattern=$2
    wait_attempt=0
    while ! grep -Fq "$wait_pattern" "$state/$wait_name/console.log" 2>/dev/null; do
        wait_attempt=$((wait_attempt + 1))
        if test "$wait_attempt" -ge 120; then
            printf 'timed out waiting for %s in %s\n' "$wait_pattern" "$wait_name" >&2
            tail -n 80 "$state/$wait_name/console.log" >&2 || true
            return 1
        fi
        sleep 1
    done
}

stage_instance() {
    stage_name=$1
    stage_peer=$2
    stage_external=$3
    stage_directory=$state/$stage_name
    rm -rf "$stage_directory"
    mkdir -p "$stage_directory/root"
    tar -xf "$rootfs" -C "$stage_directory/root"
    "$runtime" config-merge "$recipe" \
        "instance.id=\"$stage_name\"" \
        "identity.hostname=\"$stage_name\"" \
        "contract.filesystem.deny=[\"/ginux-gate17/blocked-$stage_name\"]" \
        >"$stage_directory/config.toml"
    {
        printf 'forward\tprotocol=tcp external_port=%s instance_port=18180\n' \
            "$stage_external"
        printf 'contract\tlocked\n'
    } >"$stage_directory/deployment.state"
    chmod 600 "$stage_directory/config.toml" "$stage_directory/deployment.state"
    "$helper" prepare-root "$stage_directory/root"
    mkdir -p \
        "$stage_directory/root/ginux-gate17" \
        "$stage_directory/root/usr/local/sbin" \
        "$stage_directory/root/etc/systemd/system/multi-user.target.wants"
    printf '%s\n' "$stage_name" >"$stage_directory/root/ginux-gate17/$stage_name"
    install -m 755 /var/tmp/ginux-gate17-check \
        "$stage_directory/root/usr/local/sbin/ginux-gate17-check"
    cat >"$stage_directory/root/etc/systemd/system/ginux-gate17.service" <<EOF
[Unit]
Description=Ginux Gate 17 instance isolation check
After=systemd-journald.service

[Service]
Type=oneshot
Environment=GINUX_GATE17_NAME=$stage_name
Environment=GINUX_GATE17_PEER=$stage_peer
ExecStart=/usr/local/sbin/ginux-gate17-check
StandardOutput=journal+console
StandardError=journal+console

[Install]
WantedBy=multi-user.target
EOF
    ln -sf ../ginux-gate17.service \
        "$stage_directory/root/etc/systemd/system/multi-user.target.wants/ginux-gate17.service"
    chown -R 65534:65534 "$stage_directory/root/ginux-gate17" \
        "$stage_directory/root/usr/local/sbin/ginux-gate17-check" \
        "$stage_directory/root/etc/systemd/system/ginux-gate17.service" \
        "$stage_directory/root/etc/systemd/system/multi-user.target.wants/ginux-gate17.service"
}

cleanup() {
    "$helper" stop gate17-a >/dev/null 2>&1 || true
    "$helper" stop gate17-b >/dev/null 2>&1 || true
}
trap cleanup 0 HUP INT TERM

install -m 755 /var/tmp/ginux-gate17-runtime "$runtime"
install -m 755 /var/tmp/ginux-gate17-helper "$helper"
install -m 755 /var/tmp/ginux-gate17-controller "$controller"
mkdir -p "$state"

stage_instance gate17-a gate17-b 18181
stage_instance gate17-b gate17-a 18182

"$controller" start gate17-a
"$controller" start gate17-b

wait_for_log gate17-a 'GINUX_GATE17_INSTANCE: name=gate17-a'
wait_for_log gate17-b 'GINUX_GATE17_INSTANCE: name=gate17-b'
wait_for_log gate17-a 'Reached target multi-user.target - Multi-User System.'
wait_for_log gate17-b 'Reached target multi-user.target - Multi-User System.'
wait_for_log gate17-a 'GINUX_GATE17_CHECK: PASS name=gate17-a'
wait_for_log gate17-b 'GINUX_GATE17_CHECK: PASS name=gate17-b'

pid_a=$(awk 'NR == 1 { print $1 }' "$state/gate17-a/supervisor.pid")
pid_b=$(awk 'NR == 1 { print $1 }' "$state/gate17-b/supervisor.pid")
test -n "$pid_a" && test -n "$pid_b" && test "$pid_a" != "$pid_b"
kill -0 "$pid_a"
kill -0 "$pid_b"

list=$($controller list)
printf '%s\n' "$list"
printf '%s\n' "$list" | grep -Eq '^gate17-a[[:space:]]+running[[:space:]]+locked[[:space:]]+[0-9]+'
printf '%s\n' "$list" | grep -Eq '^gate17-b[[:space:]]+running[[:space:]]+locked[[:space:]]+[0-9]+'

if "$controller" start gate17-a >/var/tmp/gate17-duplicate.log 2>&1; then
    printf '%s\n' 'duplicate start unexpectedly succeeded' >&2
    exit 1
fi
grep -Fq 'gate17-a: already running' /var/tmp/gate17-duplicate.log
kill -0 "$pid_b"

test -f "$state/gate17-a/root/ginux-gate17/gate17-a-runtime"
test ! -e "$state/gate17-a/root/ginux-gate17/gate17-b-runtime"
test -f "$state/gate17-b/root/ginux-gate17/gate17-b-runtime"
test ! -e "$state/gate17-b/root/ginux-gate17/gate17-a-runtime"
test -z "$(find "$state/gate17-a/root/proc" "$state/gate17-a/root/sys" -mindepth 1 -print -quit)"
test -z "$(find "$state/gate17-b/root/proc" "$state/gate17-b/root/sys" -mindepth 1 -print -quit)"

shared_winners=$(grep -h 'GINUX_GATE17_INSTANCE:' \
    "$state/gate17-a/console.log" "$state/gate17-b/console.log" | \
    grep -c 'shared-port=winner')
shared_conflicts=$(grep -h 'GINUX_GATE17_INSTANCE:' \
    "$state/gate17-a/console.log" "$state/gate17-b/console.log" | \
    grep -c 'shared-port=conflict')
test "$shared_winners" -eq 1
test "$shared_conflicts" -eq 1
grep -Fq 'translated-port=bound' "$state/gate17-a/console.log"
grep -Fq 'translated-port=bound' "$state/gate17-b/console.log"

boot_a=$(sed -n 's/.*boot-id=\([^ ]*\).*/\1/p' "$state/gate17-a/console.log" | tail -n 1)
boot_b=$(sed -n 's/.*boot-id=\([^ ]*\).*/\1/p' "$state/gate17-b/console.log" | tail -n 1)
test -n "$boot_a" && test -n "$boot_b" && test "$boot_a" != "$boot_b"

"$controller" stop gate17-a
kill -0 "$pid_b"
test "$($controller status gate17-b | sed -n 's/^state: //p')" = running
"$controller" unlock gate17-a
grep -Fq 'contract=unlocked' "$state/gate17-a/deployment.state" || \
    grep -Fq 'contract	unlocked' "$state/gate17-a/deployment.state"
grep -Fq 'contract	locked' "$state/gate17-b/deployment.state"
"$controller" lock gate17-a

"$controller" stop gate17-b
trap - 0 HUP INT TERM
printf '%s\n' 'GATE17_MULTI_INSTANCE: PASS'
