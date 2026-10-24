#!/usr/bin/env bash

set -euo pipefail

benchmark_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=/dev/null
source "$benchmark_root/tools/qemu/common.sh"
# shellcheck source=/dev/null
source "$benchmark_root/profiles/benchmarks/rocky10-single-vcpu.env"

benchmark_dir="${GINUX_BENCHMARK_DIR:-$benchmark_root/build/benchmark/rocky10-single-vcpu}"
export benchmark_root benchmark_dir

hash_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

benchmark_accelerator() {
    printf '%s\n' "${QEMU_ACCEL:-$(default_accelerator)}"
}

benchmark_cpu() {
    local accelerator=$1
    if [[ -n "${QEMU_CPU:-}" ]]; then
        printf '%s\n' "$QEMU_CPU"
    elif [[ "$accelerator" == kvm ]]; then
        printf 'host,+smep,+smap,+umip\n'
    else
        printf 'max,+smep,+smap,+umip\n'
    fi
}

now_ms() {
    perl -MTime::HiRes=time -e 'printf "%.0f\n", time() * 1000'
}

require_benchmark_tool() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "benchmark: missing required tool: $1" >&2
        exit 1
    }
}

stop_process() {
    local pid=${1:-}
    [[ "$pid" =~ ^[0-9]+$ ]] || return 0
    kill -0 "$pid" 2>/dev/null || return 0
    kill "$pid" 2>/dev/null || true
    for _ in {1..20}; do
        kill -0 "$pid" 2>/dev/null || return 0
        sleep 0.1
    done
    kill -KILL "$pid" 2>/dev/null || true
}
