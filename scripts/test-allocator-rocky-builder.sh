#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

rocky_scp tests/fixtures/allocator-conformance.c \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/allocator-conformance.c"
rocky_scp tests/fixtures/allocator-benchmark.c \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/allocator-benchmark.c"
rocky_ssh '
    set -eu
    rm -rf /var/tmp/ginux-allocator-source
    mkdir -p /var/tmp/ginux-allocator-source
    tar -xf /var/tmp/ginux-rpmbuild/SOURCES/ginux-0.1.0.tar.gz \
        -C /var/tmp/ginux-allocator-source --strip-components=1
    tar -xf /var/tmp/ginux-rpmbuild/SOURCES/ginux-0.1.0-vendor.tar.gz \
        -C /var/tmp/ginux-allocator-source
    cat >>/var/tmp/ginux-allocator-source/.cargo/config.toml <<"EOF"

[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF
'
rocky_scp crates/ginux-allocator/src/lib.rs \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-allocator-source/crates/ginux-allocator/src/lib.rs"
rocky_ssh '
    set -eu
    cd /var/tmp/ginux-allocator-source
    export CARGO_TARGET_DIR=/var/tmp/ginux-rpmbuild/CARGO_TARGET
    RUSTFLAGS="-C link-arg=-Wl,-z,relro -C link-arg=-Wl,-z,now" \
        cargo build --offline --locked --release -p ginux-allocator
    allocator=/var/tmp/ginux-rpmbuild/CARGO_TARGET/release/libginux_allocator.so
    jemalloc=/usr/lib64/libjemalloc.so.2
    test -r "$allocator"
    test -r "$jemalloc"
    cc -O2 -Wall -Wextra -Werror -pthread /var/tmp/allocator-conformance.c \
        -ldl -o /var/tmp/allocator-conformance
    cc -O2 -Wall -Wextra -Werror -pthread /var/tmp/allocator-benchmark.c \
        -o /var/tmp/allocator-benchmark
    LD_PRELOAD="$allocator" /var/tmp/allocator-conformance
    report=/var/tmp/allocator-benchmark.tsv
    provenance=/var/tmp/allocator-provenance.txt
    printf "%s\n" "allocator	threads	repetition	operations	elapsed_ns	ops_per_second	latency_ns	peak_rss_kib	live_requested_bytes	live_rss_delta_bytes	fragmentation_ratio" >"$report"
    {
        uname -a
        rustc --version
        cargo --version
        cc --version | sed -n "1p"
        rpm -q jemalloc
        sha256sum "$allocator" /var/tmp/allocator-benchmark
        printf "%s\n" "environment=Rocky Linux QEMU TCG"
    } >"$provenance"
    for threads in 1 2 4; do
        repetition=1
        while test "$repetition" -le 5; do
            set +e
            native=$(/var/tmp/allocator-benchmark "$threads")
            native_status=$?
            jemalloc_result=$(LD_PRELOAD="$jemalloc" /var/tmp/allocator-benchmark "$threads")
            jemalloc_status=$?
            guarded=$(LD_PRELOAD="$allocator" /var/tmp/allocator-benchmark "$threads")
            guarded_status=$?
            set -e
            test "$native_status" -eq 0 || {
                printf "allocator benchmark failed: allocator=native threads=%s status=%s\n" \
                    "$threads" "$native_status" >&2
                exit "$native_status"
            }
            test "$jemalloc_status" -eq 0 || {
                printf "allocator benchmark failed: allocator=jemalloc threads=%s status=%s\n" \
                    "$threads" "$jemalloc_status" >&2
                exit "$jemalloc_status"
            }
            test "$guarded_status" -eq 0 || {
                printf "allocator benchmark failed: allocator=ginux-guarded threads=%s status=%s\n" \
                    "$threads" "$guarded_status" >&2
                exit "$guarded_status"
            }
            for row in "native:$native" "jemalloc:$jemalloc_result" "ginux-guarded:$guarded"; do
                kind=${row%%:*}
                values=${row#*:}
                operations=$(printf "%s\n" "$values" | sed -n "s/.*operations=\([0-9]*\).*/\1/p")
                elapsed=$(printf "%s\n" "$values" | sed -n "s/.*elapsed_ns=\([0-9]*\).*/\1/p")
                rate=$(printf "%s\n" "$values" | sed -n "s/.*ops_per_second=\([0-9.]*\).*/\1/p")
                latency=$(printf "%s\n" "$values" | sed -n "s/.*latency_ns=\([0-9.]*\).*/\1/p")
                peak=$(printf "%s\n" "$values" | sed -n "s/.*peak_rss_kib=\([0-9]*\).*/\1/p")
                requested=$(printf "%s\n" "$values" | sed -n "s/.*live_requested_bytes=\([0-9]*\).*/\1/p")
                resident=$(printf "%s\n" "$values" | sed -n "s/.*live_rss_delta_bytes=\([0-9]*\).*/\1/p")
                fragmentation=$(printf "%s\n" "$values" | sed -n "s/.*fragmentation_ratio=\([0-9.]*\).*/\1/p")
                test -n "$operations" && test -n "$elapsed" && test -n "$rate" && \
                    test -n "$latency" && test -n "$peak" && test -n "$requested" && \
                    test -n "$resident" && test -n "$fragmentation"
                printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n" \
                    "$kind" "$threads" "$repetition" "$operations" "$elapsed" "$rate" \
                    "$latency" "$peak" "$requested" "$resident" "$fragmentation" >>"$report"
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
printf 'source_commit=%s\n' "$(git rev-parse HEAD)" >> \
    .artifacts/allocator/rocky-provenance.txt
printf '%s\n' 'GINUX_ALLOCATOR_ROCKY: PASS'
