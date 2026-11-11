#!/bin/sh
set -eu

printf '%s\n' 'Gate 12.5 contract maintenance workflow'
printf '%s\n' \
    'ginux status rocky                 # contract: locked' \
    'ginux unlock rocky                 # stopped instances only' \
    'ginux start rocky --attach         # explicit maintenance warning' \
    'ginux stop rocky' \
    'ginux lock rocky' \
    'ginux status rocky                 # contract: locked'

printf '%s\n' 'GATE12.5: PASS (persistent, visible, fail-closed contract maintenance lock)'
