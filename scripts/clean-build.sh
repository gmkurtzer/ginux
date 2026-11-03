#!/bin/sh
set -eu

rm -rf \
    build/initramfs-root \
    build/ginus-initramfs.cpio.gz \
    build/gate1-intercept.log \
    build/bench-baseline.log \
    build/bench-baseline.csv \
    build/bench-baseline-summary.csv

printf '%s\n' 'cleaned generated Gate 1 artifacts; retained build/cache'
