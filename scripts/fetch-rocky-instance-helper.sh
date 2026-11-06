#!/bin/sh
set -eu

. ./config/rocky-instance-image.env
docker pull "$ROCKY_INSTANCE_HELPER_IMAGE"
