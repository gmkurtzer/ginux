#!/bin/sh
set -eu

rm -rf \
    build/rocky-seed \
    build/rocky-seed.iso \
    build/gate1-intercept.log \
    build/gate3-runtime.log \
    build/gate3-malformed.log \
    build/bench-baseline.log \
    build/bench-baseline.csv \
    build/bench-baseline-summary.csv \
    build/rocky-instance-work

printf '%s\n' 'cleaned generated artifacts; retained host overlay and build/cache'
