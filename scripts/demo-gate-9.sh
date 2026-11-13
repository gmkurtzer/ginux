#!/bin/sh
set -eu

printf '%s\n' 'Gate 9 human demonstration: Rocky Linux 10 systemd under Ginux.'
printf '%s\n' 'This opens the Rocky QEMU host and leaves every instance action to you.'
printf '%s\n' 'The session remains exploratory until you shut down the instance and exit the host.'
printf '%s\n' ''

exec ./scripts/run-host.sh
