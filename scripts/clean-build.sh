#!/bin/sh
set -eu

case "${1:-build}" in
    build) rm -rf build ;;
    *) printf '%s\n' 'refusing unsafe build cleanup' >&2; exit 1 ;;
esac
