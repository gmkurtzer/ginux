#!/bin/sh
set -eu

cd "$(dirname "$0")/.."

case $(uname -s):$(uname -m) in
    Linux:x86_64) ;;
    *) printf '%s\n' 'allocator Linux integration requires Linux x86-64' >&2; exit 77 ;;
esac

RUSTFLAGS='-C link-arg=-Wl,-z,relro -C link-arg=-Wl,-z,now' \
    cargo build --locked --release -p ginux-allocator
allocator=target/release/libginux_allocator.so
test -r "$allocator"

mkdir -p .artifacts/allocator
cc -O2 -Wall -Wextra -Werror -pthread tests/fixtures/allocator-conformance.c \
    -ldl -o .artifacts/allocator/conformance
cc -O2 -Wall -Wextra -Werror -pthread tests/fixtures/allocator-benchmark.c \
    -o .artifacts/allocator/benchmark

LD_PRELOAD="$PWD/$allocator" .artifacts/allocator/conformance

report=.artifacts/allocator/benchmark.tsv
provenance=.artifacts/allocator/provenance.txt
printf '%s\n' 'allocator	threads	repetition	operations	elapsed_ns	ops_per_second' >"$report"
{
    uname -a
    rustc --version
    cargo --version
    cc --version | sed -n '1p'
} >"$provenance"
for threads in 1 2 4; do
    repetition=1
    while test "$repetition" -le 5; do
        native=$(.artifacts/allocator/benchmark "$threads")
        guarded=$(LD_PRELOAD="$PWD/$allocator" .artifacts/allocator/benchmark "$threads")
        for row in "native:$native" "ginux-guarded:$guarded"; do
            kind=${row%%:*}
            values=${row#*:}
            operations=$(printf '%s\n' "$values" | sed -n 's/.*operations=\([0-9]*\).*/\1/p')
            elapsed=$(printf '%s\n' "$values" | sed -n 's/.*elapsed_ns=\([0-9]*\).*/\1/p')
            rate=$(printf '%s\n' "$values" | sed -n 's/.*ops_per_second=\([0-9.]*\).*/\1/p')
            test -n "$operations" && test -n "$elapsed" && test -n "$rate"
            printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
                "$kind" "$threads" "$repetition" "$operations" "$elapsed" "$rate" >>"$report"
        done
        repetition=$((repetition + 1))
    done
done

cat "$report"
printf '%s\n' 'GINUX_ALLOCATOR_LINUX: PASS'
