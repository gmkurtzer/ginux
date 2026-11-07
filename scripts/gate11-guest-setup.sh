#!/bin/sh
set -eu

instance=gate11-benchmark
config=/var/tmp/instance-gate11-benchmark.toml
runtime=/usr/local/libexec/ginux-runtime
directory=/var/lib/ginux/instances/$instance
sysbench_nevra=sysbench-1.0.20-19.el10_2.x86_64

if ! rpm -q --quiet "$sysbench_nevra"; then
    sudo dnf -y install epel-release >/dev/null
    sudo dnf -y install "$sysbench_nevra" >/dev/null
fi
test "$(rpm -q sysbench)" = "$sysbench_nevra"

if test -d "$directory"; then
    sudo /usr/local/libexec/ginux-instance-helper stop "$instance" >/dev/null || true
    sudo rm -rf "$directory"
fi
ginux instance create "$instance" "$config"

root=$directory/root
dependencies=$(ldd /usr/bin/sysbench | awk '
    /=> \// { print $3 }
    $1 ~ /^\// { print $1 }
' | LC_ALL=C sort -u)
sudo install -D -m 755 /usr/bin/sysbench "$root/usr/bin/sysbench"
for dependency in $dependencies; do
    sudo install -D -m 755 "$dependency" "$root$dependency"
done
if test -d /usr/share/sysbench; then
    sudo mkdir -p "$root/usr/share"
    sudo cp -a /usr/share/sysbench "$root/usr/share/"
fi
sudo chown -R 65534:65534 "$root/usr/bin/sysbench" "$root/usr/share/sysbench" 2>/dev/null || true

host_hash=$(sha256sum /usr/bin/sysbench | awk '{ print $1 }')
instance_hash=$(sudo sha256sum "$root/usr/bin/sysbench" | awk '{ print $1 }')
test "$host_hash" = "$instance_hash"
{
    printf 'sysbench_nevra=%s\n' "$sysbench_nevra"
    printf 'sysbench_sha256=%s\n' "$host_hash"
    printf 'host_kernel=%s\n' "$(uname -r)"
    printf 'host_arch=%s\n' "$(uname -m)"
} | sudo tee "$directory/benchmark-meta" >/dev/null

printf 'GATE11_SETUP: sysbench=%s sha256=%s instance=%s\n' \
    "$sysbench_nevra" "$host_hash" "$instance"
