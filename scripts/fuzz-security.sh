#!/bin/sh
set -eu

seconds=${GINUX_FUZZ_SECONDS:-5}
artifacts=${GINUX_FUZZ_ARTIFACTS:-build/security-fuzz}
case $seconds in
    ''|*[!0-9]*|0) printf '%s\n' 'invalid GINUX_FUZZ_SECONDS' >&2; exit 2 ;;
esac
test "$seconds" -le 5 || {
    printf '%s\n' 'per-target fuzz duration exceeds Gate 13 bound' >&2
    exit 2
}

mkdir -p "$artifacts"
rm -f "$artifacts"/current-*.bin
# A host build: the target rustflags in .cargo/config.toml are freestanding,
# so clear them as test-unit and test-security do.
RUSTFLAGS='' cargo build --quiet -p security-fuzzer
fuzzer=${CARGO_TARGET_DIR:-target}/debug/security-fuzzer
test -x "$fuzzer"
# The memory bound is for the fuzzer. Set before the build, it also binds
# rustc, which cannot load LLVM under it.
ulimit -v 262144 2>/dev/null || true

for target in config selector elf vfs-index kernel-path device syscall audit; do
    timeout_seconds=$((seconds + 2))
    timeout_marker=$artifacts/timeout-$target
    rm -f "$timeout_marker"
    "$fuzzer" "$target" "$seconds" "$artifacts" &
    fuzz_pid=$!
    (
        sleep "$timeout_seconds"
        if kill -0 "$fuzz_pid" 2>/dev/null; then
            : >"$timeout_marker"
            kill -TERM "$fuzz_pid" 2>/dev/null || true
        fi
    ) &
    watchdog_pid=$!
    set +e
    wait "$fuzz_pid"
    fuzz_status=$?
    set -e
    kill "$watchdog_pid" 2>/dev/null || true
    wait "$watchdog_pid" 2>/dev/null || true
    if test "$fuzz_status" -ne 0 || test -e "$timeout_marker"; then
        printf 'SECURITY_FUZZ: target=%s result=FAIL artifacts=%s\n' \
            "$target" "$artifacts" >&2
        exit 1
    fi
    rm -f "$timeout_marker"
done

if find "$artifacts" -type f \( -name 'current-*' -o -name 'crash-*' \) | grep -q .; then
    printf 'SECURITY_FUZZ: untriaged artifact remains in %s\n' "$artifacts" >&2
    exit 1
fi
printf '%s\n' 'SECURITY_FUZZ: PASS (8 targets, deterministic corpora, bounded time/input/memory)'
