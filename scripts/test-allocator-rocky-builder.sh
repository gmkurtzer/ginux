#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

rocky_scp tests/fixtures/allocator-conformance.c \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/allocator-conformance.c"
rocky_scp tests/fixtures/allocator-benchmark.c \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/allocator-benchmark.c"
rocky_scp crates/ginux-allocator/src/lib.rs \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-rpmbuild/BUILD/ginux-0.1.0/crates/ginux-allocator/src/lib.rs"
rocky_ssh '
    set -eu
    cd /var/tmp/ginux-rpmbuild/BUILD/ginux-0.1.0
    export CARGO_TARGET_DIR=/var/tmp/ginux-rpmbuild/CARGO_TARGET
    RUSTFLAGS="-C link-arg=-Wl,-z,relro -C link-arg=-Wl,-z,now" \
        cargo build --offline --locked --release -p ginux-allocator
    allocator=/var/tmp/ginux-rpmbuild/CARGO_TARGET/release/libginux_allocator.so
    test -r "$allocator"
    cc -O2 -Wall -Wextra -Werror -pthread /var/tmp/allocator-conformance.c \
        -o /var/tmp/allocator-conformance
    cc -O2 -Wall -Wextra -Werror -pthread /var/tmp/allocator-benchmark.c \
        -o /var/tmp/allocator-benchmark
    LD_PRELOAD="$allocator" /var/tmp/allocator-conformance
    report=/var/tmp/allocator-benchmark.tsv
    provenance=/var/tmp/allocator-provenance.txt
    printf "%s\n" "allocator	threads	repetition	operations	elapsed_ns	ops_per_second" >"$report"
    {
        uname -a
        rustc --version
        cargo --version
        cc --version | sed -n "1p"
        printf "%s\n" "environment=Rocky Linux QEMU TCG"
    } >"$provenance"
    for threads in 1 2 4; do
        repetition=1
        while test "$repetition" -le 5; do
            native=$(/var/tmp/allocator-benchmark "$threads")
            guarded=$(LD_PRELOAD="$allocator" /var/tmp/allocator-benchmark "$threads")
            for row in "native:$native" "ginux-guarded:$guarded"; do
                kind=${row%%:*}
                values=${row#*:}
                operations=$(printf "%s\n" "$values" | sed -n "s/.*operations=\([0-9]*\).*/\1/p")
                elapsed=$(printf "%s\n" "$values" | sed -n "s/.*elapsed_ns=\([0-9]*\).*/\1/p")
                rate=$(printf "%s\n" "$values" | sed -n "s/.*ops_per_second=\([0-9.]*\).*/\1/p")
                test -n "$operations" && test -n "$elapsed" && test -n "$rate"
                printf "%s\t%s\t%s\t%s\t%s\t%s\n" \
                    "$kind" "$threads" "$repetition" "$operations" "$elapsed" "$rate" >>"$report"
            done
            repetition=$((repetition + 1))
        done
    done
    cat "$report"
'
mkdir -p .artifacts/allocator
rocky_scp \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/allocator-benchmark.tsv" \
    .artifacts/allocator/rocky-benchmark.tsv
rocky_scp \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/allocator-provenance.txt" \
    .artifacts/allocator/rocky-provenance.txt
printf '%s\n' 'GINUX_ALLOCATOR_ROCKY: PASS'
