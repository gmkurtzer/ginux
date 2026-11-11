#!/bin/sh
set -eu

helper=${GINUX_INSTANCE_HELPER:-/usr/local/libexec/ginux-instance-helper}
runtime=${GINUX_RUNTIME:-/usr/local/libexec/ginux-runtime}

usage() {
    printf '%s\n' 'usage:' >&2
    printf '%s\n' '  ginux create NAME [--set KEY=TOML_VALUE]... [--network ATTACHMENT] [--forward SPEC]...' >&2
    printf '%s\n' '  ginux import NAME FILE [--set KEY=TOML_VALUE]... [--network ATTACHMENT] [--forward SPEC]...' >&2
    printf '%s\n' '  ginux set NAME [--set KEY=TOML_VALUE]... [--network ATTACHMENT] [--forward SPEC]...' >&2
    printf '%s\n' '  ginux edit NAME' >&2
    printf '%s\n' '  ginux export NAME' >&2
    printf '%s\n' '  ginux list' >&2
    printf '%s\n' '  ginux lock NAME' >&2
    printf '%s\n' '  ginux unlock NAME' >&2
    printf '%s\n' '  ginux start NAME [--attach] [--audit]' >&2
    printf '%s\n' '  ginux attach NAME' >&2
    printf '%s\n' '  ginux status NAME' >&2
    printf '%s\n' '  ginux stop NAME' >&2
    printf '%s\n' '  ginux validate FILE' >&2
    printf '%s\n' '  ginux inspect FILE' >&2
    printf '%s\n' '  ginux explain FILE OPERATION' >&2
    printf '%s\n' '  ginux diff LEFT RIGHT' >&2
    exit 64
}

test "$#" -ge 1 || usage
command=$1
shift
case $command in
    validate|inspect)
        test "$#" -eq 1 || usage
        exec "$runtime" "instance-$command" "$1"
        ;;
    explain)
        test "$#" -eq 2 || usage
        exec "$runtime" instance-explain "$1" "$2"
        ;;
    diff)
        test "$#" -eq 2 || usage
        exec "$runtime" instance-diff "$1" "$2"
        ;;
    attach|create|export|import|list|lock|set|start|status|stop|unlock) ;;
    edit)
        test "$#" -eq 1 || usage
        name=$1
        edit_file=$(mktemp "${TMPDIR:-/tmp}/ginux-edit.XXXXXX") || exit 70
        cleanup_edit() {
            rm -f "$edit_file"
        }
        trap cleanup_edit 0 HUP INT TERM
        sudo "$helper" export "$name" >"$edit_file"
        editor=${VISUAL:-${EDITOR:-vi}}
        "$editor" "$edit_file"
        sudo "$helper" import "$name" "$edit_file"
        exit
        ;;
    *) usage ;;
esac

if test "$command" = start && test "$#" -eq 2 && test "$2" = --attach; then
    name=$1
    sudo "$helper" start "$name" --attach
    command=attach
    set -- "$name"
fi

if test "$command" = attach && test "$#" -eq 1; then
    # Authorize access to this instance's console endpoints, then keep sudo
    # entirely out of the live terminal path. The attach transport switches to
    # raw mode and forwards terminal controls to Ginux's virtual foreground
    # process group.
    control_endpoint=$(sudo "$helper" authorize-attach "$1" "$(id -u)" "$(id -g)")
    exec 9>"$control_endpoint"
    exec 7<&0
    saved_tty=$(stty -g)
    restore_tty() {
        trap - 0 HUP TERM
        stty "$saved_tty"
    }
    forward_interrupt() {
        printf 'I' >&9 || true
    }
    forward_suspend() {
        printf 'S' >&9 || true
    }
    trap restore_tty 0 HUP TERM
    trap forward_interrupt INT
    trap forward_suspend TSTP
    "$helper" attach "$1" <&7 &
    attach_pid=$!
    status=0
    while kill -0 "$attach_pid" 2>/dev/null; do
        if wait "$attach_pid"; then
            status=0
            break
        else
            status=$?
            # A caught terminal signal interrupts wait so its trap can forward
            # immediately. The helper is still active; resume waiting for it.
            if kill -0 "$attach_pid" 2>/dev/null; then
                status=0
                continue
            fi
            break
        fi
    done
    restore_tty
    trap - INT TSTP
    exec 7<&-
    exec 9>&-
    exit "$status"
fi

exec sudo "$helper" "$command" "$@"
