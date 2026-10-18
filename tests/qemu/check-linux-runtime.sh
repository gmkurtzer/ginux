#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 1 || ! -f "$1" ]]; then
    echo "Usage: $0 BOOT-OUTPUT" >&2
    exit 2
fi

boot_output="$1"
failed=0

check_category() {
    local category="$1"
    shift
    local marker
    local missing=0

    for marker in "$@"; do
        if ! grep -Fq "ginux: ${marker}" "$boot_output"; then
            echo "Linux runtime regression: ${category}: missing '${marker}'." >&2
            missing=1
        fi
    done

    if [[ "$missing" -eq 0 ]]; then
        printf '%-24s PASS\n' "$category"
    else
        printf '%-24s FAIL\n' "$category"
        failed=1
    fi
}

echo "Linux ELF/runtime compatibility"
printf '%-24s %s\n' "CATEGORY" "RESULT"
check_category "ELF64 executable forms" \
    "toolchain-linked static Linux ELF executed" \
    "three-load Linux ELF runtime mapping verified" \
    "bounded Linux ET_DYN load bias validated" \
    "Linux static executable metadata validated"
check_category "PIE and process entry" \
    "toolchain-linked Linux PIE and shared object executed" \
    "static Linux exec auxiliary vector rebuilt" \
    "interpreted Linux exec auxiliary vector rebuilt" \
    "Linux auxiliary vector encoding validated"
check_category "Dynamic loader" \
    "toolchain-linked Linux interpreter executed" \
    "interpreter-first Linux ring-3 entry verified" \
    "linker-emitted Linux dynamic tags validated" \
    "bounded Linux dependency authority graph validated"
check_category "Libraries and symbols" \
    "userspace Linux interpreter self-relocation applied" \
    "userspace Linux relative relocation applied" \
    "userspace Linux GLOB_DAT relocation applied" \
    "userspace Linux JUMP_SLOT relocation applied" \
    "userspace Linux cross-object symbol resolved"
check_category "TLS and initialization" \
    "combined Linux static TLS materialized" \
    "Linux TLS active before initializers" \
    "userspace Linux main preinit executed" \
    "userspace Linux dependency constructors executed" \
    "Linux dependency-before-main initialization ordered"
check_category "Heap and process break" \
    "Linux bounded brk heap validated" \
    "Linux heap copy bounds validated" \
    "Linux exec process break reset"
check_category "Virtual memory" \
    "Linux bounded anonymous mapping lifecycle validated" \
    "bounded Linux mmap area splitting validated" \
    "Linux anonymous mapping write seal validated" \
    "Linux mmap frames allocated on demand" \
    "Linux mmap frame leases reclaimed"
check_category "ELF hardening" \
    "non-executable Linux GNU stack enforced" \
    "Linux RELRO ownership permissions synchronized" \
    "Linux dependency W^X mapping verified" \
    "Linux page-aliasing ELF segments rejected" \
    "Linux ELF entry metadata overlap rejected"
check_category "Exec lifecycle" \
    "old Linux image pages removed after exec" \
    "replaced Linux image retired cleanly" \
    "complete interpreted Linux graph retired at exec" \
    "static Linux mmap registry empty across exec" \
    "interpreted Linux mmap registry empty across exec"

if [[ "$(grep -Fc 'ginux: measured Linux exec image replaced atomically' "$boot_output")" -ne 5 ]]; then
    echo "Linux runtime regression: Exec lifecycle: expected five atomic image replacements." >&2
    failed=1
fi

if [[ "$failed" -ne 0 ]]; then
    exit 1
fi

echo "Linux ELF/runtime compatibility passed"
