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
    exec sudo "$helper" attach "$name"
fi

exec sudo "$helper" "$command" "$@"
