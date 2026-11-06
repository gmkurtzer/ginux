#!/bin/sh
set -eu

./scripts/test-config.sh
./scripts/test-gate975-network.sh
./scripts/test-gate975-hooks.sh

printf '%s\n' \
    'Gate 9.75 demonstration: PASS' \
    '  - complete typed domain schema and CLI precedence' \
    '  - source/interface selection and unavailable-attachment rejection' \
    '  - ingress/egress denial, translated ports, conflicts, and cleanup' \
    '  - ordered mediated hooks, authority denial, failure policy, and timeout'
