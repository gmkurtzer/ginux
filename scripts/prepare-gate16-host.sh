#!/bin/sh
set -eu

. ./scripts/gate16-host-env.sh
. ./scripts/rocky-host-common.sh

export ROCKY_NETWORK_RESTRICT=off
./scripts/start-rocky-host.sh
rocky_ssh 'sudo dnf -y install gcc make kernel-devel-$(uname -r) \
    elfutils-libelf-devel clang llvm bpftool'
printf '%s\n' 'Gate 16 Rocky host prepared.'
