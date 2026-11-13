#!/bin/sh
set -eu

state_root=/var/lib/ginux
image_root=/usr/local/libexec/ginux-image
cache=$state_root/images/gate125-fixture
rootfs=$cache/rootfs.tar
name=maintenance
directory=$state_root/instances/$name

rm -rf "$state_root/instances" "$state_root/work" "$cache"
mkdir -p "$state_root/instances" "$state_root/work" "$cache" "$image_root"

fixture=$(mktemp -d /var/tmp/ginux-gate125-root.XXXXXX)
cleanup() {
    test -z "${fake_pid:-}" || kill "$fake_pid" 2>/dev/null || true
    rm -rf "$fixture" "$directory"
}
trap cleanup 0 HUP INT TERM
mkdir -p "$fixture/sbin" "$fixture/etc" "$fixture/usr/lib/systemd/system"
printf '#!/bin/sh\nexit 0\n' >"$fixture/sbin/init"
chmod 0755 "$fixture/sbin/init"
printf 'root:*:0:0:99999:7:::\n' >"$fixture/etc/shadow"
tar -cf "$rootfs" -C "$fixture" .

cat >"$image_root/ensure-oci-image.sh" <<EOF
#!/bin/sh
printf '%s\n' '$rootfs' >"\$GINUX_OCI_RESULT_FILE"
exit 0
EOF
chmod 0755 "$image_root/ensure-oci-image.sh"

/usr/local/bin/ginux import "$name" /var/tmp/gate12-recipe.toml --network default
test "$(awk -F '\\t' '$1 == "contract" && $2 == "locked" { count++ } END { print count + 0 }' "$directory/deployment.state")" -eq 1
/usr/local/bin/ginux status "$name" | grep -Fqx 'contract: locked'
/usr/local/bin/ginux list | grep -Eq '^maintenance[[:space:]]+stopped[[:space:]]+locked[[:space:]]+-$'
config_before=$(sha256sum "$directory/config.toml" | awk '{ print $1 }')
cp "$directory/runtime.toml" /var/tmp/ginux-gate125-locked.toml
grep -Fqx 'deny = ["/ginux-demo/read-only"]' "$directory/runtime.toml"

/usr/local/bin/ginux unlock "$name"
test "$(awk -F '\\t' '$1 == "contract" && $2 == "unlocked" { count++ } END { print count + 0 }' "$directory/deployment.state")" -eq 1
/usr/local/bin/ginux status "$name" | grep -Fqx 'contract: unlocked'
/usr/local/bin/ginux list | grep -Eq '^maintenance[[:space:]]+stopped[[:space:]]+unlocked[[:space:]]+-$'
test "$config_before" = "$(sha256sum "$directory/config.toml" | awk '{ print $1 }')"
grep -Fqx 'allow = ["*"]' "$directory/runtime.toml"
grep -Fqx 'deny = []' "$directory/runtime.toml"
grep -Fqx 'write = ["/"]' "$directory/runtime.toml"
grep -Fqx 'create = ["/"]' "$directory/runtime.toml"
grep -Fqx 'setuid = ["/"]' "$directory/runtime.toml"
grep -Fqx 'proc_write = []' "$directory/runtime.toml"
grep -Fqx 'sys_write = []' "$directory/runtime.toml"

set +e
start_output=$(/usr/local/bin/ginux start "$name" 2>&1)
start_status=$?
set -e
printf '%s\n' "$start_output" |
    grep -Fqx 'WARNING: maintenance is starting with its contract unlocked for maintenance.'
if test "$start_status" -eq 0; then
    /usr/local/bin/ginux stop "$name" >/dev/null
fi

/usr/local/bin/ginux unlock "$name" |
    grep -Fqx 'Instance "maintenance" contract is already unlocked.'

sleep 60 &
fake_pid=$!
fake_start=$(awk '{ print $22 }' "/proc/$fake_pid/stat")
printf '%s %s\n' "$fake_pid" "$fake_start" >"$directory/supervisor.pid"
if /usr/local/bin/ginux lock "$name" >/dev/null 2>&1; then
    printf '%s\n' 'running instance contract lock was incorrectly changed' >&2
    exit 1
fi
test "$(awk -F '\\t' '$1 == "contract" && $2 == "unlocked" { count++ } END { print count + 0 }' "$directory/deployment.state")" -eq 1
rm -f "$directory/supervisor.pid"
kill "$fake_pid"
wait "$fake_pid" 2>/dev/null || true
fake_pid=

/usr/local/bin/ginux lock "$name"
test "$(awk -F '\\t' '$1 == "contract" && $2 == "locked" { count++ } END { print count + 0 }' "$directory/deployment.state")" -eq 1
cmp /var/tmp/ginux-gate125-locked.toml "$directory/runtime.toml"
test "$config_before" = "$(sha256sum "$directory/config.toml" | awk '{ print $1 }')"

# A pre-Gate-12.5 deployment file has no contract record and must fail closed.
sed -i '/^contract/d' "$directory/deployment.state"
/usr/local/bin/ginux status "$name" | grep -Fqx 'contract: locked'

printf 'unknown\tvalue\n' >>"$directory/deployment.state"
malformed_before=$(sha256sum "$directory/deployment.state" | awk '{ print $1 }')
if /usr/local/bin/ginux unlock "$name" >/dev/null 2>&1; then
    printf '%s\n' 'malformed deployment state was incorrectly unlocked' >&2
    exit 1
fi
test "$malformed_before" = "$(sha256sum "$directory/deployment.state" | awk '{ print $1 }')"

printf '%s\n' 'GATE12.5_LOCK: PASS (default locked, persistent maintenance mode, atomic relock, running and malformed rejection)'
