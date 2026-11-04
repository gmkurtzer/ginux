#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

rocky_ssh 'test "$(cat /proc/1/comm)" = systemd'
rocky_ssh 'systemctl is-active --quiet multi-user.target'
rocky_ssh '. /etc/os-release; test "$ID" = rocky; case "$VERSION_ID" in 10*) ;; *) exit 1 ;; esac'
printf '%s\n' 'HOST: Rocky Linux 10 ready with systemd as PID 1'
