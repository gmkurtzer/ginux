#!/bin/sh
set -eu

source_dir=${1:?usage: gate16-kernel-assist-guest.sh SOURCE_DIR}
cgroup=/sys/fs/cgroup/ginux-gate16
cgroup_alt=/sys/fs/cgroup/ginux-gate16-alt
bpf_programs=/sys/fs/bpf/ginux16
bpf_maps=/sys/fs/bpf/ginux16-maps

cleanup() {
    set +e
    sudo "$source_dir/test-control" --clear >/dev/null 2>&1
    sudo rmmod ginux_control >/dev/null 2>&1
    sudo rm -rf "$bpf_programs" "$bpf_maps"
    sudo rmdir "$cgroup" "$cgroup_alt" >/dev/null 2>&1
}
trap cleanup 0 HUP INT TERM

cd "$source_dir"
make clean >/dev/null 2>&1 || true
make >/dev/null
gcc -O2 -Wall -Wextra -Werror -o test-control test-control.c
gcc -O2 -Wall -Wextra -Werror -o raw-mkdirat raw-mkdirat.c
gcc -nostdlib -static -Wl,--build-id=none -o raw-mkdirat-static raw-mkdirat.S
gcc -O2 -Wall -Wextra -Werror -pthread -o transition-test transition-test.c
gcc -O2 -Wall -Wextra -Werror -o bench-yield bench-yield.c
gcc -O2 -Wall -Wextra -Werror -o bench-mkdirat bench-mkdirat.c
gcc -O2 -Wall -Wextra -Werror -o bench-deny bench-deny.c
gcc -O2 -Wall -Wextra -Werror -o landlock-probe landlock-probe.c
clang -target bpf -D__TARGET_ARCH_x86 -O2 -g -Wall -Werror \
    -c ginux_lsm.bpf.c -o ginux_lsm.bpf.o

sudo mkdir -p "$cgroup" "$cgroup_alt"
landlock=$(./landlock-probe 2>/dev/null || true)
test -n "$landlock"
printf '%s\n' "$landlock"

clear_paths() {
    sudo rm -rf /tmp/ginux-parent-raw /tmp/ginux-thread-raw \
        /tmp/ginux-child-raw /tmp/ginux-static-raw /tmp/ginux-after-detach
}

run_transitions() {
    sudo sh -c "echo \$\$ > '$cgroup/cgroup.procs'; \
        exec '$source_dir/transition-test' '$source_dir/raw-mkdirat-static'"
    test ! -e /tmp/ginux-parent-raw
    test ! -e /tmp/ginux-thread-raw
    test ! -e /tmp/ginux-child-raw
    test ! -e /tmp/ginux-static-raw
}

cgroup_key() {
    identifier=$(stat -c %i "$cgroup")
    value=$identifier
    count=0
    key=
    while test "$count" -lt 8; do
        key="$key $(printf '%02x' "$((value & 255))")"
        value=$((value >> 8))
        count=$((count + 1))
    done
    printf '%s\n' "$key"
}

load_bpf_policy() {
    sudo mkdir -p "$bpf_programs" "$bpf_maps"
    sudo bpftool prog loadall ginux_lsm.bpf.o "$bpf_programs" \
        pinmaps "$bpf_maps" autoattach
    # shellcheck disable=SC2046
    sudo bpftool map update pinned "$bpf_maps/blocked_cgroups" \
        key hex $(cgroup_key) value hex 01 00 00 00
}

clear_paths
sudo insmod ./ginux_control.ko
if sudo ./test-control /tmp >/dev/null 2>&1; then
    printf '%s\n' 'module accepted a non-cgroup policy identifier' >&2
    exit 1
fi
sudo ./test-control "$cgroup"
run_transitions
sudo ./test-control "$cgroup_alt" >/dev/null
sudo sh -c "echo \$\$ > '$cgroup/cgroup.procs'; \
    exec '$source_dir/raw-mkdirat' /tmp/ginux-replaced-policy"
test -d /tmp/ginux-replaced-policy
sudo rm -rf /tmp/ginux-replaced-policy
if sudo sh -c "echo \$\$ > '$cgroup_alt/cgroup.procs'; \
    exec '$source_dir/raw-mkdirat' /tmp/ginux-alt-policy"; then
    printf '%s\n' 'replacement cgroup escaped module policy' >&2
    exit 1
fi
test ! -e /tmp/ginux-alt-policy
if sudo rmmod ginux_control >/dev/null 2>&1; then
    printf '%s\n' 'active module policy permitted unload' >&2
    exit 1
fi
grep -q '^ginux_control ' /proc/modules
sudo ./test-control --clear
sudo rmmod ginux_control
printf '%s\n' 'GATE16_MODULE: raw=denied thread=denied fork=denied exec=denied stale=rejected replacement=atomic unload=fail-closed'

clear_paths
load_bpf_policy
run_transitions
sudo rm -f "$bpf_programs/ginux_path_mkdir"
sudo sh -c "echo \$\$ > '$cgroup/cgroup.procs'; \
    exec '$source_dir/raw-mkdirat' /tmp/ginux-after-detach"
test -d /tmp/ginux-after-detach
sudo rm -rf "$bpf_programs" "$bpf_maps" /tmp/ginux-after-detach
printf '%s\n' 'GATE16_BPF_LSM: raw=denied thread=denied fork=denied exec=denied pinned=fail-closed detach=PASS'

results=/home/ginux/gate16-results
rm -rf "$results"
mkdir -p "$results"
{
    printf 'kernel=%s\n' "$(uname -r)"
    printf 'architecture=%s\n' "$(uname -m)"
    printf 'vcpus=%s\n' "$(getconf _NPROCESSORS_ONLN)"
    printf 'landlock=%s\n' "$landlock"
    printf 'module_sha256=%s\n' "$(sha256sum ginux_control.ko | awk '{print $1}')"
    printf 'bpf_sha256=%s\n' "$(sha256sum ginux_lsm.bpf.o | awk '{print $1}')"
} >"$results/provenance.txt"

benchmark_yield() {
    label=$1
    output=$2
    : >"$output"
    taskset -c 0 ./bench-yield 100000 >/dev/null
    repetition=1
    while test "$repetition" -le 5; do
        line=$(sudo sh -c "echo \$\$ > '$cgroup/cgroup.procs'; \
            exec taskset -c 0 '$source_dir/bench-yield' 500000")
        rate=$(printf '%s\n' "$line" | sed -n \
            's/.*events_per_second=\([0-9.]*\).*/\1/p')
        test -n "$rate"
        printf '%s\t%s\n' "$repetition" "$rate" >>"$output"
        repetition=$((repetition + 1))
    done
    mean=$(awk '{ total += $2 } END { printf "%.3f", total / NR }' "$output")
    printf 'GATE16_BENCH: workload=yield mode=%s mean_events_per_second=%s\n' "$label" "$mean"
}

benchmark_mkdir() {
    label=$1
    output=$2
    : >"$output"
    taskset -c 0 ./bench-mkdirat 100 >/dev/null
    repetition=1
    while test "$repetition" -le 5; do
        line=$(taskset -c 0 ./bench-mkdirat 5000)
        rate=$(printf '%s\n' "$line" | sed -n \
            's/.*operations_per_second=\([0-9.]*\).*/\1/p')
        test -n "$rate"
        printf '%s\t%s\n' "$repetition" "$rate" >>"$output"
        repetition=$((repetition + 1))
    done
    mean=$(awk '{ total += $2 } END { printf "%.3f", total / NR }' "$output")
    printf 'GATE16_BENCH: workload=mkdir-allowed mode=%s mean_operations_per_second=%s\n' "$label" "$mean"
}

benchmark_deny() {
    label=$1
    output=$2
    mechanism=$3
    : >"$output"
    repetition=1
    while test "$repetition" -le 5; do
        if test "$mechanism" = seccomp; then
            line=$(taskset -c 0 ./bench-deny 100000 seccomp)
        else
            line=$(sudo sh -c "echo \$\$ > '$cgroup/cgroup.procs'; \
                exec taskset -c 0 '$source_dir/bench-deny' 100000 external")
        fi
        rate=$(printf '%s\n' "$line" | sed -n \
            's/.*denials_per_second=\([0-9.]*\).*/\1/p')
        test -n "$rate"
        printf '%s\t%s\n' "$repetition" "$rate" >>"$output"
        repetition=$((repetition + 1))
    done
    mean=$(awk '{ total += $2 } END { printf "%.3f", total / NR }' "$output")
    printf 'GATE16_BENCH: workload=mkdir-denied mode=%s mean_denials_per_second=%s\n' "$label" "$mean"
}

benchmark_yield native "$results/yield-native.tsv"
benchmark_mkdir native "$results/mkdir-native.tsv"
benchmark_deny seccomp "$results/deny-seccomp.tsv" seccomp

sudo insmod ./ginux_control.ko
sudo ./test-control "$cgroup" >/dev/null
benchmark_yield module "$results/yield-module.tsv"
benchmark_mkdir module "$results/mkdir-module.tsv"
benchmark_deny module "$results/deny-module.tsv" external
sudo ./test-control --clear >/dev/null
sudo rmmod ginux_control

load_bpf_policy
benchmark_yield bpf-lsm "$results/yield-bpf-lsm.tsv"
benchmark_mkdir bpf-lsm "$results/mkdir-bpf-lsm.tsv"
benchmark_deny bpf-lsm "$results/deny-bpf-lsm.tsv" external
sudo rm -rf "$bpf_programs" "$bpf_maps"

native_mean=$(awk '{ total += $2 } END { print total / NR }' \
    "$results/yield-native.tsv")
module_mean=$(awk '{ total += $2 } END { print total / NR }' \
    "$results/yield-module.tsv")
bpf_mean=$(awk '{ total += $2 } END { print total / NR }' \
    "$results/yield-bpf-lsm.tsv")
awk -v value="$module_mean" -v native="$native_mean" \
    'BEGIN { ratio = value / native; printf "GATE16_RATIO: mode=module ratio=%.4fx\n", ratio; exit ratio < 0.94 }'
awk -v value="$bpf_mean" -v native="$native_mean" \
    'BEGIN { ratio = value / native; printf "GATE16_RATIO: mode=bpf-lsm ratio=%.4fx\n", ratio; exit ratio < 0.94 }'

printf '%s\n' 'GATE16_KERNEL_ASSIST: PASS'
