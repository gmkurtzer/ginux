#!/bin/sh

export ROCKY_SSH_PORT=${ROCKY_SSH_PORT:-22416}
export ROCKY_PID_FILE=${ROCKY_PID_FILE:-build/gate16-host.pid}
export ROCKY_SERIAL_LOG=${ROCKY_SERIAL_LOG:-build/gate16-host-serial.log}
export ROCKY_OVERLAY_DISK=${ROCKY_OVERLAY_DISK:-build/gate16-host-overlay.qcow2}
export ROCKY_VCPUS=${ROCKY_VCPUS:-4}
export ROCKY_MEMORY_MIB=${ROCKY_MEMORY_MIB:-4096}
