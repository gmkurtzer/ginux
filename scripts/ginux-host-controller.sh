#!/bin/sh
set -eu

helper=/usr/local/libexec/ginux-domain-helper

usage() {
    printf '%s\n' 'usage:' >&2
    printf '%s\n' '  ginux domain create NAME --config FILE' >&2
    printf '%s\n' '  ginux domain list' >&2
    printf '%s\n' '  ginux domain start NAME [--attach]' >&2
    printf '%s\n' '  ginux domain attach NAME' >&2
    printf '%s\n' '  ginux domain status NAME' >&2
    printf '%s\n' '  ginux domain stop NAME' >&2
    exit 64
}

test "$#" -ge 2 || usage
test "$1" = domain || usage
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
