#!/usr/bin/env bash

set -euo pipefail

root=$(git rev-parse --show-toplevel)
source_file=${1:-"$root/kernel/src/arch/x86_64/cpu.rs"}

fail() {
    printf 'x86 runtime-owner check: %s\n' "$*" >&2
    exit 1
}

declarations=$(rg -c '^static X86_RUNTIME_BACKING_' "$source_file")
[[ "$declarations" == 404 ]] || fail "expected 404 backing declarations, found $declarations"

initializers=$(awk '
    /^static X86_RUNTIME: X86RuntimeOwner = X86RuntimeOwner \{/ { owner = 1; next }
    owner && /^};/ { owner = 0 }
    owner && /X86_RUNTIME_BACKING_/ { count++ }
    END { print count + 0 }
' "$source_file")
[[ "$initializers" == 404 ]] || fail "expected 404 owner initializers, found $initializers"

bypass=$(awk '
    /^static X86_RUNTIME: X86RuntimeOwner = X86RuntimeOwner \{/ { owner = 1 }
    owner && /^};/ { owner = 0; next }
    /X86_RUNTIME_BACKING_/ && !owner && !/^static X86_RUNTIME_BACKING_/ { print NR ":" $0 }
' "$source_file")
[[ -z "$bypass" ]] || {
    printf '%s\n' "$bypass" >&2
    fail 'runtime backing accessed outside its declaration or owner initializer'
}

if [[ $# == 0 ]]; then
    negative=$(mktemp "${TMPDIR:-/tmp}/ginux-x86-owner-negative.XXXXXX")
    trap 'rm -f "$negative"' EXIT
    cp "$source_file" "$negative"
    printf '\nfn invalid_runtime_bypass() { let _ = &X86_RUNTIME_BACKING_LINUX_RUNTIME_MODE; }\n' >>"$negative"
    if "$0" "$negative" >/dev/null 2>&1; then
        fail 'negative test accepted a direct backing access'
    fi
fi

printf 'x86 runtime-owner check passed (%s managed backings)\n' "$declarations"
