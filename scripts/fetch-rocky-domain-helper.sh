#!/bin/sh
set -eu

. ./config/rocky-domain-image.env
docker pull "$ROCKY_DOMAIN_HELPER_IMAGE"
