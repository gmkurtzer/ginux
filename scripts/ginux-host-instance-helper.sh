#!/bin/sh
set -eu

state_root=/var/lib/ginux
runtime=/usr/local/libexec/ginux-runtime
image_tools=/usr/local/libexec/ginux-image
image_config=$image_tools/rocky-systemd-oci.env
image_cache=$state_root/images/rocky-10-ubi-init-amd64
image_tar=$image_cache/rocky-10-ubi-init-amd64-rootfs.tar

usage() {
    printf '%s\n' 'usage: ginux instance create NAME FILE [--set KEY=TOML_VALUE]... [--network ATTACHMENT] [--forward SPEC]...' >&2
    printf '%s\n' '       ginux instance set NAME [--network ATTACHMENT] [--forward SPEC]...' >&2
    printf '%s\n' '       ginux instance list' >&2
    printf '%s\n' '       ginux instance start NAME [--attach] [--audit]' >&2
    printf '%s\n' '       ginux instance attach NAME' >&2
    printf '%s\n' '       ginux instance status NAME' >&2
    printf '%s\n' '       ginux instance stop NAME' >&2
    exit 64
}

validate_name() {
    case ${1-} in
        ''|*[!A-Za-z0-9_.-]*)
            printf 'invalid instance name: %s\n' "${1-}" >&2
            exit 64
            ;;
    esac
}

instance_dir() {
    printf '%s/instances/%s' "$state_root" "$1"
}

supervisor_identity() {
    supervisor_directory=$(instance_dir "$1")
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
    runtime_directory=$(instance_dir "$1")
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

network_addresses() {
    attachment=$1
    if ip link show dev "$attachment" >/dev/null 2>&1; then
        ip -o -4 addr show dev "$attachment"
    else
        alias_addresses=$(ip -o -4 addr show label "$attachment")
        test -n "$alias_addresses" || return 1
        printf '%s\n' "$alias_addresses"
    fi
}

resolve_network_config() {
    resolve_file=$1
    resolve_interface=$(sed -n 's/^[[:space:]]*interface[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$resolve_file")
    resolve_source=$(sed -n 's/^[[:space:]]*source[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$resolve_file")
    resolve_addresses=
    if test -n "$resolve_interface"; then
        resolve_addresses=$(network_addresses "$resolve_interface") || {
            printf 'configured network attachment is unavailable: %s\n' "$resolve_interface" >&2
            exit 65
        }
        test -n "$resolve_addresses" || {
            printf 'configured network attachment has no IPv4 address: %s\n' "$resolve_interface" >&2
            exit 65
        }
    fi
    if test -z "$resolve_source" && test -n "$resolve_addresses"; then
        resolve_source=$(printf '%s\n' "$resolve_addresses" | awk 'NR == 1 { split($4, address, "/"); print address[1] }')
        resolve_output=$(mktemp "$state_root/work/config.XXXXXX")
        "$runtime" config-merge "$resolve_file" "network.source=\"$resolve_source\"" >"$resolve_output"
        mv "$resolve_output" "$resolve_file"
    fi
    if test -n "$resolve_source"; then
        if test -n "$resolve_addresses"; then
            printf '%s\n' "$resolve_addresses" |
                awk -v expected="$resolve_source" '{ split($4, address, "/"); if (address[1] == expected) found=1 } END { exit !found }' || {
                    printf 'configured source address is not assigned to %s: %s\n' "$resolve_interface" "$resolve_source" >&2
                    exit 65
                }
        else
            ip -o -4 addr show |
                awk -v expected="$resolve_source" '{ split($4, address, "/"); if (address[1] == expected) found=1 } END { exit !found }' || {
                    printf 'configured source address is unavailable: %s\n' "$resolve_source" >&2
                    exit 65
                }
        fi
    fi
}

validate_forward() {
    forward_spec=$1
    forward_protocol=
    forward_external=
    forward_instance=
    old_ifs=$IFS
    IFS=,
    set -- $forward_spec
    IFS=$old_ifs
    test "$#" -eq 3 || return 1
    for forward_field in "$@"; do
        case $forward_field in
            protocol=tcp|protocol=udp)
                test -z "$forward_protocol" || return 1
                forward_protocol=${forward_field#protocol=}
                ;;
            external_port=*)
                test -z "$forward_external" || return 1
                forward_external=${forward_field#external_port=}
                ;;
            instance_port=*)
                test -z "$forward_instance" || return 1
                forward_instance=${forward_field#instance_port=}
                ;;
            *) return 1 ;;
        esac
    done
    for forward_port in "$forward_external" "$forward_instance"; do
        case $forward_port in
            ''|*[!0-9]*) return 1 ;;
        esac
        test "$forward_port" -ge 1 && test "$forward_port" -le 65535 || return 1
    done
    test -n "$forward_protocol"
}

validate_attachment() {
    case ${1-} in
        ''|*[!A-Za-z0-9_.:-]*) return 1 ;;
        *) return 0 ;;
    esac
}

read_deployment() {
    deployment_file=$1
    deployment_network=
    deployment_forwards=
    test -f "$deployment_file" || return 0
    while IFS="$(printf '\t')" read -r deployment_key deployment_value deployment_extra; do
        test -z "$deployment_extra" || return 1
        case $deployment_key in
            network)
                test -z "$deployment_network" || return 1
                deployment_network=$deployment_value
                ;;
            forward)
                validate_forward "$deployment_value" || return 1
                deployment_forwards="$deployment_forwards
$deployment_value"
                ;;
            '') ;;
            *) return 1 ;;
        esac
    done <"$deployment_file"
}

write_deployment() {
    deployment_file=$1
    deployment_network=$2
    deployment_forwards=$3
    deployment_tmp=$deployment_file.tmp.$$
    : >"$deployment_tmp"
    if test -n "$deployment_network"; then
        printf 'network\t%s\n' "$deployment_network" >>"$deployment_tmp"
    fi
    old_ifs=$IFS
    IFS='
'
    for deployment_forward in $deployment_forwards; do
        test -n "$deployment_forward" || continue
        printf 'forward\t%s\n' "$deployment_forward" >>"$deployment_tmp"
    done
    IFS=$old_ifs
    chmod 600 "$deployment_tmp"
    mv "$deployment_tmp" "$deployment_file"
}

materialize_runtime_config() {
    materialize_recipe=$1
    materialize_deployment=$2
    materialize_output=$3
    read_deployment "$materialize_deployment" || {
        printf '%s\n' 'invalid persisted instance deployment state' >&2
        return 1
    }
    set --
    if test -n "$deployment_network"; then
        set -- "$@" "network.interface=\"$deployment_network\""
    fi
    forward_list=
    forward_separator=
    old_ifs=$IFS
    IFS='
'
    for materialize_forward in $deployment_forwards; do
        test -n "$materialize_forward" || continue
        validate_forward "$materialize_forward" || return 1
        forward_attachment=${deployment_network:-default}
        forward_value=$forward_protocol/$forward_attachment/$forward_external/$forward_instance
        forward_list=$forward_list$forward_separator\"$forward_value\"
        forward_separator=', '
    done
    IFS=$old_ifs
    if test -n "$forward_list"; then
        set -- "$@" "network.port_forwards=[$forward_list]"
    fi
    "$runtime" config-merge "$materialize_recipe" "$@" >"$materialize_output"
    resolve_network_config "$materialize_output"
}

prepare_root() {
    root=$1
    mkdir -p \
        "$root/run" \
        "$root/run/host" \
        "$root/proc" \
        "$root/sys" \
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
    # procfs and sysfs are Ginux-owned, on-demand object models. Clear any
    # materialized nodes left by an older runtime, retaining only mountpoints.
    find "$root/proc" -mindepth 1 -delete
    find "$root/sys" -mindepth 1 -delete
    for device_spec in 'console 5 0 600' 'null 1 3 666' 'zero 1 5 666' 'random 1 8 666' 'urandom 1 9 666'; do
        device_ifs=$IFS
        IFS=' '
        set -- $device_spec
        IFS=$device_ifs
        test -e "$root/dev/$1" || mknod -m "$4" "$root/dev/$1" c "$2" "$3"
    done
    # The runtime drops root-launched workloads to the host nobody identity.
    # Ginux maintains instance-visible credentials independently.
    chown -R 65534:65534 "$root"
    find "$root" -type d -exec chmod u+w {} +
    chmod 600 "$root/etc/shadow"
    test -x "$root/sbin/init"
}

create_instance() {
    test "$#" -ge 2 || usage
    name=$1
    config=$2
    shift 2
    overrides=
    create_network=
    create_network_seen=false
    create_forwards=
    create_forward_mode=
    while test "$#" -gt 0; do
        case $1 in
            --set)
                test "$#" -ge 2 || usage
                case $2 in *=*) ;; *) usage ;; esac
                overrides="$overrides
$2"
                shift 2
                ;;
            --network)
                test "$#" -ge 2 || usage
                test "$create_network_seen" = false || usage
                case $2 in
                    default) create_network= ;;
                    *) validate_attachment "$2" || usage; create_network=$2 ;;
                esac
                create_network_seen=true
                shift 2
                ;;
            --forward)
                test "$#" -ge 2 || usage
                if test "$2" = none; then
                    test -z "$create_forward_mode" || usage
                    create_forward_mode=none
                else
                    test "$create_forward_mode" != none || usage
                    validate_forward "$2" || usage
                    create_forward_mode=list
                    create_forwards="$create_forwards
$2"
                fi
                shift 2
                ;;
            *) usage ;;
        esac
    done
    validate_name "$name"
    test -r "$config" || { printf 'configuration is not readable: %s\n' "$config" >&2; exit 66; }
    test -x "$runtime" || { printf '%s\n' 'Ginux runtime is not installed' >&2; exit 69; }
    test -x "$image_tools/ensure-rocky-oci.sh" || {
        printf '%s\n' 'Ginux image acquisition support is not installed' >&2
        exit 69
    }
    directory=$(instance_dir "$name")
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
    "$runtime" config-merge "$config" "$@" "instance.id=\"$name\"" >"$effective"
    configured_root=$(sed -n 's/^[[:space:]]*root[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$effective")
    expected_root=docker.io/rockylinux/rockylinux@sha256:7a18ce2c9c1e0d8735f3e72685f9666150af25036e7f7b5ee4eede3754496f8c
    if test "$configured_root" != "$expected_root"; then
        printf 'unsupported configured image: %s\n' "$configured_root" >&2
        exit 65
    fi
    printf 'Creating instance "%s"\n' "$name"
    GINUX_OCI_CONFIG_FILE=$image_config \
        GINUX_OCI_CACHE_DIR_OVERRIDE=$image_cache \
        GINUX_OCI_WORK_DIR_OVERRIDE=$state_root/work/oci \
        GINUX_OCI_NATIVE_ROOTFS_ONLY=1 \
        GINUX_OCI_OUTPUT=human \
        "$image_tools/ensure-rocky-oci.sh"
    printf '  Preparing instance... '
    mkdir -p "$directory/root"
    install -m 600 "$effective" "$directory/config.toml"
    write_deployment "$directory/deployment.state" "$create_network" "$create_forwards"
    materialize_runtime_config \
        "$directory/config.toml" "$directory/deployment.state" "$directory/runtime.toml"
    tar -xf "$image_tar" -C "$directory/root"
    prepare_root "$directory/root"
    printf 'done\n'
    trap - 0 HUP INT TERM
    cleanup_effective
    printf 'Instance "%s" created.\n' "$name"
}

set_instance() {
    test "$#" -ge 3 || usage
    name=$1
    shift
    validate_name "$name"
    directory=$(instance_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    if supervisor_identity "$name" >/dev/null; then
        printf '%s: stop the instance before changing deployment settings\n' "$name" >&2
        exit 73
    fi
    read_deployment "$directory/deployment.state" || {
        printf '%s: invalid persisted deployment state\n' "$name" >&2
        exit 65
    }
    next_network=$deployment_network
    next_forwards=$deployment_forwards
    set_network_seen=false
    set_forward_seen=false
    set_forward_mode=
    replacement_forwards=
    while test "$#" -gt 0; do
        case $1 in
            --network)
                test "$#" -ge 2 || usage
                test "$set_network_seen" = false || usage
                case $2 in
                    default) next_network= ;;
                    ''|*[!A-Za-z0-9_.:-]*) usage ;;
                    *) next_network=$2 ;;
                esac
                set_network_seen=true
                shift 2
                ;;
            --forward)
                test "$#" -ge 2 || usage
                if test "$2" = none; then
                    test -z "$set_forward_mode" || usage
                    replacement_forwards=
                    set_forward_mode=none
                    set_forward_seen=true
                else
                    test "$set_forward_mode" != none || usage
                    validate_forward "$2" || usage
                    set_forward_mode=list
                    replacement_forwards="$replacement_forwards
$2"
                    set_forward_seen=true
                fi
                shift 2
                ;;
            *) usage ;;
        esac
    done
    test "$set_network_seen" = true || test "$set_forward_seen" = true || usage
    if test "$set_forward_seen" = true; then
        next_forwards=$replacement_forwards
    fi
    candidate_deployment=$directory/deployment.state.candidate.$$
    candidate_runtime=$directory/runtime.toml.candidate.$$
    cleanup_candidate() {
        rm -f "$candidate_deployment" "$candidate_runtime"
    }
    trap cleanup_candidate 0 HUP INT TERM
    write_deployment "$candidate_deployment" "$next_network" "$next_forwards"
    materialize_runtime_config \
        "$directory/config.toml" "$candidate_deployment" "$candidate_runtime"
    mv "$candidate_deployment" "$directory/deployment.state"
    mv "$candidate_runtime" "$directory/runtime.toml"
    trap - 0 HUP INT TERM
    printf 'Instance "%s" updated.\n' "$name"
}

list_instances() {
    printf '%-24s %-10s %s\n' NAME STATE PID
    for list_directory in "$state_root"/instances/*; do
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

status_instance() {
    test "$#" -eq 1 || usage
    name=$1
    validate_name "$name"
    directory=$(instance_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    state=$(reported_state "$name")
    printf 'name: %s\nstate: %s\n' "$name" "$state"
    if test "$state" = running; then
        printf 'pid: %s\n' "$(supervisor_identity "$name")"
    fi
    printf 'config: %s\ndeployment: %s\nconsole-log: %s\naudit-log: %s\n' \
        "$directory/config.toml" "$directory/deployment.state" \
        "$directory/console.log" "$directory/audit.log"
}

start_instance() {
    test "$#" -ge 1 || usage
    name=$1
    shift
    start_mode=detached
    start_audit=false
    while test "$#" -gt 0; do
        case $1 in
            --attach)
                test "$start_mode" = detached || usage
                start_mode=attached
                ;;
            --audit)
                test "$start_audit" = false || usage
                start_audit=true
                ;;
            *) usage ;;
        esac
        shift
    done
    validate_name "$name"
    directory=$(instance_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    if supervisor_identity "$name" >/dev/null; then
        printf '%s: already running\n' "$name" >&2
        exit 73
    fi
    rm -f \
        "$directory/supervisor.pid" \
        "$directory/runtime.pid" \
        "$directory/exit.status" \
        "$directory/console.in" \
        "$directory/control.in"
    mkfifo -m 600 "$directory/console.in"
    mkfifo -m 600 "$directory/control.in"
    : >"$directory/audit.log"
    : >"$directory/console.log"

    nohup "$0" supervise "$name" "$start_audit" >/dev/null 2>&1 </dev/null &
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

supervise_instance() {
    test "$#" -eq 2 || usage
    name=$1
    audit_override=$2
    case $audit_override in
        true|false) ;;
        *) usage ;;
    esac
    validate_name "$name"
    directory=$(instance_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    exec 9<>"$directory/console.in"
    exec 10<>"$directory/control.in"
    set +e
    # Let script(1) provide the pseudo-terminal, but record only the runtime
    # stream. Its normal typescript header/footer are implementation details,
    # not instance console output.
    script -qefE never -c "$0 run-runtime $name $audit_override" /dev/null \
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
    test "$#" -eq 2 || usage
    name=$1
    audit_override=$2
    case $audit_override in
        true|false) ;;
        *) usage ;;
    esac
    validate_name "$name"
    directory=$(instance_dir "$name")
    test -d "$directory" || exit 66
    record_runtime "$directory" "$$"
    materialize_runtime_config \
        "$directory/config.toml" "$directory/deployment.state" "$directory/runtime.toml"
    if test "$audit_override" = true; then
        audit_candidate=$directory/runtime.toml.audit.$$
        "$runtime" config-merge "$directory/runtime.toml" 'audit.mode="stderr"' \
            >"$audit_candidate"
        mv "$audit_candidate" "$directory/runtime.toml"
    fi
    hook_plan=$($runtime hook-plan "$directory/runtime.toml") || exit 65
    hook_count=
    hook_timeout=
    hook_failure=
    hook_extra=
    IFS=' ' read -r hook_count hook_timeout hook_failure hook_extra <<EOF
$hook_plan
EOF
    case $hook_count:$hook_timeout in
        *[!0-9:]*|:*|*:)
            printf '%s\n' 'invalid pre-boot hook plan' >&2
            exit 65
            ;;
    esac
    case $hook_failure:$hook_extra in
        stop:|continue:) ;;
        *) printf '%s\n' 'invalid pre-boot hook plan' >&2; exit 65 ;;
    esac
    hook_index=0
    while test "$hook_index" -lt "$hook_count"; do
        printf 'GINUX_HOOK_AUDIT: action=dispatch index=%s timeout=%s failure=%s context=instance\n' \
            "$hook_index" "$hook_timeout" "$hook_failure" >>"$directory/audit.log"
        set +e
        timeout --signal=TERM --kill-after=2 "$hook_timeout" \
            "$runtime" native-hook "$directory/runtime.toml" "$directory/root" "$hook_index" \
            >>"$directory/audit.log" 2>&1
        hook_status=$?
        set -e
        if test "$hook_status" -eq 0; then
            printf 'GINUX_HOOK_AUDIT: action=complete index=%s result=pass\n' \
                "$hook_index" >>"$directory/audit.log"
        else
            if test "$hook_status" -eq 124 || test "$hook_status" -eq 137; then
                hook_result=timeout
            else
                hook_result=failed
            fi
            printf 'GINUX_HOOK_AUDIT: action=complete index=%s result=%s status=%s\n' \
                "$hook_index" "$hook_result" "$hook_status" >>"$directory/audit.log"
            if test "$hook_failure" = stop; then
                exit "$hook_status"
            fi
        fi
        hook_index=$((hook_index + 1))
    done
    exec "$runtime" native "$directory/runtime.toml" "$directory/root" configured \
        2>>"$directory/audit.log"
}

attach_instance() {
    test "$#" -eq 1 || usage
    name=$1
    validate_name "$name"
    directory=$(instance_dir "$name")
    test -d "$directory" || { printf '%s: not found\n' "$name" >&2; exit 66; }
    supervisor_pid=$(supervisor_identity "$name") || {
        printf '%s: not running\n' "$name" >&2
        exit 69
    }
    test -p "$directory/console.in" || {
        printf '%s: console is unavailable\n' "$name" >&2
        exit 69
    }
    test -p "$directory/control.in" || {
        printf '%s: control channel is unavailable\n' "$name" >&2
        exit 69
    }

    detach_marker=$directory/attach.$$.detach
    rm -f "$detach_marker"
    saved_tty=$(stty -g)
    exec 7<&0
    exec 8>"$directory/console.in"
    exec 9>"$directory/control.in"
    # A terminal-generated SIGINT belongs to the attached foreground process,
    # not to the controller-side attach transport. Background helpers inherit
    # SIGINT ignored; the attach process sends the same interrupt request over
    # Ginux's control channel if its terminal driver emits a signal instead.
    trap '' INT
    tail -n 80 -F "$directory/console.log" &
    tail_pid=$!
    reader_pid=
    sync_console_size() {
        size=$(stty size 2>/dev/null) || return 0
        rows=
        columns=
        extra=
        IFS=' ' read -r rows columns extra <<EOF
$size
EOF
        case "$rows:$columns:$extra" in
            *[!0-9:]*|0:*|*:0:*|*:*:?*) return 0 ;;
        esac
        attached_runtime=$(runtime_identity "$name" || true)
        test -n "$attached_runtime" || return 0
        runtime_tty=$(readlink "/proc/$attached_runtime/fd/0" 2>/dev/null || true)
        case "$runtime_tty" in
            /dev/pts/[0-9]*)
                stty -F "$runtime_tty" rows "$rows" cols "$columns" 2>/dev/null || true
                ;;
        esac
    }
    cleanup_attach() {
        trap - 0 HUP INT TERM WINCH
        kill "$tail_pid" ${reader_pid:+"$reader_pid"} 2>/dev/null || true
        wait "$tail_pid" ${reader_pid:+"$reader_pid"} 2>/dev/null || true
        exec 8>&-
        exec 9>&-
        exec 7<&-
        stty "$saved_tty"
        rm -f "$detach_marker"
    }
    forward_interrupt() {
        printf 'I' >&9 || true
    }
    trap cleanup_attach 0 HUP TERM
    trap forward_interrupt INT
    trap 'sync_console_size' WINCH
    sync_console_size
    stty raw -echo
    (
        trap '' INT
        detach=$(printf '\035')
        interrupt=$(printf '\003')
        while IFS= read -r -n 1 character <&7; do
            if test "$character" = "$detach"; then
                : >"$detach_marker"
                exit 0
            fi
            if test "$character" = "$interrupt"; then
                printf 'I' >&9
                continue
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
        printf '\n%s: detached; instance remains running\n' "$name"
    else
        status=unknown
        test -r "$directory/exit.status" && status=$(cat "$directory/exit.status")
        printf '\n%s: stopped (status %s)\n' "$name" "$status"
    fi
}

stop_instance() {
    test "$#" -eq 1 || usage
    name=$1
    validate_name "$name"
    directory=$(instance_dir "$name")
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

mkdir -p "$state_root/instances"
command=${1-}
test -n "$command" || usage
shift
case $command in
    attach) attach_instance "$@" ;;
    create) create_instance "$@" ;;
    list) test "$#" -eq 0 || usage; list_instances ;;
    prepare-root) test "$#" -eq 1 || usage; prepare_root "$1" ;;
    resolve-network) test "$#" -eq 1 || usage; resolve_network_config "$1" ;;
    set) set_instance "$@" ;;
    start) start_instance "$@" ;;
    status) status_instance "$@" ;;
    stop) stop_instance "$@" ;;
    run-runtime) run_runtime "$@" ;;
    supervise) supervise_instance "$@" ;;
    *) usage ;;
esac
