#!/bin/sh
set -eu

cargo build -q -p ginux-cli
cli=target/debug/ginux-cli

printf '%s\n' 'Gate 25 public command surface:'
"$cli" --help
printf '%s\n' 'Gate 25 representative command help:'
"$cli" start --help

if "$cli" diff left right >/dev/null 2>&1 || \
    "$cli" native config root >/dev/null 2>&1; then
    printf '%s\n' 'private or removed command leaked into the public CLI' >&2
    exit 1
fi

printf '%s\n' 'GATE25: PASS (declarative help, approved public surface, private selector rejection)'
