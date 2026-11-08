#!/bin/sh
set -eu

state_root=/var/lib/ginux
image_root=/usr/local/libexec/ginux-image
cache=$state_root/images/rocky-10-ubi-init-amd64
rootfs=$cache/rocky-10-ubi-init-amd64-rootfs.tar

rm -rf "$state_root/instances" "$state_root/work" "$cache"
mkdir -p "$state_root/instances" "$state_root/work" "$cache" "$image_root"

fixture=$(mktemp -d /var/tmp/ginux-gate12-root.XXXXXX)
cleanup() {
    rm -rf "$fixture" "$state_root/instances/gate12" "$state_root/instances/defaulted"
}
trap cleanup 0 HUP INT TERM
mkdir -p "$fixture/sbin" "$fixture/etc" "$fixture/usr/lib/systemd/system"
printf '#!/bin/sh\nexit 0\n' >"$fixture/sbin/init"
chmod 0755 "$fixture/sbin/init"
printf 'root:*:0:0:99999:7:::\n' >"$fixture/etc/shadow"
tar -cf "$rootfs" -C "$fixture" .

cat >"$image_root/ensure-rocky-oci.sh" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod 0755 "$image_root/ensure-rocky-oci.sh"
: >"$image_root/rocky-systemd-oci.env"

/usr/local/bin/ginux instance create gate12 /var/tmp/gate12-recipe.toml \
    --network lo \
    --forward protocol=tcp,external_port=18080,instance_port=80 \
    --forward protocol=tcp,external_port=18443,instance_port=443

directory=$state_root/instances/gate12
test -f "$directory/config.toml"
test -f "$directory/deployment.state"
test -f "$directory/runtime.toml"
! grep -Eq '^(interface|source|port_forwards)[[:space:]]*=' "$directory/config.toml"
test "$(awk -F '\t' '$1 == "network" && $2 == "lo" { count++ } END { print count + 0 }' "$directory/deployment.state")" -eq 1
test "$(awk -F '\t' '$1 == "forward" { count++ } END { print count + 0 }' "$directory/deployment.state")" -eq 2
grep -Fqx 'interface = "lo"' "$directory/runtime.toml"
grep -Fqx 'port_forwards = ["tcp/lo/18080/80", "tcp/lo/18443/443"]' "$directory/runtime.toml"

/usr/local/bin/ginux instance set gate12 \
    --network default \
    --forward protocol=tcp,external_port=28080,instance_port=80 \
    --forward protocol=udp,external_port=25353,instance_port=53
! grep -q '^network' "$directory/deployment.state"
test "$(awk -F '\t' '$1 == "forward" { count++ } END { print count + 0 }' "$directory/deployment.state")" -eq 2
! grep -q '^interface[[:space:]]*=' "$directory/runtime.toml"
grep -Fqx 'port_forwards = ["tcp/default/28080/80", "udp/default/25353/53"]' "$directory/runtime.toml"

before=$(sha256sum "$directory/deployment.state" | awk '{ print $1 }')
if /usr/local/bin/ginux instance set gate12 \
    --network lo \
    --forward protocol=tcp,external_port=38080,instance_port=80 \
    --forward protocol=tcp,external_port=38080,instance_port=81; then
    printf '%s\n' 'conflicting multi-flag update was incorrectly accepted' >&2
    exit 1
fi
after=$(sha256sum "$directory/deployment.state" | awk '{ print $1 }')
test "$before" = "$after"

if /usr/local/bin/ginux instance set gate12 --forward none \
    --forward protocol=tcp,external_port=48080,instance_port=80; then
    printf '%s\n' 'mixed forward reset and mapping were incorrectly accepted' >&2
    exit 1
fi
test "$before" = "$(sha256sum "$directory/deployment.state" | awk '{ print $1 }')"

/usr/local/bin/ginux instance create defaulted /var/tmp/gate12-recipe.toml \
    --network default --forward none
test ! -s "$state_root/instances/defaulted/deployment.state"

printf '%s\n' 'GATE12_CLI: PASS (repeatable create/set flags, portable recipe, default attachment, atomic failure)'
