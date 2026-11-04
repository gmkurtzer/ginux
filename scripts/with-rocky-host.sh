#!/bin/sh
set -eu

cleanup() {
    ./scripts/stop-rocky-host.sh
}
trap cleanup 0 HUP INT TERM

./scripts/start-rocky-host.sh
"$@"
