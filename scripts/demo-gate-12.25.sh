#!/bin/sh
set -eu

printf '%s\n' 'Gate 12.25 top-level instance workflow'
printf '%s\n' \
    'ginux create rocky' \
    'ginux import rocky recipe.toml' \
    'ginux set rocky --set resources.vcpus=2 --network default' \
    'ginux edit rocky' \
    'ginux export rocky' \
    'ginux list' \
    'ginux start rocky --attach' \
    'ginux status rocky' \
    'ginux stop rocky'

printf '%s\n' 'GATE12.25: PASS (top-level instance creation, import, configuration, inspection, and lifecycle UX)'
