#!/bin/sh
set -eu

helper=/usr/local/libexec/ginux-instance-helper
runtime=/usr/local/libexec/ginux-runtime

usage() {
    printf '%s\n' 'usage:' >&2
    printf '%s\n' '  ginux instance create NAME --config FILE [--set KEY=TOML_VALUE]...' >&2
    printf '%s\n' '  ginux instance list' >&2
    printf '%s\n' '  ginux instance start NAME [--attach]' >&2
    printf '%s\n' '  ginux instance attach NAME' >&2
    printf '%s\n' '  ginux instance status NAME' >&2
    printf '%s\n' '  ginux instance stop NAME' >&2
    printf '%s\n' '  ginux contract init PRESET' >&2
    printf '%s\n' '  ginux contract check|show|inspect FILE' >&2
    printf '%s\n' '  ginux contract explain FILE OPERATION' >&2
    printf '%s\n' '  ginux contract diff LEFT RIGHT' >&2
    exit 64
}

test "$#" -ge 2 || usage
if test "$1" = contract; then
    shift
    command=$1
    shift
    case $command in
        init)
            test "$#" -eq 1 || usage
            exec "$runtime" contract-init "$1"
            ;;
        check|show|inspect)
            test "$#" -eq 1 || usage
            exec "$runtime" "contract-$command" "$1"
            ;;
        explain)
            test "$#" -eq 2 || usage
            exec "$runtime" contract-explain "$1" "$2"
            ;;
        diff)
            test "$#" -eq 2 || usage
            exec "$runtime" contract-diff "$1" "$2"
            ;;
        *) usage ;;
    esac
fi
test "$1" = instance || usage
shift

command=$1
shift
case $command in
    attach|create|list|start|status|stop) ;;
    *) usage ;;
esac

if test "$command" = start && test "$#" -eq 2 && test "$2" = --attach; then
    name=$1
    sudo "$helper" start "$name" --attach
    exec sudo "$helper" attach "$name"
fi

exec sudo "$helper" "$command" "$@"
