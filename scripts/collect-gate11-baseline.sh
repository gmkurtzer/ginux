#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

output_dir=${GINUX_BENCH_OUTPUT_DIR:-build/gate11}
duration=${GINUX_BENCH_DURATION:-1}
repetitions=${GINUX_BENCH_REPETITIONS:-3}
allocations=${GINUX_BENCH_VCPUS:-"1 2 4"}
workloads=${GINUX_BENCH_WORKLOADS:-"cpu memory threads mutex"}
audit_mode=${GINUX_BENCH_AUDIT_MODE:-stderr}
supervisor_cpu=${GINUX_BENCH_SUPERVISOR_CPU:-}
commit=$(git rev-parse HEAD)
tree_state=clean
test -z "$(git status --porcelain)" || tree_state=dirty
mkdir -p "$output_dir"

test "$(rocky_ssh nproc)" -ge 4 || {
    printf '%s\n' 'Gate 11 requires a Rocky host with at least 4 vCPUs' >&2
    exit 1
}

./scripts/install-ginux-host.sh >/dev/null
rocky_scp config/instance-gate11-benchmark.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/instance-gate11-benchmark.toml"
rocky_scp scripts/gate11-guest-setup.sh scripts/gate11-benchmark-guest.sh \
    scripts/gate11-sysbench-command.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/"

rocky_ssh 'chmod 0755 /var/tmp/gate11-guest-setup.sh /var/tmp/gate11-benchmark-guest.sh /var/tmp/gate11-sysbench-command.sh; /var/tmp/gate11-guest-setup.sh'
rocky_ssh "GINUX_BENCH_DURATION='$duration' GINUX_BENCH_REPETITIONS='$repetitions' GINUX_BENCH_VCPUS='$allocations' GINUX_BENCH_WORKLOADS='$workloads' GINUX_BENCH_AUDIT_MODE='$audit_mode' GINUX_BENCH_SUPERVISOR_CPU='$supervisor_cpu' /var/tmp/gate11-benchmark-guest.sh"

rocky_scp "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate11-results.tsv" "$output_dir/results.tsv"
rocky_ssh 'tar -C /var/tmp -czf /var/tmp/gate11-raw.tar.gz gate11-raw'
rocky_scp "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate11-raw.tar.gz" "$output_dir/raw.tar.gz"
rocky_ssh 'sudo install -m 0644 /var/lib/ginux/instances/gate11-benchmark/audit.log /var/tmp/gate11-final-audit.log'
rocky_scp "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate11-final-audit.log" "$output_dir/final-audit.log"

{
    printf 'ginux_commit=%s\n' "$commit"
    printf 'ginux_working_tree=%s\n' "$tree_state"
    printf 'benchmark_duration_seconds=%s\n' "$duration"
    printf 'repetitions=%s\n' "$repetitions"
    printf 'warmups_per_row=1\n'
    printf 'memory_limit_bytes=1073741824\n'
    printf 'cpu_affinity=logical CPUs 0 through allocation-1\n'
    printf 'ordering=alternating native/Ginux by repetition\n'
    printf 'audit_mode=%s\n' "$audit_mode"
    printf 'ginux_supervisor_cpu=%s\n' "${supervisor_cpu:-shared-with-instance}"
    # Expansion belongs to the remote shell.
    # shellcheck disable=SC2016
    rocky_ssh 'printf "host_kernel=%s\n" "$(uname -r)"; printf "host_arch=%s\n" "$(uname -m)"; printf "host_vcpus=%s\n" "$(nproc)"; printf "host_memory_kib=%s\n" "$(awk "/^MemTotal:/ { print \$2 }" /proc/meminfo)"; printf "sysbench_nevra=%s\n" "$(rpm -q sysbench)"; printf "sysbench_sha256=%s\n" "$(sha256sum /usr/bin/sysbench | awk "{ print \$1 }")"; printf "rocky_release=%s\n" "$(cat /etc/rocky-release)"'
    printf 'instance_image=%s\n' 'docker.io/rockylinux/rockylinux@sha256:7a18ce2c9c1e0d8735f3e72685f9666150af25036e7f7b5ee4eede3754496f8c'
    printf 'instance_config_sha256=%s\n' "$(sha256sum config/instance-gate11-benchmark.toml | awk '{ print $1 }')"
} >"$output_dir/metadata.txt"

./scripts/bench-report.sh "$output_dir/results.tsv" "$output_dir/metadata.txt" "$output_dir/report.md"
cat "$output_dir/report.md"
