#!/bin/sh
set -eu

helper=${GINUX_INSTANCE_HELPER:-/usr/local/libexec/ginux-instance-helper}
runtime=${GINUX_RUNTIME:-/usr/local/libexec/ginux-runtime}

usage() {
    printf '%s\n' 'usage:' >&2
    printf '%s\n' '  ginux instance create NAME FILE [--set KEY=TOML_VALUE]... [--network ATTACHMENT] [--forward SPEC]...' >&2
    printf '%s\n' '  ginux instance set NAME [--network ATTACHMENT] [--forward SPEC]...' >&2
    printf '%s\n' '  ginux instance list' >&2
    printf '%s\n' '  ginux instance start NAME [--attach]' >&2
    printf '%s\n' '  ginux instance attach NAME' >&2
    printf '%s\n' '  ginux instance status NAME' >&2
    printf '%s\n' '  ginux instance stop NAME' >&2
    printf '%s\n' '  ginux instance validate FILE' >&2
    printf '%s\n' '  ginux instance inspect FILE' >&2
    printf '%s\n' '  ginux instance explain FILE OPERATION' >&2
    printf '%s\n' '  ginux instance diff LEFT RIGHT' >&2
    exit 64
}

test "$#" -ge 2 || usage
test "$1" = instance || usage
shift

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
    attach|create|list|set|start|status|stop) ;;
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
