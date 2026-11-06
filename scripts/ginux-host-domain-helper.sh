#!/bin/sh
set -eu

state_root=/var/lib/ginux
runtime=/usr/local/libexec/ginux-runtime
image_tools=/usr/local/libexec/ginux-image
image_config=$image_tools/rocky-systemd-oci.env
image_cache=$state_root/images/rocky-10-ubi-init-amd64
image_tar=$image_cache/rocky-10-ubi-init-amd64-rootfs.tar

usage() {
    printf '%s\n' 'usage: ginux domain create NAME --config FILE [--set KEY=TOML_VALUE]...' >&2
    printf '%s\n' '       ginux domain list' >&2
    printf '%s\n' '       ginux domain start NAME [--attach]' >&2
    printf '%s\n' '       ginux domain attach NAME' >&2
    printf '%s\n' '       ginux domain status NAME' >&2
    printf '%s\n' '       ginux domain stop NAME' >&2
    exit 64
}

validate_name() {
    case ${1-} in
        ''|*[!A-Za-z0-9_.-]*)
            printf 'invalid domain name: %s\n' "${1-}" >&2
            exit 64
            ;;
    esac
}

domain_dir() {
    printf '%s/domains/%s' "$state_root" "$1"
}

supervisor_identity() {
    supervisor_directory=$(domain_dir "$1")
    supervisor_file=$supervisor_directory/supervisor.pid
    test -f "$supervisor_file" || return 1
    supervisor_pid=
    supervisor_start=
    supervisor_extra=
    IFS=' ' read -r supervisor_pid supervisor_start supervisor_extra \
        <"$supervisor_file" || return 1
    test -z "$supervisor_extra" || return 1
    case $supervisor_pid:$supervisor_start in
        *[!0-9:]*|:*) return 1 ;;
    esac
    test -r "/proc/$supervisor_pid/stat" || return 1
    supervisor_state=$(awk '{ print $3 }' "/proc/$supervisor_pid/stat")
    supervisor_actual_start=$(awk '{ print $22 }' "/proc/$supervisor_pid/stat")
    test "$supervisor_state" != Z || return 1
    test "$supervisor_actual_start" = "$supervisor_start" || return 1
    printf '%s\n' "$supervisor_pid"
}

record_supervisor() {
    record_directory=$1
    record_pid=$2
    record_start=$(awk '{ print $22 }' "/proc/$record_pid/stat")
    printf '%s %s\n' "$record_pid" "$record_start" >"$record_directory/supervisor.pid"
}

record_runtime() {
    record_directory=$1
    record_pid=$2
    record_start=$(awk '{ print $22 }' "/proc/$record_pid/stat")
    printf '%s %s\n' "$record_pid" "$record_start" >"$record_directory/runtime.pid"
}

runtime_identity() {
    runtime_directory=$(domain_dir "$1")
    runtime_file=$runtime_directory/runtime.pid
    test -f "$runtime_file" || return 1
    runtime_pid=
    runtime_start=
    runtime_extra=
    IFS=' ' read -r runtime_pid runtime_start runtime_extra <"$runtime_file" || return 1
    test -z "$runtime_extra" || return 1
    case $runtime_pid:$runtime_start in
        *[!0-9:]*|:*) return 1 ;;
    esac
    test -r "/proc/$runtime_pid/stat" || return 1
    runtime_state=$(awk '{ print $3 }' "/proc/$runtime_pid/stat")
    runtime_actual_start=$(awk '{ print $22 }' "/proc/$runtime_pid/stat")
    test "$runtime_state" != Z || return 1
    test "$runtime_actual_start" = "$runtime_start" || return 1
    printf '%s\n' "$runtime_pid"
}

reported_state() {
    if supervisor_identity "$1" >/dev/null; then
        printf '%s' running
    else
        printf '%s' stopped
    fi
}

prepare_root() {
    root=$1
    mkdir -p \
        "$root/run" \
        "$root/run/host" \
        "$root/proc/1" \
        "$root/proc/self/fd" \
        "$root/proc/sys/fs" \
        "$root/proc/sys/kernel" \
        "$root/proc/sys/kernel/random" \
        "$root/proc/pressure" \
        "$root/sys/kernel/security" \
        "$root/sys/fs/cgroup/systemd" \
        "$root/sys/fs/cgroup/init.scope" \
        "$root/sys/fs/cgroup/system.slice" \
        "$root/sys/fs/cgroup/system.slice/console-getty.service" \
        "$root/sys/fs/cgroup/system.slice/systemd-user-sessions.service" \
        "$root/dev/shm" \
        "$root/dev/pts" \
        "$root/run/log/journal" \
        "$root/run/systemd/journal" \
        "$root/run/systemd/propagate/.os-release-stage" \
        "$root/usr/local/sbin"
    touch "$root/etc/resolv.conf"
    rm -f "$root/run/initctl"
    if test -e /etc/resolv.conf; then
        cp /etc/resolv.conf "$root/etc/resolv.conf"
    fi
    printf '%s\000' '/sbin/init --system --unit=multi-user.target' >"$root/proc/1/cmdline"
    printf '0::/\n' >"$root/proc/1/cgroup"
    printf '0::/\n' >"$root/proc/self/cgroup"
    printf 'cpu  1 0 1 100 0 0 0 0 0 0\nbtime 1\nprocesses 1\nprocs_running 1\nprocs_blocked 0\n' >"$root/proc/stat"
    printf '1.00 0.00\n' >"$root/proc/uptime"
    printf 'MemTotal:        1048576 kB\nMemFree:          524288 kB\nMemAvailable:     524288 kB\n' >"$root/proc/meminfo"
    printf 'none / proc rw,nosuid,nodev,noexec,relatime 0 0\n' >"$root/proc/mounts"
    printf '0.00 0.00 0.00 1/1 1\n' >"$root/proc/loadavg"
    : >"$root/proc/cpuinfo"
    printf 'Ginux userspace kernel 0.1.0 (Linux ABI personality 6.12.0-ginux-abi1; contract gate9-systemd; x86_64)\n' >"$root/proc/version"
    printf '9223372036854775807\n' >"$root/proc/sys/fs/file-max"
    printf '1048576\n' >"$root/proc/sys/fs/nr_open"
    printf '40\n' >"$root/proc/sys/kernel/cap_last_cap"
    printf 'ginux\n' >"$root/proc/sys/kernel/hostname"
    printf 'Linux\n' >"$root/proc/sys/kernel/ostype"
    printf '6.12.0-ginux-abi1\n' >"$root/proc/sys/kernel/osrelease"
    printf 'Ginux userspace kernel 0.1.0\n' >"$root/proc/sys/kernel/version"
    printf '4e45c53e-0e6a-4fb8-9f25-e0f11b40db9a\n' \
        >"$root/proc/sys/kernel/random/boot_id"
    printf 'some avg10=0.00 avg60=0.00 avg300=0.00 total=0\n' >"$root/proc/pressure/cpu"
    : >"$root/sys/kernel/security/lsm"
    printf 'cpu memory pids\n' >"$root/sys/fs/cgroup/cgroup.controllers"
    touch \
        "$root/sys/fs/cgroup/cgroup.procs" \
        "$root/sys/fs/cgroup/cgroup.subtree_control" \
        "$root/sys/fs/cgroup/init.scope/cgroup.procs" \
        "$root/sys/fs/cgroup/system.slice/cgroup.procs" \
        "$root/sys/fs/cgroup/system.slice/cgroup.events" \
        "$root/sys/fs/cgroup/system.slice/cgroup.kill" \
        "$root/sys/fs/cgroup/system.slice/console-getty.service/cgroup.procs" \
        "$root/sys/fs/cgroup/system.slice/console-getty.service/cgroup.events" \
        "$root/sys/fs/cgroup/system.slice/systemd-user-sessions.service/cgroup.procs" \
        "$root/sys/fs/cgroup/system.slice/systemd-user-sessions.service/cgroup.events"
    for service_file in "$root"/usr/lib/systemd/system/*.service; do
        test -e "$service_file" || continue
        service=${service_file##*/}
        service_cgroup=$root/sys/fs/cgroup/system.slice/$service
        mkdir -p "$service_cgroup"
        touch \
            "$service_cgroup/cgroup.procs" \
            "$service_cgroup/cgroup.events" \
            "$service_cgroup/cgroup.kill"
    done
    mkdir -p \
        "$root/sys/fs/cgroup/system.slice/system-modprobe.slice" \
        "$root/sys/fs/cgroup/system.slice/system-modprobe.slice/modprobe@configfs.service"
    touch \
        "$root/sys/fs/cgroup/system.slice/system-modprobe.slice/cgroup.procs" \
        "$root/sys/fs/cgroup/system.slice/system-modprobe.slice/cgroup.events" \
        "$root/sys/fs/cgroup/system.slice/system-modprobe.slice/cgroup.kill" \
        "$root/sys/fs/cgroup/system.slice/system-modprobe.slice/modprobe@configfs.service/cgroup.procs" \
        "$root/sys/fs/cgroup/system.slice/system-modprobe.slice/modprobe@configfs.service/cgroup.events" \
        "$root/sys/fs/cgroup/system.slice/system-modprobe.slice/modprobe@configfs.service/cgroup.kill"
    cp --remove-destination /usr/share/zoneinfo/UTC "$root/etc/localtime"
    for device_spec in 'console 5 0 600' 'null 1 3 666' 'zero 1 5 666' 'random 1 8 666' 'urandom 1 9 666'; do
        set -- $device_spec
        test -e "$root/dev/$1" || mknod -m "$4" "$root/dev/$1" c "$2" "$3"
    done
    # The minimal OCI image intentionally omits these optional host services.
    # Use ordinary drop-ins instead of /dev/null masks: Ginux virtualizes the
    # domain path view, so an explicit false condition is both observable and
    # independent of absolute-symlink resolution.
    for unit in \
        dbus-broker.service \
        dbus.service \
        dbus.socket \
        modprobe@.service \
        systemd-userdbd.service \
        systemd-userdbd.socket
    do
        rm -f "$root/etc/systemd/system/$unit"
        mkdir -p "$root/etc/systemd/system/$unit.d"
        printf '%s\n' \
            '[Unit]' \
            'ConditionPathExists=/run/ginux-enable-optional-service' \
            >"$root/etc/systemd/system/$unit.d/90-ginux-minimal-image.conf"
    done
    # This minimal image has no UTMP database. Remove the image-provided wants
    # instead of condition-skipping the prerequisite and producing a dependency
    # failure for the runlevel recorder.
    rm -f \
        "$root/usr/lib/systemd/system/sysinit.target.wants/systemd-update-utmp.service" \
        "$root/usr/lib/systemd/system/graphical.target.wants/systemd-update-utmp-runlevel.service" \
        "$root/usr/lib/systemd/system/multi-user.target.wants/systemd-update-utmp-runlevel.service" \
        "$root/usr/lib/systemd/system/rescue.target.wants/systemd-update-utmp-runlevel.service"
    # Network access is already mediated by Ginux. Do not ask systemd to add a
    # second cgroup/BPF firewall that this virtual kernel does not expose.
    mkdir -p "$root/etc/systemd/system/systemd-journald.service.d"
    printf '%s\n' \
        '[Service]' \
        'IPAddressDeny=' \
        >"$root/etc/systemd/system/systemd-journald.service.d/90-ginux-network.conf"
    rm -f \
        "$root/etc/systemd/system/systemd-initctl.service" \
        "$root/etc/systemd/system/systemd-initctl.socket"
    printf '%s\n' \
        '[Unit]' \
        'Description=Keyboard Request Target' \
        'DefaultDependencies=no' >"$root/etc/systemd/system/kbrequest.target"
    ln -s / "$root/proc/1/root"
    rm -f "$root/etc/systemd/system/getty.target" "$root/etc/systemd/system/console-getty.service"
    ln -sf /usr/lib/systemd/system/console-getty.service \
        "$root/etc/systemd/system/getty.target.wants/console-getty.service"
    printf '%s\n' \
        '#%PAM-1.0' \
        'auth required pam_unix.so' \
        'account required pam_unix.so' \
        'password required pam_unix.so' \
        'session required pam_unix.so' >"$root/etc/pam.d/login"
    printf 'root:ginux\n' | chroot "$root" /usr/sbin/chpasswd
    chown -R 1000:1000 "$root"
    find "$root" -type d -exec chmod u+w {} +
    chmod 600 "$root/etc/shadow"
    test -x "$root/sbin/init"
}

create_domain() {
    test "$#" -ge 3 || usage
    name=$1
    test "$2" = --config || usage
    config=$3
    shift 3
    overrides=
    while test "$#" -gt 0; do
        test "$#" -ge 2 || usage
        test "$1" = --set || usage
        case $2 in
            *=*) ;;
            *) usage ;;
        esac
        overrides="$overrides
$2"
        shift 2
    done
    validate_name "$name"
    test -r "$config" || { printf 'configuration is not readable: %s\n' "$config" >&2; exit 66; }
    test -x "$runtime" || { printf '%s\n' 'Ginux runtime is not installed' >&2; exit 69; }
    test -x "$image_tools/ensure-rocky-oci.sh" || {
        printf '%s\n' 'Ginux image acquisition support is not installed' >&2
        exit 69
    }
    directory=$(domain_dir "$name")
    test ! -e "$directory" || { printf '%s: already exists\n' "$name" >&2; exit 73; }

    mkdir -p "$state_root/work"
    effective=$(mktemp "$state_root/work/config.XXXXXX")
    cleanup_effective() {
        rm -f "$effective"
    }
    trap cleanup_effective 0 HUP INT TERM
    set --
    old_ifs=$IFS
    IFS='
'
    for override in $overrides; do
        test -n "$override" || continue
        set -- "$@" "$override"
    done
    IFS=$old_ifs
    "$runtime" config-merge "$config" "$@" "domain.id=\"$name\"" >"$effective"
    configured_root=$(sed -n 's/^[[:space:]]*root[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$effective")
    expected_root=docker.io/rockylinux/rockylinux@sha256:7a18ce2c9c1e0d8735f3e72685f9666150af25036e7f7b5ee4eede3754496f8c
    if test "$configured_root" != "$expected_root"; then
        printf 'unsupported configured image: %s\n' "$configured_root" >&2
        exit 65
    fi
    GINUX_OCI_CONFIG_FILE=$image_config \
        GINUX_OCI_CACHE_DIR_OVERRIDE=$image_cache \
        GINUX_OCI_WORK_DIR_OVERRIDE=$state_root/work/oci \
        GINUX_OCI_NATIVE_ROOTFS_ONLY=1 \
        "$image_tools/ensure-rocky-oci.sh"
    mkdir -p "$directory/root"
    install -m 600 "$effective" "$directory/config.toml"
    tar -xf "$image_tar" -C "$directory/root"
    prepare_root "$directory/root"
    trap - 0 HUP INT TERM
    cleanup_effective
    printf '%s: created\n' "$name"
}

list_domains() {
    printf '%-24s %-10s %s\n' NAME STATE PID
    for list_directory in "$state_root"/domains/*; do
        test -d "$list_directory" || continue
        list_name=${list_directory##*/}
        list_state=$(reported_state "$list_name")
        list_pid=-
        if test "$list_state" = running; then
            list_pid=$(supervisor_identity "$list_name")
        fi
        printf '%-24s %-10s %s\n' "$list_name" "$list_state" "$list_pid"
    done
}

status_domain() {
    test "$#" -eq 1 || usage
    name=$1
    validate_name "$name"
    directory=$(domain_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    state=$(reported_state "$name")
    printf 'name: %s\nstate: %s\n' "$name" "$state"
    if test "$state" = running; then
        printf 'pid: %s\n' "$(supervisor_identity "$name")"
    fi
    printf 'config: %s\nconsole-log: %s\naudit-log: %s\n' \
        "$directory/config.toml" "$directory/console.log" "$directory/audit.log"
}

start_domain() {
    test "$#" -ge 1 && test "$#" -le 2 || usage
    name=$1
    start_mode=detached
    if test "$#" -eq 2; then
        test "$2" = --attach || usage
        start_mode=attached
    fi
    validate_name "$name"
    directory=$(domain_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    if supervisor_identity "$name" >/dev/null; then
        printf '%s: already running\n' "$name" >&2
        exit 73
    fi
    rm -f \
        "$directory/supervisor.pid" \
        "$directory/runtime.pid" \
        "$directory/exit.status" \
        "$directory/console.in"
    mkfifo -m 600 "$directory/console.in"
    : >"$directory/audit.log"
    : >"$directory/console.log"

    nohup "$0" supervise "$name" >/dev/null 2>&1 </dev/null &
    pid=$!
    record_supervisor "$directory" "$pid"
    sleep 1
    if ! kill -0 "$pid" 2>/dev/null; then
        rm -f "$directory/supervisor.pid"
        printf '%s: failed to start; inspect %s and %s\n' \
            "$name" "$directory/console.log" "$directory/audit.log" >&2
        exit 70
    fi
    if test "$start_mode" = attached; then
        printf '%s: started (supervisor pid %s); attaching console\n' "$name" "$pid"
    else
        printf '%s: started detached (supervisor pid %s)\n' "$name" "$pid"
    fi
}

supervise_domain() {
    test "$#" -eq 1 || usage
    name=$1
    validate_name "$name"
    directory=$(domain_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    exec 9<>"$directory/console.in"
    set +e
    # Let script(1) provide the pseudo-terminal, but record only the runtime
    # stream. Its normal typescript header/footer are implementation details,
    # not domain console output.
    script -qefE never -c "$0 run-runtime $name" /dev/null \
        <&9 >>"$directory/console.log" 2>>"$directory/audit.log" &
    console_pid=$!
    wait "$console_pid"
    status=$?
    set -e
    printf '%s\n' "$status" >"$directory/exit.status"
    rm -f "$directory/runtime.pid" "$directory/supervisor.pid"
    exit "$status"
}

run_runtime() {
    test "$#" -eq 1 || usage
    name=$1
    validate_name "$name"
    directory=$(domain_dir "$name")
    test -d "$directory" || exit 66
    record_runtime "$directory" "$$"
    exec "$runtime" native "$directory/config.toml" "$directory/root" \
        2>>"$directory/audit.log"
}

attach_domain() {
    test "$#" -eq 1 || usage
    name=$1
    validate_name "$name"
    directory=$(domain_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    supervisor_pid=$(supervisor_identity "$name") || {
        printf '%s: not running\n' "$name" >&2
        exit 69
    }
    test -p "$directory/console.in" || {
        printf '%s: console is unavailable\n' "$name" >&2
        exit 69
    }

    detach_marker=$directory/attach.$$.detach
    rm -f "$detach_marker"
    saved_tty=$(stty -g)
    exec 7<&0
    tail -n 80 -F "$directory/console.log" &
    tail_pid=$!
    reader_pid=
    cleanup_attach() {
        trap - 0 HUP INT TERM
        kill "$tail_pid" ${reader_pid:+"$reader_pid"} 2>/dev/null || true
        wait "$tail_pid" ${reader_pid:+"$reader_pid"} 2>/dev/null || true
        exec 7<&-
        stty "$saved_tty"
        rm -f "$detach_marker"
    }
    trap cleanup_attach 0 HUP INT TERM
    stty raw -echo
    (
        exec 8>"$directory/console.in"
        detach=$(printf '\035')
        while IFS= read -r -n 1 character <&7; do
            if test "$character" = "$detach"; then
                : >"$detach_marker"
                exit 0
            fi
            if test -n "$character"; then
                printf '%s' "$character" >&8
            else
                printf '\n' >&8
            fi
        done
    ) &
    reader_pid=$!

    while kill -0 "$supervisor_pid" 2>/dev/null && test ! -e "$detach_marker"; do
        sleep 1
    done
    detached=false
    test -e "$detach_marker" && detached=true
    cleanup_attach
    trap - 0 HUP INT TERM
    if test "$detached" = true; then
        printf '\n%s: detached; domain remains running\n' "$name"
    else
        status=unknown
        test -r "$directory/exit.status" && status=$(cat "$directory/exit.status")
        printf '\n%s: stopped (status %s)\n' "$name" "$status"
    fi
}

stop_domain() {
    test "$#" -eq 1 || usage
    name=$1
    validate_name "$name"
    directory=$(domain_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    if ! pid=$(supervisor_identity "$name"); then
        rm -f "$directory/supervisor.pid"
        printf '%s: already stopped\n' "$name"
        return
    fi

    runtime_pid=$(runtime_identity "$name" || true)
    root_pid=
    if test -n "$runtime_pid"; then
        root_pid=$(pgrep -P "$runtime_pid" | sed -n '1p' || true)
    fi
    if test -n "$root_pid"; then
        kill -RTMIN+4 "$root_pid" 2>/dev/null || true
    fi
    attempt=0
    while kill -0 "$pid" 2>/dev/null && test "$attempt" -lt 60; do
        attempt=$((attempt + 1))
        sleep 1
    done
    if kill -0 "$pid" 2>/dev/null; then
        kill -TERM "$pid" 2>/dev/null || true
        sleep 2
    fi
    if kill -0 "$pid" 2>/dev/null; then
        kill -KILL "$pid" 2>/dev/null || true
    fi
    rm -f "$directory/supervisor.pid" "$directory/runtime.pid"
    printf '%s: stopped\n' "$name"
}

mkdir -p "$state_root/domains"
command=${1-}
test -n "$command" || usage
shift
case $command in
    attach) attach_domain "$@" ;;
    create) create_domain "$@" ;;
    list) test "$#" -eq 0 || usage; list_domains ;;
    prepare-root) test "$#" -eq 1 || usage; prepare_root "$1" ;;
    start) start_domain "$@" ;;
    status) status_domain "$@" ;;
    stop) stop_domain "$@" ;;
    run-runtime) run_runtime "$@" ;;
    supervise) supervise_domain "$@" ;;
    *) usage ;;
esac
