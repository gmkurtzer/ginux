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
    attach|create|export|import|list|set|start|status|stop) ;;
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
    # Keep terminal-generated Ctrl-C out of sudo's foreground process group.
    # The attach transport switches to raw mode and forwards the ETX byte to
    # the instance; this wrapper restores the caller's terminal on every exit.
    saved_tty=$(stty -g)
    restore_tty() {
        trap - 0 HUP TERM
        stty "$saved_tty"
    }
    trap restore_tty 0 HUP TERM
    trap '' INT
    stty -isig
    status=0
    sudo "$helper" attach "$1" || status=$?
    restore_tty
    trap - INT
    exit "$status"
fi

exec sudo "$helper" "$command" "$@"
