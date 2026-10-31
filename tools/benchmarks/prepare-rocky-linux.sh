#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tools/benchmarks/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

require_benchmark_tool curl
require_benchmark_tool python3
require_benchmark_tool qemu-img

qemu=$(find_qemu)
ovmf_code=$(find_ovmf_code)
ovmf_vars_template=$(find_ovmf_vars)
accelerator=$(benchmark_accelerator)
cpu_model=$(benchmark_cpu "$accelerator")
cloud_image="$benchmark_asset_dir/$ROCKY_CLOUD_IMAGE_NAME"
prepared_image="$benchmark_asset_dir/rocky-linux-prepared-v2.qcow2"
preparation_log="$benchmark_asset_dir/rocky-linux-prepare-v2.log"
seed_dir="$benchmark_asset_dir/nocloud-v2"
http_port="${GINUX_BENCHMARK_HTTP_PORT:-18080}"
http_pid=""
qemu_pid=""

cleanup() {
    stop_process "$qemu_pid"
    stop_process "$http_pid"
    if [[ "$http_pid" =~ ^[0-9]+$ ]]; then
        wait "$http_pid" 2>/dev/null || true
    fi
}
trap cleanup EXIT

mkdir -p "$benchmark_asset_dir" "$seed_dir"

if [[ ! -f "$cloud_image" ]] || [[ "$(hash_file "$cloud_image")" != "$ROCKY_CLOUD_IMAGE_SHA256" ]]; then
    partial="$cloud_image.partial"
    echo "Downloading pinned Rocky cloud image..."
    curl --fail --location --continue-at - --output "$partial" "$ROCKY_CLOUD_IMAGE_URL"
    actual=$(hash_file "$partial")
    [[ "$actual" == "$ROCKY_CLOUD_IMAGE_SHA256" ]] || {
        echo "benchmark: cloud image checksum mismatch: $actual" >&2
        exit 1
    }
    mv "$partial" "$cloud_image"
fi

[[ "$(hash_file "$cloud_image")" == "$ROCKY_CLOUD_IMAGE_SHA256" ]] || {
    echo "benchmark: pinned cloud image verification failed" >&2
    exit 1
}

if [[ -f "$prepared_image" ]] && qemu-img check -q "$prepared_image"; then
    echo "Prepared Linux benchmark image already exists: $prepared_image"
    exit 0
fi

rm -f "$prepared_image"
qemu-img create -q -f qcow2 -F qcow2 -b "$cloud_image" "$prepared_image"

cat >"$seed_dir/meta-data" <<'EOF'
instance-id: ginux-rocky-benchmark-v1
local-hostname: rocky-linux-benchmark
EOF

cat >"$seed_dir/user-data" <<EOF
#cloud-config
ssh_pwauth: false
write_files:
  - path: /usr/local/sbin/ginux-benchmark
    permissions: '0755'
    content: |
      #!/usr/bin/bash
      output=/dev/ttyS0
      if [ -e /dev/virtio-ports/org.ginux.benchmark ]; then
          output=/dev/virtio-ports/org.ginux.benchmark
      fi
      exec 3>"\$output"
      threads=1
      threads_file=/sys/firmware/qemu_fw_cfg/by_name/opt/ginux/threads/raw
      if [ -r "\$threads_file" ]; then
          candidate=\$(tr -dc '0-9' <"\$threads_file")
          case "\$candidate" in 1|2|4) threads=\$candidate ;; esac
      fi
      echo GINUX_BENCH_READY >&3
      {
          printf 'GINUX_BENCH_KERNEL_TO_MULTI_USER_SECONDS '
          cut -d' ' -f1 /proc/uptime
          printf 'GINUX_BENCH_SYSBENCH_SHA256 '
          sha256sum /usr/bin/sysbench | awk '{print \$1}'
          echo "GINUX_BENCH_THREADS \$threads"
          for workload in cpu memory threads mutex; do
              run=1
              while [ \$run -le ${BENCHMARK_WORKLOAD_RUNS} ]; do
                  echo "GINUX_BENCH_WORKLOAD_BEGIN \$workload \$run"
                  case \$workload in
                      cpu) sysbench cpu --threads=\$threads --time=${BENCHMARK_WORKLOAD_SECONDS} --cpu-max-prime=20000 run ;;
                      memory) sysbench memory --threads=\$threads --time=${BENCHMARK_WORKLOAD_SECONDS} --memory-total-size=100G run ;;
                      threads) sysbench threads --threads=\$threads --time=${BENCHMARK_WORKLOAD_SECONDS} run ;;
                      mutex) sysbench mutex --threads=\$threads --time=${BENCHMARK_WORKLOAD_SECONDS} run ;;
                  esac
                  echo "GINUX_BENCH_WORKLOAD_END \$workload \$run \$?"
                  run=\$((run + 1))
              done
          done
      } >/var/tmp/ginux-benchmark-results 2>&1
      cat /var/tmp/ginux-benchmark-results >&3
      echo GINUX_BENCH_COMPLETE >&3
      sync
      sleep 1
      systemctl poweroff
  - path: /etc/systemd/system/ginux-benchmark.service
    permissions: '0644'
    content: |
      [Unit]
      Description=Ginux comparative benchmark workload
      After=multi-user.target
      [Service]
      Type=simple
      ExecStart=/usr/local/sbin/ginux-benchmark
  - path: /etc/systemd/system/ginux-benchmark.timer
    permissions: '0644'
    content: |
      [Unit]
      Description=Run benchmark after normal userspace boot
      [Timer]
      OnBootSec=1s
      Unit=ginux-benchmark.service
      [Install]
      WantedBy=timers.target
runcmd:
  - [dnf, -y, install, epel-release]
  - [dnf, -y, install, sysbench]
  - [systemctl, enable, ginux-benchmark.timer]
  - [sh, -c, 'printf "GINUX_LINUX_PREPARED\\n" >/dev/ttyS0']
  - [systemctl, poweroff]
EOF

python3 -m http.server "$http_port" --bind 127.0.0.1 --directory "$seed_dir" \
    >"$benchmark_dir/nocloud-http.log" 2>&1 &
http_pid=$!
sleep 1
kill -0 "$http_pid" 2>/dev/null || {
    echo "benchmark: failed to start NoCloud HTTP server on port $http_port" >&2
    exit 1
}

cp "$ovmf_vars_template" "$benchmark_asset_dir/rocky-linux-prepare-v2-vars.fd"
: >"$preparation_log"
echo "Preparing Rocky Linux baseline under QEMU/$accelerator; this installs EPEL and sysbench once."
"$qemu" \
    -machine "$BENCHMARK_MACHINE,accel=$accelerator" \
    -cpu "$cpu_model" -smp "$BENCHMARK_VCPUS" -m "$BENCHMARK_MEMORY" \
    -display none -monitor none -serial stdio -no-reboot \
    -drive "if=pflash,format=raw,readonly=on,file=$ovmf_code" \
    -drive "if=pflash,format=raw,file=$benchmark_asset_dir/rocky-linux-prepare-v2-vars.fd" \
    -drive "if=none,id=linux-root,format=qcow2,file=$prepared_image" \
    -device "virtio-blk-pci,drive=linux-root,bootindex=1" \
    -netdev user,id=linux-net,restrict=off \
    -device virtio-net-pci,netdev=linux-net \
    -smbios "type=1,serial=ds=nocloud-net;s=http://10.0.2.2:${http_port}/" \
    >"$preparation_log" 2>&1 &
qemu_pid=$!

for _ in {1..1800}; do
    grep -Fq GINUX_LINUX_PREPARED "$preparation_log" && break
    kill -0 "$qemu_pid" 2>/dev/null || break
    sleep 1
done

wait "$qemu_pid" || true
qemu_pid=""
grep -Fq GINUX_LINUX_PREPARED "$preparation_log" || {
    tail -n 120 "$preparation_log" >&2
    echo "benchmark: Rocky Linux preparation did not complete" >&2
    exit 1
}
qemu-img check -q "$prepared_image"
echo "Prepared and verified $prepared_image"
