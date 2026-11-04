#!/bin/sh
set -eu

./scripts/stop-rocky-host.sh

rm -rf \
    build/rocky-seed \
    build/rocky-seed.iso \
    build/rocky-host-overlay.qcow2 \
    build/rocky-host.pid \
    build/rocky-host-serial.log \
    build/gate1-intercept.log \
    build/bench-baseline.log \
    build/bench-baseline.csv \
    build/bench-baseline-summary.csv

printf '%s\n' 'cleaned generated Gate 1 artifacts; retained build/cache'
