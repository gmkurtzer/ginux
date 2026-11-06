#!/bin/sh
set -eu

state_root=/var/lib/ginux
runtime=/usr/local/libexec/ginux-runtime
image_tools=/usr/local/libexec/ginux-image
image_config=$image_tools/rocky-systemd-oci.env
image_cache=$state_root/images/rocky-10-ubi-init-amd64
image_tar=$image_cache/rocky-10-ubi-init-amd64-rootfs.tar

usage() {
    printf '%s\n' 'usage: ginux domain create NAME --config FILE' >&2
    printf '%s\n' '       ginux domain list' >&2
    printf '%s\n' '       ginux domain start NAME [--attach]' >&2
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
        "$root/sys/fs/cgroup/systemd" \
        "$root/sys/fs/cgroup/init.scope" \
        "$root/sys/fs/cgroup/system.slice/console-getty.service" \
        "$root/sys/fs/cgroup/system.slice/systemd-user-sessions.service" \
        "$root/dev/shm" \
        "$root/dev/pts" \
        "$root/usr/local/sbin"
    touch "$root/etc/resolv.conf"
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
    printf 'Linux version 6.12.0-ginux (ginux)\n' >"$root/proc/version"
    printf '9223372036854775807\n' >"$root/proc/sys/fs/file-max"
    printf '1048576\n' >"$root/proc/sys/fs/nr_open"
    printf 'cpu memory pids\n' >"$root/sys/fs/cgroup/cgroup.controllers"
    touch \
        "$root/sys/fs/cgroup/cgroup.procs" \
        "$root/sys/fs/cgroup/cgroup.subtree_control" \
        "$root/sys/fs/cgroup/init.scope/cgroup.procs" \
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
    mkdir -p "$root/sys/fs/cgroup/system-modprobe.slice/modprobe@configfs.service"
    touch \
        "$root/sys/fs/cgroup/system-modprobe.slice/modprobe@configfs.service/cgroup.procs" \
        "$root/sys/fs/cgroup/system-modprobe.slice/modprobe@configfs.service/cgroup.events" \
        "$root/sys/fs/cgroup/system-modprobe.slice/modprobe@configfs.service/cgroup.kill"
    test -p "$root/run/initctl" || mkfifo "$root/run/initctl"
    if test ! -e "$root/etc/localtime" && test ! -L "$root/etc/localtime"; then
        ln -s /usr/share/zoneinfo/UTC "$root/etc/localtime"
    fi
    mknod -m 600 "$root/dev/console" c 5 0
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
    test "$#" -eq 3 || usage
    name=$1
    test "$2" = --config || usage
    config=$3
    validate_name "$name"
    test -r "$config" || { printf 'configuration is not readable: %s\n' "$config" >&2; exit 66; }
    test -x "$runtime" || { printf '%s\n' 'Ginux runtime is not installed' >&2; exit 69; }
    test -x "$image_tools/ensure-rocky-oci.sh" || {
        printf '%s\n' 'Ginux image acquisition support is not installed' >&2
        exit 69
    }
    directory=$(domain_dir "$name")
    test ! -e "$directory" || { printf '%s: already exists\n' "$name" >&2; exit 73; }

    "$runtime" config "$config" >/dev/null
    configured_root=$(sed -n 's/^[[:space:]]*root[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$config")
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
    install -m 600 "$config" "$directory/config.toml"
    tar -xf "$image_tar" -C "$directory/root"
    prepare_root "$directory/root"
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
    test "$#" -eq 1 || usage
    name=$1
    validate_name "$name"
    directory=$(domain_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    if supervisor_identity "$name" >/dev/null; then
        printf '%s: already running\n' "$name" >&2
        exit 73
    fi
    rm -f "$directory/supervisor.pid"
    : >"$directory/audit.log"

    nohup "$runtime" native "$directory/config.toml" "$directory/root" \
        >"$directory/console.log" 2>"$directory/audit.log" </dev/null &
    pid=$!
    record_supervisor "$directory" "$pid"
    sleep 1
    if ! kill -0 "$pid" 2>/dev/null; then
        rm -f "$directory/supervisor.pid"
        printf '%s: failed to start; inspect %s and %s\n' \
            "$name" "$directory/console.log" "$directory/audit.log" >&2
        exit 70
    fi
    printf '%s: started detached (supervisor pid %s)\n' "$name" "$pid"
}

attach_domain() {
    test "$#" -eq 1 || usage
    name=$1
    validate_name "$name"
    directory=$(domain_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    if supervisor_identity "$name" >/dev/null; then
        printf '%s: already running\n' "$name" >&2
        exit 73
    fi
    rm -f "$directory/supervisor.pid"
    : >"$directory/audit.log"
    record_supervisor "$directory" "$$"
    printf '%s: starting attached (supervisor pid %s)\n' "$name" "$$"
    exec "$runtime" native "$directory/config.toml" "$directory/root" \
        2>"$directory/audit.log"
}

finish_domain() {
    test "$#" -eq 2 || usage
    name=$1
    status=$2
    validate_name "$name"
    case $status in ''|*[!0-9]*) usage ;; esac
    directory=$(domain_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    rm -f "$directory/supervisor.pid"
    printf '%s: stopped (status %s)\n' "$name" "$status"
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

    root_pid=$(pgrep -P "$pid" | sed -n '1p' || true)
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
    rm -f "$directory/supervisor.pid"
    printf '%s: stopped\n' "$name"
}

mkdir -p "$state_root/domains"
command=${1-}
test -n "$command" || usage
shift
case $command in
    attach) attach_domain "$@" ;;
    create) create_domain "$@" ;;
    finish) finish_domain "$@" ;;
    list) test "$#" -eq 0 || usage; list_domains ;;
    start) start_domain "$@" ;;
    status) status_domain "$@" ;;
    stop) stop_domain "$@" ;;
    *) usage ;;
esac
