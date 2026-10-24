#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tools/benchmarks/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

require_benchmark_tool docker
mkdir -p "$benchmark_dir"
rootfs="$benchmark_dir/sysbench-minimal-rootfs.tar"
volume="$benchmark_dir/ginux-volume.raw"

docker build --platform linux/amd64 -f "$benchmark_root/tools/benchmarks/sysbench.Dockerfile" \
    -t "$GINUX_SYSBENCH_IMAGE" "$benchmark_root"
docker run --rm --platform linux/amd64 "$GINUX_SYSBENCH_IMAGE" /bin/sh -c '
    set -eu
    directories="bin dev etc home lib lib64 mnt opt proc root run sbin srv sys tmp usr usr/bin usr/lib usr/lib64 var"
    files="bin/sh usr/bin/sysbench"
    dependencies=$(ldd /bin/sh /usr/bin/sysbench | awk '\''/=> \// { print $(NF - 1) } $1 ~ /^\// && $1 !~ /:$/ { print $1 }'\'' | sort -u)
    for dependency in $dependencies; do files="$files ${dependency#/}"; done
    [ ! -f /etc/ld.so.cache ] || files="$files etc/ld.so.cache"
    tar -C / --format=ustar --dereference --no-recursion -cf - $directories $files
' >"$rootfs"

cargo run --quiet --package ginux-manifest-tool --bin seed-benchmark-cache -- \
    "$volume" "$rootfs" "$GINUX_SYSBENCH_IMAGE"
echo "Prepared Ginux sysbench rootfs and cache: $rootfs"
